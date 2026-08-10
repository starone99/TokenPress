//! Thin entry point. All logic lives in the library crate (`cli.rs`) so the
//! coverage gate can instrument it; this file is excluded from coverage.
fn main(){std::process::exit(tokenpress_cli::run(std::env::args_os(),&mut std::io::stdout(),&mut std::io::stderr(),));}