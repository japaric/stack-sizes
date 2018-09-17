extern crate cargo_project;
extern crate clap;
#[macro_use]
extern crate failure;
extern crate stack_sizes;

use std::{
    env,
    process::{self, Command},
};

use cargo_project::{Artifact, Profile, Project};
use clap::{App, AppSettings, Arg, ArgMatches};

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
        ).arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Use verbose output"),
        ).arg(Arg::with_name("--").short("-").hidden_short_help(true))
        .arg(Arg::with_name("args").multiple(true))
        .after_help(AFTER_HELP)
        .arg(
            Arg::with_name("bin")
                .long("bin")
                .takes_value(true)
                .value_name("NAME")
                .help("Build only the specified binary"),
        ).arg(
            Arg::with_name("example")
                .long("example")
                .takes_value(true)
                .value_name("NAME")
                .help("Build only the specified example"),
        ).arg(
            Arg::with_name("release")
                .long("release")
                .help("Build artifacts in release mode, with optimizations"),
        ).get_matches();

    match run(&matches) {
        Ok(ec) => process::exit(ec),
        Err(e) => eprintln!("error: {}", e),
    }
}

fn run(matches: &ArgMatches) -> Result<i32, failure::Error> {
    let artifact = if let Some(bin) = matches.value_of("bin") {
        Artifact::Bin(bin)
    } else if let Some(example) = matches.value_of("example") {
        Artifact::Example(example)
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

    cargo.arg("--");
    if let Some(arg) = matches.value_of("--") {
        cargo.arg(arg);
    }

    if let Some(args) = matches.values_of("args") {
        cargo.args(args);
    }

    cargo.env("RUSTC", "stack-sizes-rustc");

    if verbose {
        eprintln!("RUSTC=stack-sizes-rustc {:?}", cargo);
    }

    let status = cargo.status()?;

    if !status.success() {
        return Ok(status.code().unwrap_or(101));
    }

    let path = project.path(artifact, profile, target);

    if verbose {
        eprintln!("\"stack-sizes\" \"{}\"", path.display());
    }

    stack_sizes::run(path)?;

    Ok(0)
}
