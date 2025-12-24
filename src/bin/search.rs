use hot_eval::codegen::compiled_expression::CompiledExpression;
use hot_eval::codegen::jit_context::JITContext;
use hot_eval::common::binding::{Binding, FnPointer, FnSpecCallArg, FnSpecChoice};
use hot_eval::common::ir_const::IRConst;
use hot_eval::common::table::Table;
use hot_eval::common::value::Value;
use hot_eval::common::value_type::ValueType;
use noita_eye_messages::analysis::alphabet::Alphabet;
use noita_eye_messages::data::alphabet_io::import_csv_alphabet_or_default;
use noita_eye_messages::data::language_io::import_csv_languages;
use noita_eye_messages::data::message_io::import_messages;
use noita_eye_messages::data::render_message::MessageRenderMap;
use noita_eye_messages::main_error_wrap;
use noita_eye_messages::utils::run::UnitResult;
use prost::Message;
use clap::Parser;
use noita_eye_messages::analysis::unit_freq::UnitFrequency;
use noita_eye_messages::ciphers::base::{Cipher, CipherCodecContext, CipherKey, CipherWorkerContext};
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
use noita_eye_messages::data::message::{AcceleratedMessageList, InterleavedMessageData};
use noita_eye_messages::utils::print::{MessagesPrintConfig, format_big_float, format_big_uint, format_seconds_left, print_messages};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[derive(clap::Parser)]
struct Args {
    /// Path to CSV or TXT file containing message data
    data_path: std::path::PathBuf,
    /// Condition to match. Values greater than 0 are treated as true, which should make it easy to use heuristics with thresholds as conditions (simply subtract the threshold value from the heuristic)
    condition: Box<str>,
    /// Cipher to use
    cipher: Box<str>,
    /// Cipher configuration. Format is cipher-specific, but generally expected to be Rusty Object Notation. It's recommended to add this as the last argument after a "--"
    config: Option<Box<str>>,
    /// Encrypt input message instead of decrypting (disabled by default)
    #[arg(short, long)]
    encrypt: bool,
    /// Disable parallelism (search messages using only the main thread). Equivalent to setting max parallelism to 1, but takes priority over max parallelism
    #[arg(short, long)]
    sequential: bool,
    /// Maximum number of workers (including main thread). Using all available cores has diminishing returns, so tweaking this value is recommended
    #[arg(short, long)]
    max_parallelism: Option<NonZeroU32>,
    /// Path to CSV file containing an alphabet with letter frequency distribution. Used to register languages for doing analysis. Refer to a language by its index (0-based) in the order specified in the terminal
    #[clap(short, long)]
    language: Vec<std::path::PathBuf>,
    /// Path to key dump file, if you want to store matches in a file instead of logging to the console
    #[arg(short, long)]
    key_dump_path: Option<std::path::PathBuf>,
    /// Path to alphabet file for interpreting the units in the message data. Any character not present in the alphabet will not be included in the message. If not passed, then an ASCII alphabet which includes all units will be used by default
    #[arg(short, long)]
    alphabet: Option<std::path::PathBuf>,
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
        net_key: Box<[u8]>,
    },
    Error {
        message: Box<str>,
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

fn preamble(messages_render_map: &MessageRenderMap, alphabet: &Alphabet, worker_total: u32, keys_total: &Integer, decrypt: bool) {
    println!("Searching {} keys with {} workers", format_big_uint(keys_total), worker_total);
    let title = if decrypt { "Ciphertexts" } else { "Plaintexts" };
    print_messages(title, messages_render_map, alphabet, &MessagesPrintConfig::default());
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

fn eval_in(messages: &InterleavedMessageData, m: usize, u: usize) -> u8 {
    messages[(m, u)]
}

fn eval_in_freq_dist_error(in_freq_dist_errors: &Box<[f64]>, l: usize) -> f64 {
    in_freq_dist_errors[l]
}

fn eval_out<const DECRYPT: bool, K, W>(codec_ctx: &W::CodecContext<'_, DECRYPT>, m: usize, u: usize) -> u8
where
    K: CipherKey,
    W: CipherWorkerContext<K>,
{
    codec_ctx.get_output(m, u)
}

unsafe fn eval_out_unchecked<const DECRYPT: bool, K, W>(codec_ctx: &W::CodecContext<'_, DECRYPT>, m: usize, u: usize) -> u8
where
    K: CipherKey,
    W: CipherWorkerContext<K>,
{
    // SAFETY: caller must verify bounds
    unsafe { codec_ctx.get_output_unchecked(m, u) }
}

fn eval_out_freq_dist_error_specific<const DECRYPT: bool, K, W>(codec_ctx: &W::CodecContext<'_, DECRYPT>, out_freq_dist: &OnceCell<UnitFrequency>, language: &UnitFrequency) -> f64
where
    K: CipherKey,
    W: CipherWorkerContext<K>,
{
    language.get_error(out_freq_dist.get_or_init(|| {
        UnitFrequency::from_message_data_list(&codec_ctx.get_output_messages())
    }))
}

fn eval_out_freq_dist_error<const DECRYPT: bool, K, W>(codec_ctx: &W::CodecContext<'_, DECRYPT>, out_freq_dist: &OnceCell<UnitFrequency>, languages: &Vec<UnitFrequency>, l: usize) -> f64
where
    K: CipherKey,
    W: CipherWorkerContext<K>,
{
    eval_out_freq_dist_error_specific::<DECRYPT, K, W>(codec_ctx, out_freq_dist, &languages[l])
}

fn search_task<'inputs, 'src, const DECRYPT: bool, K, W>(_worker_id: u32, messages: &'inputs InterleavedMessageData, worker_ctx: W, cond_src: &'src str, languages: &'inputs Vec<UnitFrequency>, tx: &SyncSender<TaskPacket>) -> Result<(), Box<dyn Error + 'src>>
where
    K: CipherKey,
    W: CipherWorkerContext<K>,
{
    let mut jit_ctx = JITContext::new();
    let mut comp_ctx = jit_ctx.make_compilation_context()?;
    let mut out_freq_dist = OnceCell::<UnitFrequency>::new();
    let mut cond_table = Table::new();
    let out_freq_dist_ptr = &out_freq_dist as *const OnceCell<UnitFrequency>;
    let languages_ptr = languages as *const Vec<UnitFrequency>;
    let codec_ctx_hsi = cond_table.add_hidden_state(ValueType::USize);

    let in_freq_dist_errors: Box<[f64]> = {
        let mut errors = Vec::<f64>::new();
        for language in languages {
            errors.push(language.get_error(
                &UnitFrequency::from_interleaved_message_data(messages)
            ));
        }

        errors.into()
    };

    // SAFETY: all specialization closures only return an unchecked function's
    //         pointer if it can prove the inputs are always in-bounds, and have
    //         correctly mapped parameters

    unsafe { cond_table.add_binding("in".into(), Binding::Function {
        ret_type: ValueType::U8,
        params: [
            // param 0: usize
            ValueType::USize,
            // param 1: usize
            ValueType::USize,
        ].into(),
        fn_spec: Box::new(move |hints| {
            let args = [
                // messages: &InterleavedMessageData
                FnSpecCallArg::from((messages as *const InterleavedMessageData).addr()),
                // m: usize (param 0)
                FnSpecCallArg::MappedArgument { param_idx: 0 },
                // u: usize (param 1)
                FnSpecCallArg::MappedArgument { param_idx: 1 },
            ].into();

            if let [Some(IRConst::Uint { inner: m }), Some(IRConst::Uint { inner: u })] = *hints.consts {
                let m = m as usize;
                let u = u as usize;
                if m < messages.get_message_count() && u < messages.get_unit_count(m) {
                    Ok(FnSpecChoice::Const { value: Value::U8 { inner: messages[(m, u)] } })
                } else {
                    Err("in() call in expression is always out of bounds".into())
                }
            } else {
                Ok(FnSpecChoice::Call { fn_ptr: eval_in as FnPointer, args })
            }
        }),
    })? };

    unsafe { cond_table.add_binding("in_freq_dist_error".into(), Binding::Function {
        ret_type: ValueType::F64,
        params: [
            // param 0: usize
            ValueType::USize,
        ].into(),
        fn_spec: Box::new(move |hints| {
            if let [Some(IRConst::Uint { inner: l })] = *hints.consts {
                let l = l as usize;
                if l < languages.len() {
                    Ok(FnSpecChoice::Const { value: Value::F64 { inner: in_freq_dist_errors[l] } })
                } else {
                    Err("in_freq_dist_error() call in expression is always out of bounds".into())
                }
            } else {
                Ok(FnSpecChoice::Call {
                    fn_ptr: eval_in_freq_dist_error as FnPointer,
                    args: [
                        // in_freq_dist_errors: &Box<[f64]>
                        FnSpecCallArg::from((&in_freq_dist_errors as *const Box<[f64]>).addr()),
                        // l: usize (param 0)
                        FnSpecCallArg::MappedArgument { param_idx: 0 },
                    ].into(),
                })
            }
        }),
    })? };

    unsafe { cond_table.add_binding("out".into(), Binding::Function {
        ret_type: ValueType::U8,
        params: [
            // param 0: usize
            ValueType::USize,
            // param 1: usize
            ValueType::USize,
        ].into(),
        fn_spec: Box::new(move |hints| {
            let args = [
                // codec_ctx: &W::CodecContext<'_, DECRYPT>
                FnSpecCallArg::from_hidden_state(codec_ctx_hsi),
                // m: usize (param 0)
                FnSpecCallArg::MappedArgument { param_idx: 0 },
                // u: usize (param 1)
                FnSpecCallArg::MappedArgument { param_idx: 1 },
            ].into();

            if let [Some(IRConst::Uint { inner: m }), Some(IRConst::Uint { inner: u })] = *hints.consts {
                let m = m as usize;
                if m < messages.get_message_count() && (u as usize) < messages.get_unit_count(m) {
                    Ok(FnSpecChoice::Call { fn_ptr: eval_out_unchecked::<DECRYPT, K, W> as FnPointer, args })
                } else {
                    Err("out() call in expression is always out of bounds".into())
                }
            } else {
                Ok(FnSpecChoice::Call { fn_ptr: eval_out::<DECRYPT, K, W> as FnPointer, args })
            }
        }),
    })? };

    unsafe { cond_table.add_binding("out_freq_dist_error".into(), Binding::Function {
        ret_type: ValueType::F64,
        params: [
            // param 0: usize
            ValueType::USize,
        ].into(),
        fn_spec: Box::new(move |hints| {
            if let [Some(IRConst::Uint { inner: l })] = *hints.consts {
                let l = l as usize;
                if l < languages.len() {
                    Ok(FnSpecChoice::Call {
                        fn_ptr: eval_out_freq_dist_error_specific::<DECRYPT, K, W> as FnPointer,
                        args: [
                            // codec_ctx: &W::CodecContext<'_, DECRYPT>
                            FnSpecCallArg::from_hidden_state(codec_ctx_hsi),
                            // out_freq_dist: &OnceCell<UnitFrequency>
                            FnSpecCallArg::from(out_freq_dist_ptr.addr()),
                            // language: &UnitFrequency
                            FnSpecCallArg::from((&languages[l] as *const UnitFrequency).addr()),
                        ].into(),
                    })
                } else {
                    Err("out_freq_dist_error() call in expression is always out of bounds".into())
                }
            } else {
                Ok(FnSpecChoice::Call {
                    fn_ptr: eval_out_freq_dist_error::<DECRYPT, K, W> as FnPointer,
                    args: [
                        // codec_ctx: &W::CodecContext<'_, DECRYPT>
                        FnSpecCallArg::from_hidden_state(codec_ctx_hsi),
                        // out_freq_dist: &OnceCell<UnitFrequency>
                        FnSpecCallArg::from(out_freq_dist_ptr.addr()),
                        // languages: &Vec<UnitFrequency>
                        FnSpecCallArg::from(languages_ptr.addr()),
                        // l: usize (param 0)
                        FnSpecCallArg::MappedArgument { param_idx: 0 },
                    ].into(),
                })
            }
        }),
    })? };

    let (mut slab, jit_fn) = match comp_ctx.compile_str(&cond_src, &cond_table)? {
        CompiledExpression::Bool { slab, jit_fn } => (slab, jit_fn),
        _ => return Err(PredicateError::BadExpressionType.into()),
    };

    // clone messages to keep them closer in memory with other working values
    let messages = &(*messages).clone();

    worker_ctx.permute_keys_interruptible(|key| {
        // TODO clearing the cache results in a 5% slowdown. hot-eval should
        //      support pure functions, so that it reuses outputs when possible,
        //      otherwise we have to unnecessarily clear a cache and manage our
        //      own lazy cell, even when there's only a single call in the
        //      expression
        out_freq_dist.take(); // clear cache

        let codec_ctx = W::CodecContext::<'_, DECRYPT>::new(messages, key);
        // SAFETY: &codec_ctx is only used during expression evaluation, it's
        //         replaced before every expression evaluation, and codec_ctx
        //         outlives the call
        unsafe { slab.set_ptr_value_unchecked(codec_ctx_hsi, &codec_ctx); }

        // SAFETY: we're assuming that LLVM generated a valid function, that the
        //         slab has valid data, and that hot-eval is not broken (no bad
        //         codegen, sane types, etc...). not a very strong guarantee...
        if unsafe { jit_fn.call() } {
            tx.send(TaskPacket::Match { net_key: key.encode_to_buffer() }).unwrap();
        }
    }, |keys| {
        tx.send(TaskPacket::Progress { keys }).unwrap();
        true
    });

    Ok(())
}

fn main() { main_error_wrap!({
    let args = Args::parse();

    let languages = import_csv_languages(&args.language)?;
    let alphabet = import_csv_alphabet_or_default(&args.alphabet)?;
    let messages_render_map = import_messages(&args.data_path, &alphabet)?;
    let cipher = deserialise_cipher(&args.cipher, args.config.as_deref())?;

    let mut key_dump_file: Option<File> = match &args.key_dump_path {
        Some(path) => {
            let mut file = File::create_new(path)?;
            file.write(KeyDumpMeta {
                build_hash: String::from(env!("GIT_HASH")),
                cipher_name: args.cipher.clone().into(),
                cipher_config: args.config.clone().map(|x| x.into_string()),
            }.encode_to_vec().as_slice())?;

            Some(file)
        },
        None => None,
    };

    let decrypt = !args.encrypt;
    let worker_total = if args.sequential {
        1u32
    } else {
        let mut max_parallelism: u32 = args.max_parallelism.unwrap_or(NonZeroU32::new(u32::MAX).unwrap()).into();
        max_parallelism = max_parallelism.min(cipher.get_max_parallelism());
        get_parallelism().min(max_parallelism)
    };

    let (tx, rx) = sync_channel::<TaskPacket>(64);
    let messages = AcceleratedMessageList::from_messages(messages_render_map.get_messages());

    std::thread::scope(|scope| -> UnitResult {
        let mut keys_total = Integer::new();
        let mut worker_ctxs = Vec::new();

        for worker_id in 0..worker_total {
            let worker_ctx = cipher.create_worker_context_parallel(worker_id, worker_total);
            keys_total += worker_ctx.get_total_keys();
            worker_ctxs.push(worker_ctx);
        }

        preamble(&messages_render_map, &alphabet, worker_total, &keys_total, decrypt);

        let start_time = Instant::now();

        let mut worker_id = 0;
        for worker_ctx in worker_ctxs {
            let worker_id_clone = worker_id.clone();
            let messages = &messages.data;
            let cond_src = &args.condition;
            let languages = &languages;
            let tx = tx.clone();

            scope.spawn(move || {
                let task_res = if decrypt {
                    search_task::<true, _, _>(worker_id_clone, messages, worker_ctx, cond_src, languages, &tx)
                } else {
                    search_task::<false, _, _>(worker_id_clone, messages, worker_ctx, cond_src, languages, &tx)
                };

                match task_res {
                    Ok(_) => tx.send(TaskPacket::Finished { worker_id }).unwrap(),
                    Err(err) => tx.send(TaskPacket::Error { message: err.to_string().into_boxed_str() }).unwrap(),
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
                                    file.write(net_key.iter().as_slice())?;
                                },
                                None => {
                                    println!("Matched key {}", cipher.net_key_to_boxed_str(&net_key)?);
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
    })?;
}) }
