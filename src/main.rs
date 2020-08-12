mod chunk;
mod error;
mod parser;
mod scanner;
mod vm;

use error::LoxError;
use parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use vm::Vm;

fn interpret(code: &str) -> Result<(), LoxError> {
    let mut parser = Parser::new(code);
    parser.compile()?;
    let mut vm = Vm::new(parser.chunk); // TODO: Improve how chunk is accessed from parser
    vm.run()
}

fn repl() {
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
        let _ = interpret(&line);
    }
}

fn run_file(path: &str) {
    let code = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprint!("Unable to read file {}: {}", path, error);
            process::exit(74);
        }
    };

    match interpret(&code) {
        Ok(_) => process::exit(65),
        _ => process::exit(70),
    };
}

fn main() {
    println!("{}", std::mem::size_of::<String>());
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => repl(),
        2 => run_file(&args[1]),
        _ => process::exit(64),
    }
}
