extern crate failure;

use std::{
    env,
    process::{self, Command},
};

fn main() -> Result<(), failure::Error> {
    // NOTE(skip) the first argument is the name of the command, i.e. `rustc`
    let mut args: Vec<_> = env::args().skip(1).collect();

    args.push("-Z".to_owned());
    args.push("emit-stack-sizes".to_owned());

    let status = Command::new("rustc").args(args).status()?;

    if !status.success() {
        process::exit(101)
    }

    Ok(())
}
