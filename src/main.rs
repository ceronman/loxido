mod chunk;
mod compiler;
mod error;
mod function;
mod scanner;
mod strings;
mod vm;

use chunk::{Instruction, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use vm::Vm;

#[macro_use]
extern crate lazy_static;

fn repl(vm: &mut Vm) {
    let mut state = vm.new_state();
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
        vm.interpret(&line, &mut state).ok();
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
    let mut state = vm.new_state();
    match vm.interpret(&code, &mut state) {
        Ok(_) => process::exit(65),
        _ => process::exit(70),
    };
}

fn main() {
    println!("value size:       {}", std::mem::size_of::<Value>());
    println!("instruction size: {}", std::mem::size_of::<Instruction>());
    let args: Vec<String> = env::args().collect();
    let mut vm = Vm::default();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&mut vm, &args[1]),
        _ => process::exit(64),
    }
}
