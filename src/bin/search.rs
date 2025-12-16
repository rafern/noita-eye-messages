use hot_eval::codegen::compiled_expression::CompiledExpression;
use hot_eval::codegen::jit_context::JITContext;
use hot_eval::common::binding::{Binding, BindingFunctionParameter};
use hot_eval::common::table::Table;
use hot_eval::common::value_type::ValueType;
use prost::Message;
use clap::Parser;
use noita_eye_messages::analysis::freq::UnitFrequency;
use noita_eye_messages::ciphers::base::{Cipher, CipherWorkerContext, CipherCodecContext};
use noita_eye_messages::ciphers::deserialise_cipher;
use noita_eye_messages::data::key_dump::KeyDumpMeta;
use rug::{Integer, Rational};
use std::cell::OnceCell;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::num::NonZeroU32;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::time::{Duration, Instant};
use noita_eye_messages::utils::threading::get_parallelism;
use noita_eye_messages::data::message::MessageList;
use noita_eye_messages::utils::print::{MessagePrintConfig, format_big_float, format_big_uint, format_seconds_left, print_message};
use noita_eye_messages::data::csv_import::{import_csv_languages_or_exit, import_csv_messages_or_exit};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(clap::Parser)]
struct Args {
    /// Path to CSV file containing message data
    data_path: std::path::PathBuf,
    /// Condition to match. Values greater than 0 are treated as true, which should make it easy to use heuristics with thresholds as conditions (simply subtract the threshold value from the heuristic)
    condition: String,
    /// Cipher to use
    cipher: String,
    /// Cipher configuration. Format is cipher-specific, but generally expected to be Rusty Object Notation. It's recommended to add this as the last argument after a "--"
    config: Option<String>,
    /// Disable parallelism (search messages using only the main thread). Equivalent to setting max parallelism to 1, but takes priority over max parallelism
    #[arg(short, long)]
    sequential: bool,
    /// Maximum number of workers (including main thread). Using all available cores has diminishing returns, so tweaking this value is recommended
    #[arg(short, long)]
    max_parallelism: Option<NonZeroU32>,
    /// Path to CSV file containing a language's letter frequency distribution. Used to register languages. Refer to a language by its index (0-based) in the order specified in the terminal
    #[clap(short, long)]
    language: Vec<std::path::PathBuf>,
    /// Path to key dump file, if you want to store matches in a file instead of logging to the console
    #[arg(short, long)]
    key_dump_path: Option<std::path::PathBuf>,
}

enum TaskPacket {
    Finished {
        worker_id: u32,
    },
    Progress {
        keys: u32,
    },
    Match {
        // XXX it doesn't really make sense to be passing around protobuf
        //     messages like this, but the project is still in a weird
        //     transition state where it doesn't support distributed computing
        //     yet, but will
        net_key: Vec<u8>,
    },
    Error {
        message: String,
    }
}

#[derive(Debug)]
pub enum PredicateError {
    BadExpressionType,
}

impl fmt::Display for PredicateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Self::BadExpressionType => "Bad expression type; expected a predicate (boolean)",
        })
    }
}

impl Error for PredicateError {}

const RECV_TIMEOUT: Duration = Duration::from_secs(1);

// TODO suspend to/resume from file
// TODO bin to read key dumps
// TODO bin to decrypt with individual key
// TODO bin to refine a search via key dump files

fn preamble(messages: &MessageList, worker_total: u32, keys_total: &Integer) {
    let mut working_messages: MessageList = messages.clone();

    println!("Searching {} keys with {} workers. Ciphertexts (mod_add 32):", format_big_uint(keys_total), worker_total);

    for m in 0..working_messages.len() {
        let msg = &mut working_messages[m];
        for i in 0..msg.data.len() {
            msg.data[i] = msg.data[i] + 32;
        }

        print_message(msg, MessagePrintConfig::default());
    }

    println!();
}

fn print_progress(time_range: Option<(&Instant, &Instant)>, secs_since_last: f64, keys_total: &Integer, keys_checked: &Integer, keys_checked_since_last_print: &Integer) {
    let percent = if *keys_total == 0 {
        100.0
    } else {
        Rational::from((&*keys_checked * 100, &*keys_total)).to_f32()
    };

    let kps = keys_checked_since_last_print.to_f64() / secs_since_last;
    let print_begin = format!("Progress: {percent:.2}% checked ({}/{} keys), {} keys/sec", format_big_uint(&keys_checked), format_big_uint(&keys_total), format_big_float(kps));

    match time_range {
        Some((start_time, now)) => {
            let secs_left = Rational::from((&*keys_total - &*keys_checked, &*keys_checked)).to_f64() * now.duration_since(*start_time).as_secs_f64();
            println!("{} ({})", print_begin, format_seconds_left(secs_left));
        },
        None => {
            println!("{}", print_begin);
        }
    }
}

