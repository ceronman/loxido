use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

type Value = f64;

// TODO: Investigate how to completely remove this at compile time.
const DEBUG: bool = true;

#[derive(Debug, Copy, Clone)]
enum Instruction {
    Add,
    Constant(usize),
    Divide,
    Multiply,
    Negate,
    Return,
    Substract,
}

struct Chunk {
    code: Vec<Instruction>,
    constants: Vec<Value>,
    lines: Vec<usize>,
}

impl Chunk {
    fn new() -> Chunk {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    fn write(&mut self, instruction: Instruction, line: usize) {
        self.code.push(instruction);
        self.lines.push(line);
    }

    fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    fn read_constant(&self, index: usize) -> Value {
        self.constants[index]
    }

    fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, instruction) in self.code.iter().enumerate() {
            self.disassemble_instruction(instruction, offset);
        }
    }

    fn disassemble_instruction(&self, instruction: &Instruction, offset: usize) {
        print!("{:04} ", offset);
        let line = self.lines[offset];
        if offset > 0 && line == self.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Add => println!("OP_ADD"),
            Instruction::Divide => println!("OP_DIVIDE"),
            Instruction::Multiply => println!("OP_MULTIPLY"),
            Instruction::Negate => println!("OP_NEGATE"),
            Instruction::Substract => println!("OP_SUBSTRACT"),
            Instruction::Return => println!("OP_RETURN"),
            Instruction::Constant(index) => {
                let i = *index;
                let i = i as usize;
                let value = self.constants[i];
                println!("{:<16} {:4} {}", "OP_CONSTANT", index, value);
            }
        }
    }
}

// TODO: Maybe just use standard Result?
enum InterpreterResult {
    Ok,
    CompileError,
    RuntimeError,
}

struct Vm {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
}

impl Vm {
    fn new(chunk: Chunk) -> Vm {
        Vm {
            chunk,
            ip: 0,
            stack: Vec::with_capacity(256),
        }
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("Empty stack")
    }

    fn binary_op(&mut self, f: fn(Value, Value) -> Value) {
        let a = self.pop();
        let b = self.pop();
        self.push(f(a, b));
    }

    fn run(&mut self) -> InterpreterResult {
        loop {
            let instruction = self.next_instruction();
            for value in self.stack.iter() {
                println!("[ {} ]", value)
            }
            if DEBUG {
                self.chunk
                    .disassemble_instruction(&instruction, self.ip - 1);
            }
            match instruction {
                Instruction::Constant(index) => {
                    let value = self.chunk.read_constant(index);
                    self.stack.push(value);
                }

                Instruction::Negate => {
                    let value = self.pop();
                    self.push(-value);
                }

                Instruction::Return => {
                    println!("{}", self.stack.pop().expect("emtpy stack!"));
                    return InterpreterResult::Ok;
                }

                Instruction::Add => self.binary_op(|a, b| a + b),
                Instruction::Divide => self.binary_op(|a, b| a / b),
                Instruction::Multiply => self.binary_op(|a, b| a * b),
                Instruction::Substract => self.binary_op(|a, b| a - b),
            }
        }
    }

    fn next_instruction(&mut self) -> Instruction {
        let instruction = self.chunk.code[self.ip];
        self.ip += 1;
        instruction
    }
}

enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    Error,
    Eof,
}

struct Token<'a> {
    kind: TokenType,
    line: usize,
    lexeme: &'a str
}

struct Scanner<'a> {
    code: &'a str,
    start: usize,
    current: usize,
    line: usize,
}

impl<'a> Scanner<'a> {
    fn new(code: &'a str) -> Scanner {
        Scanner {
            code,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    fn scan_token(&mut self) -> Token {
        self.start = self.current;
        if self.is_at_end() {
            self.make_token(TokenType::Eof)
        } else {
            self.error_token("Unexpected character.")
        }
    }

    fn is_at_end(&self) -> bool {
        return self.current >= self.code.len();
    }

    fn make_token(&self, kind: TokenType) -> Token {
        Token {
            kind,
            lexeme: &self.code[self.start..self.current],
            line: self.line,
        }
    }

    fn error_token(&self, message: &'static str) -> Token {
        Token {
            kind: TokenType::Error,
            lexeme: message,
            line: self.line,
        }
    }
}

fn interpret(chunk: Chunk) -> InterpreterResult {
    let mut vm = Vm::new(chunk);
    return vm.run();
}

fn compile(code: &str) {}

// TODO: Actually called interpret, but we have another function with that name for now.
fn run_code(code: &str) -> InterpreterResult {
    compile(code);
    return InterpreterResult::Ok;
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
        run_code(&line);
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

    match run_code(&code) {
        InterpreterResult::Ok => process::exit(65),
        _ => process::exit(70),
    };
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => repl(),
        2 => run_file(&args[1]),
        _ => process::exit(64),
    }
}
