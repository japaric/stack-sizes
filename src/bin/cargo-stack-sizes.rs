use std::{
    env, fs,
    process::{self, Command},
    time::SystemTime,
};

use anyhow::bail;
use cargo_project::{Artifact, Profile, Project};
use clap::{App, AppSettings, Arg, ArgMatches};
use filetime::FileTime;
use walkdir::WalkDir;

const ABOUT: &str = "Builds a Cargo project and prints the stack usage of each function";

const AFTER_HELP: &str = "\
This command behaves very much like `cargo rustc`. All the arguments *after* the `--` will be
 passed to the *top* `rustc` invocation.";

fn main() {
    let matches = App::new("cargo-stack-sizes")
        .about(ABOUT)
        .version(env!("CARGO_PKG_VERSION"))
        .setting(AppSettings::TrailingVarArg)
        .setting(AppSettings::DontCollapseArgsInUsage)
        // as this is used as a Cargo subcommand the first argument will be the name of the binary
        // we ignore this argument
        .arg(Arg::with_name("binary-name").hidden(true))
        .arg(
            Arg::with_name("target")
                .long("target")
                .takes_value(true)
                .value_name("TRIPLE")
                .help("Target triple for which the code is compiled"),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Use verbose output"),
        )
        .arg(Arg::with_name("--").short("-").hidden_short_help(true))
        .arg(Arg::with_name("args").multiple(true))
        .after_help(AFTER_HELP)
        .arg(
            Arg::with_name("bin")
                .long("bin")
                .takes_value(true)
                .value_name("NAME")
                .help("Build only the specified binary"),
        )
        .arg(
            Arg::with_name("example")
                .long("example")
                .takes_value(true)
                .value_name("NAME")
                .help("Build only the specified example"),
        )
        .arg(
            Arg::with_name("release")
                .long("release")
                .help("Build artifacts in release mode, with optimizations"),
        )
        .get_matches();

    match run(&matches) {
        Ok(ec) => process::exit(ec),
        Err(e) => eprintln!("error: {}", e),
    }
}

fn run(matches: &ArgMatches) -> anyhow::Result<i32> {
    let mut is_binary = false;
    let (krate, artifact) = if let Some(bin) = matches.value_of("bin") {
        is_binary = true;
        (bin, Artifact::Bin(bin))
    } else if let Some(example) = matches.value_of("example") {
        (example, Artifact::Example(example))
    } else {
        bail!("One of `--bin` or `--example` must be specified")
    };

    let profile = if matches.is_present("release") {
        Profile::Release
    } else {
        Profile::Dev
    };

    let target = matches.value_of("target");

    let verbose = matches.is_present("verbose");

    let cwd = env::current_dir()?;
    let project = Project::query(&cwd)?;

    let mut cargo = Command::new("cargo");
    cargo.arg("rustc");

    if let Some(target) = target {
        cargo.args(&["--target", target]);
    }

    match artifact {
        Artifact::Bin(bin) => {
            cargo.args(&["--bin", bin]);
        }
        Artifact::Example(example) => {
            cargo.args(&["--example", example]);
        }
        _ => unreachable!(),
    }

    if profile.is_release() {
        cargo.arg("--release");
    }

    cargo.args(&["--", "-C", "lto", "--emit=obj", "-Z", "emit-stack-sizes"]);
    if let Some(arg) = matches.value_of("--") {
        cargo.arg(arg);
    }

    if let Some(args) = matches.values_of("args") {
        cargo.args(args);
    }

    // "touch" some source file to trigger a rebuild
    let root = project.toml().parent().expect("UNREACHABLE");
    let now = FileTime::from_system_time(SystemTime::now());
    if !filetime::set_file_times(root.join("src/main.rs"), now, now).is_ok() {
        if !filetime::set_file_times(root.join("src/lib.rs"), now, now).is_ok() {
            // look for some rust source file and "touch" it
            let src = root.join("src");
            let haystack = if src.exists() { &src } else { root };

            for entry in WalkDir::new(haystack) {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                    filetime::set_file_times(path, now, now)?;
                    break;
                }
            }
        }
    }

    if verbose {
        eprintln!("{:?}", cargo);
    }

    let status = cargo.status()?;

    if !status.success() {
        return Ok(status.code().unwrap_or(101));
    }

    let meta = rustc_version::version_meta()?;
    let host = meta.host;
    let path = project.path(artifact, profile, target, &host)?;

    // find the object file
    let mut obj = None;
    // Most Recently Modified
    let mut mrm = SystemTime::UNIX_EPOCH;
    let prefix = format!("{}-", krate.replace('-', "_"));

    let mut dir = path.parent().expect("unreachable").to_path_buf();

    if is_binary {
        dir = dir.join("deps"); // the .ll file is placed in ../deps
    }

    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();

        if p.extension().map(|e| e == "o").unwrap_or(false) {
            if p.file_stem()
                .expect("unreachable")
                .to_str()
                .expect("unreachable")
                .starts_with(&prefix)
            {
                let modified = e.metadata()?.modified()?;
                if obj.is_none() {
                    obj = Some(p);
                    mrm = modified;
                } else {
                    if modified > mrm {
                        obj = Some(p);
                        mrm = modified;
                    }
                }
            }
        }
    }

    stack_sizes::run_exec(&path, &obj.expect("unreachable"))?;

    Ok(0)
}
