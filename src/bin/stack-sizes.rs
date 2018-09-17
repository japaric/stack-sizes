extern crate clap;
extern crate failure;
extern crate stack_sizes;

use clap::{App, Arg};

const ABOUT: &str = "\
Prints the stack usage of each function in an ELF file.";

fn main() {
    let matches = App::new("stack-sizes")
        .about(ABOUT)
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("ELF")
                .help("ELF file to analyze")
                .required(true)
                .index(1),
        ).get_matches();

    let path = matches.value_of("ELF").unwrap();

    if let Err(e) = stack_sizes::run(&path) {
        eprintln!("error: {}", e);
    }
}
