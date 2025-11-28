use std::io::Write;
use std::fs::File;
use std::path::PathBuf;
use std::io::Result;
use std::process::Command;

fn main() -> Result<()> {
    // build .proto files
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/proto");
    let mut proto_files = Vec::<PathBuf>::new();
    let mut proto_bare_names = Vec::<String>::new();

    for entry in root.read_dir()? {
        if let Ok(entry) = entry {
            if entry.metadata()?.is_file() {
                let file_path = root.join(entry.file_name());
                if let Some(ext) = file_path.extension() {
                    if ext.eq_ignore_ascii_case("proto") {
                        let bare_name = String::from(entry.path().with_extension("")
                            .file_name().expect("couldn't extract bare file name")
                            .to_str().expect("couldn't convert &OsStr to &str"));

                        proto_files.push(file_path);
                        proto_bare_names.push(bare_name);
                    }
                }
            }
        }
    }

    prost_build::compile_protos(proto_files.iter().as_slice(), &[root.to_str().unwrap()])?;

    let mut mod_file = File::create(root.join("mod.rs"))?;
    writeln!(&mut mod_file, "// WARNING: AUTO-GENERATED FILE\n// Do not put anything important here; it WILL be overridden")?;

    for bare_name in proto_bare_names {
        writeln!(&mut mod_file, r#"
pub mod {bare_name} {{
    include!(concat!(env!("OUT_DIR"), "/noita_eye_messages.proto.{bare_name}.rs"));
}}"#    )?;
    }

    // custom compile-time env vars
    let output = Command::new("git").args(&["rev-parse", "HEAD"]).output().unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    Ok(())
}