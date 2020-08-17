mod chunk;
mod error;
mod parser;
mod scanner;
mod strings;
mod vm;

use chunk::{Instruction, Value};
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
        if line.len() == 0 {
            break;
        }
        let _ = vm.interpret(&line);
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

    match vm.interpret(&code) {
        Ok(_) => process::exit(65),
        _ => process::exit(70),
    };
}

fn main() {
    println!("value size:       {}", std::mem::size_of::<Value>());
    println!("instruction size: {}", std::mem::size_of::<Instruction>());
    let args: Vec<String> = env::args().collect();
    let mut vm = Vm::new();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => process::exit(64),
    }
}