fn eval_pt<T: CipherWorkerContext>(codec_ctx: &T::DecryptionContext, m: usize, u: usize) -> u8 {
    codec_ctx.decrypt(m, u)
}

fn eval_pt_freq_dist_error<T: CipherWorkerContext>(codec_ctx: &T::DecryptionContext, pt_freq_dist: &OnceCell<UnitFrequency>, languages: &Vec<UnitFrequency>, l: usize) -> f64 {
    languages[l].get_error(pt_freq_dist.get_or_init(|| {
        UnitFrequency::from_messages(&codec_ctx.get_all_plaintexts())
    }))
}

fn search_task<'str, T: CipherWorkerContext>(_worker_id: u32, messages: &MessageList, worker_ctx: T, cond_src: &'str String, languages: &Vec<UnitFrequency>, tx: &SyncSender<TaskPacket>) -> Result<(), Box<dyn Error + 'str>> {
    let mut jit_ctx = JITContext::new();
    let mut comp_ctx = jit_ctx.make_compilation_context()?;
    let mut pt_freq_dist = OnceCell::<UnitFrequency>::new();
    let mut cond_table = Table::new();
    let codec_ctx_hsi = cond_table.add_hidden_state(ValueType::USize);
    let pt_freq_dist_hsi = cond_table.add_hidden_state(ValueType::USize);
    let languages_hsi = cond_table.add_hidden_state(ValueType::USize);

    cond_table.add_binding("pt".into(), Binding::Function {
        ret_type: ValueType::U8,
        params: vec![
            // codec_ctx: &T::DecryptionContext
            BindingFunctionParameter::HiddenStateArgument { hidden_state_idx: codec_ctx_hsi, cast_to_type: None },
            // m: usize
            BindingFunctionParameter::Parameter { value_type: ValueType::USize },
            // u: usize
            BindingFunctionParameter::Parameter { value_type: ValueType::USize },
        ],
        // TODO don't be tempted to replace `eval_pt::<T>` with
        //      `T::DecryptionContext::decrypt`. for some reason it's slightly
        //      slower. investigate why?
        fn_ptr: eval_pt::<T> as *const (),
    })?;

    cond_table.add_binding("pt_freq_dist_error".into(), Binding::Function {
        ret_type: ValueType::U8,
        params: vec![
            // codec_ctx: &T::DecryptionContext
            BindingFunctionParameter::HiddenStateArgument { hidden_state_idx: codec_ctx_hsi, cast_to_type: None },
            // pt_freq_dist: &OnceCell<UnitFrequency>
            BindingFunctionParameter::HiddenStateArgument { hidden_state_idx: pt_freq_dist_hsi, cast_to_type: None },
            // languages: &Vec<UnitFrequency>
            BindingFunctionParameter::HiddenStateArgument { hidden_state_idx: languages_hsi, cast_to_type: None },
            // l: usize
            BindingFunctionParameter::Parameter { value_type: ValueType::USize },
        ],
        fn_ptr: eval_pt_freq_dist_error::<T> as *const (),
    })?;

    let cond = comp_ctx.compile_str(&cond_src, &cond_table)?;

    let (mut slab, jit_fn) = match cond {
        CompiledExpression::Bool { mut slab, jit_fn } => {
            slab.set_ptr_value(pt_freq_dist_hsi, &pt_freq_dist);
            slab.set_ptr_value(languages_hsi, &languages);
            (slab, jit_fn)
        },
        _ => {
            return Err(PredicateError::BadExpressionType.into());
        },
    };

    worker_ctx.permute_keys_interruptible(messages, &mut |codec_ctx| {
        // TODO clearing the cache results in a 5% slowdown. hot-eval should
        //      support pure functions, so that it reuses outputs when possible,
        //      otherwise we have to unnecessarily clear a cache and manage our
        //      own lazy cell, even when there's only a single call in the
        //      expression
        pt_freq_dist.take(); // clear cache

        slab.set_ptr_value(codec_ctx_hsi, codec_ctx);

        if unsafe { jit_fn.call() } {
            tx.send(TaskPacket::Match { net_key: codec_ctx.get_current_key_net() }).unwrap();
        }
    }, &mut |_codec_ctx, keys| {
        tx.send(TaskPacket::Progress { keys }).unwrap();
        true
    });

    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let languages = import_csv_languages_or_exit(&args.language);

    let messages = import_csv_messages_or_exit(&args.data_path);
    if messages.len() == 0 {
        // TODO return error result instead
        eprintln!("Nothing to do; need at least one message");
        std::process::exit(1);
    }

    let cipher = deserialise_cipher(&args.cipher, &args.config)?;

    let mut key_dump_file: Option<File> = match &args.key_dump_path {
        Some(path) => {
            let mut file = File::create_new(path)?;
            let meta = KeyDumpMeta {
                build_hash: String::from(env!("GIT_HASH")),
                cipher_name: args.cipher.clone(),
                cipher_config: args.config.clone(),
            };
            // FIXME is there a way to write directly to the file?
            file.write(meta.encode_to_vec().as_slice())?;
            Some(file)
        },
        None => None,
    };

    let worker_total = if args.sequential {
        1u32
    } else {
        let mut max_parallelism: u32 = args.max_parallelism.unwrap_or(unsafe { NonZeroU32::new_unchecked(u32::MAX) }).into();
        max_parallelism = max_parallelism.min(cipher.get_max_parallelism());
        get_parallelism().min(max_parallelism)
    };

    let (tx, rx) = sync_channel::<TaskPacket>(64);

    std::thread::scope(|scope| {
        let mut keys_total = Integer::new();
        let mut worker_ctxs = Vec::new();

        for worker_id in 0..worker_total {
            let worker_ctx = cipher.create_worker_context_parallel(worker_id, worker_total);
            keys_total += worker_ctx.get_total_keys();
            worker_ctxs.push(worker_ctx);
        }

        preamble(&messages, worker_total, &keys_total);

        let start_time = Instant::now();

        let mut worker_id = 0;
        for worker_ctx in worker_ctxs {
            let worker_id_clone = worker_id.clone();
            let messages = &messages;
            let cond_src = &args.condition;
            let languages = &languages;
            let tx = tx.clone();

            scope.spawn(move || {
                match search_task(worker_id_clone, messages, worker_ctx, cond_src, languages, &tx) {
                    Ok(_) => tx.send(TaskPacket::Finished { worker_id }).unwrap(),
                    Err(err) => tx.send(TaskPacket::Error { message: err.to_string() }).unwrap(),
                }
            });

            worker_id += 1;
        }

        drop(tx);

        let mut keys_checked = Integer::new();
        let mut keys_checked_since_last_print = Integer::new();
        let mut last_print = start_time.clone();
        let mut workers_waiting = worker_total;

        while workers_waiting > 0 {
            match rx.recv_timeout(RECV_TIMEOUT) {
                Ok(packet) => {
                    match packet {
                        TaskPacket::Finished { worker_id } => {
                            workers_waiting -= 1;
                            println!("Worker {worker_id} finished task");
                        },
                        TaskPacket::Progress { keys } => {
                            keys_checked_since_last_print += keys;
                        },
                        TaskPacket::Match { net_key } => {
                            match key_dump_file {
                                Some(ref mut file) => {
                                    // FIXME is there a way to write directly to the file?
                                    file.write(net_key.encode_to_vec().as_slice())?;
                                },
                                None => {
                                    println!("Matched key {}", cipher.net_key_to_string(net_key));
                                },
                            }
                        },
                        TaskPacket::Error { message } => {
                            workers_waiting -= 1;
                            println!("Worker {worker_id} errored: {message}");
                            // TODO kill other workers?
                        },
                    }
                },
                Err(err) => {
                    match err {
                        RecvTimeoutError::Timeout => { /* do nothing */ },
                        RecvTimeoutError::Disconnected => {
                            println!("Worker channel disconnected (thread died?)");
                            return Err(err)?;
                        },
                    }
                },
            }

            let now = Instant::now();
            let secs_since_last = now.duration_since(last_print).as_secs_f64();
            if secs_since_last >= 5f64 {
                keys_checked += &keys_checked_since_last_print;

                print_progress(
                    Some((&start_time, &now)),
                    secs_since_last,
                    &keys_total,
                    &keys_checked,
                    &keys_checked_since_last_print,
                );

                last_print = now;
                keys_checked_since_last_print = Integer::new();
            }
        }

        keys_checked += &keys_checked_since_last_print;

        print_progress(
            None,
            Instant::now().duration_since(last_print).as_secs_f64(),
            &keys_total,
            &keys_checked,
            &keys_checked_since_last_print,
        );

        Ok(())
    })
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}