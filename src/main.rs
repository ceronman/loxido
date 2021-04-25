mod chunk;
mod compiler;
mod error;
mod gc;
mod objects;
mod scanner;
mod vm;

use error::LoxError;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use vm::Vm;

fn repl(vm: &mut Vm) {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .expect("Unable to read line from the REPL");
        if line.is_empty() {
            break;
        }
        vm.interpret(&line).ok();
    }
}

fn run_file(vm: &mut Vm, path: &str) {
    let code = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprint!("Unable to read file {}: {}", path, error);
            process::exit(74);
        }
    };
    if let Err(error) = vm.interpret(&code) {
        match error {
            LoxError::CompileError => process::exit(65),
            LoxError::RuntimeError => process::exit(70),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut vm = Vm::new();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => process::exit(64),
    }
}
