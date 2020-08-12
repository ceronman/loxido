mod scanner;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::process;
use scanner::{Scanner, TokenType, Token};

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
}

impl Value {
    fn is_falsy(&self) -> bool {
        match self {
            Value::Nil | Value::Number(_) | Value::String(_) => true,
            Value::Bool(value) => !value,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Number(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Instruction {
    Add,
    Constant(u8),
    Divide,
    Equal,
    False,
    Greater,
    Less,
    Multiply,
    Negate,
    Nil,
    Not,
    Return,
    Substract,
    True,
}

struct Chunk {
    code: Vec<Instruction>,
    constants: Vec<Value>,
    lines: Vec<usize>,
}

impl Chunk {
    fn new() -> Chunk {
        // TODO: use from capacity!
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

    fn read_constant(&self, index: u8) -> Value {
        self.constants[index as usize].clone()
    }

    #[cfg(debug_assertions)]
    fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, instruction) in self.code.iter().enumerate() {
            self.disassemble_instruction(instruction, offset);
        }
    }

    #[cfg(debug_assertions)]
    fn disassemble_instruction(&self, instruction: &Instruction, offset: usize) {
        print!("{:04} ", offset);
        let line = self.lines[offset];
        if offset > 0 && line == self.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Constant(index) => {
                let i = *index;
                let i = i as usize;
                let value = self.constants[i].clone();
                println!("{:<16} {:4} {}", "OP_CONSTANT", index, value);
            }
            Instruction::Add => println!("OP_ADD"),
            Instruction::Divide => println!("OP_DIVIDE"),
            Instruction::Equal => println!("OP_EQUAL"),
            Instruction::False => println!("OP_FALSE"),
            Instruction::Greater => println!("OP_GREATER"),
            Instruction::Less => println!("OP_LESS"),
            Instruction::Multiply => println!("OP_MULTIPLY"),
            Instruction::Negate => println!("OP_NEGATE"),
            Instruction::Not => println!("OP_NOT"),
            Instruction::Nil => println!("OP_NIL"),
            Instruction::Return => println!("OP_RETURN"),
            Instruction::Substract => println!("OP_SUBSTRACT"),
            Instruction::True => println!("OP_TRUE"),
        }
    }
}

enum LoxError {
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

    // TODO: Is peek necessary at all? Consider removing it.
    fn peek(&self, n: usize) -> Value {
        let size = self.stack.len();
        self.stack[size - 1 - n].clone()
    }

    // TODO: Investigate macros for this
    fn binary_op<T>(&mut self, f: fn(f64, f64) -> T, r: fn(T) -> Value) -> Result<(), LoxError> {
        let operands = (self.pop(), self.pop());
        match operands {
            (Value::Number(value_b), Value::Number(value_a)) => {
                self.push(r(f(value_a, value_b)));
                Ok(())
            }
            _ => {
                self.runtime_error("Operands must be numbers.");
                Err(LoxError::RuntimeError)
            }
        }
    }

    fn run(&mut self) -> Result<(), LoxError> {
        loop {
            let instruction = self.next_instruction();

            #[cfg(debug_assertions)]
            {
                for value in self.stack.iter() {
                    print!("[{}]", value);
                }
                println!("");

                #[cfg(debug_assertions)]
                self.chunk
                    .disassemble_instruction(&instruction, self.ip - 1);
            }

            match instruction {
                Instruction::Add => {
                    let (b, a) = (self.pop(), self.pop());
                    match (&a, &b) {
                        (Value::Number(value_a), Value::Number(value_b)) => {
                            self.push(Value::Number(value_a + value_b));
                        }

                        (Value::String(value_a), Value::String(value_b)) => {
                            let result = Value::String(format!("{}{}", value_a, value_b));
                            self.push(result);
                        }

                        _ => {
                            self.push(a);
                            self.push(b);
                            self.runtime_error("Operands must be numbers.");
                            return Err(LoxError::RuntimeError);
                        }
                    }
                }
                Instruction::Constant(index) => {
                    let value = self.chunk.read_constant(index);
                    self.stack.push(value);
                }
                Instruction::Divide => self.binary_op(|a, b| a / b, |n| Value::Number(n))?,
                Instruction::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::Bool(a == b));
                }
                Instruction::False => self.push(Value::Bool(false)),
                Instruction::Greater => self.binary_op(|a, b| a > b, |n| Value::Bool(n))?,
                Instruction::Less => self.binary_op(|a, b| a < b, |n| Value::Bool(n))?,
                Instruction::Multiply => self.binary_op(|a, b| a * b, |n| Value::Number(n))?,
                Instruction::Negate => {
                    if let Value::Number(value) = self.peek(0) {
                        self.pop();
                        self.push(Value::Number(-value));
                    } else {
                        self.runtime_error("Operand must be a number.");
                        return Err(LoxError::RuntimeError);
                    }
                }
                Instruction::Nil => self.push(Value::Nil),
                Instruction::Not => {
                    let value = self.pop();
                    self.push(Value::Bool(value.is_falsy()));
                }
                Instruction::Return => {
                    println!("{}", self.stack.pop().expect("emtpy stack!"));
                    return Ok(());
                }
                Instruction::Substract => self.binary_op(|a, b| a - b, |n| Value::Number(n))?,
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }

    fn next_instruction(&mut self) -> Instruction {
        let instruction = self.chunk.code[self.ip];
        self.ip += 1;
        instruction
    }

    fn runtime_error(&mut self, msg: &str) {
        eprintln!("{}", msg);
        let line = self.chunk.lines[self.ip - 1];
        eprintln!("[line {}] in script", line);
    }
}

#[derive(Copy, Clone, Debug)]
enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl Precedence {
    fn next(&self) -> Precedence {
        match self {
            Precedence::None => Precedence::Assignment,
            Precedence::Assignment => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Equality,
            Precedence::Equality => Precedence::Comparison,
            Precedence::Comparison => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::None,
        }
    }
}

type ParseFn<'a> = fn(&mut Parser<'a>) -> ();

#[derive(Copy, Clone)]
struct ParseRule<'a> {
    prefix: Option<ParseFn<'a>>,
    infix: Option<ParseFn<'a>>,
    precedence: Precedence,
}

impl<'a> ParseRule<'a> {
    fn new(
        prefix: Option<ParseFn<'a>>,
        infix: Option<ParseFn<'a>>,
        precedence: Precedence,
    ) -> ParseRule<'a> {
        ParseRule {
            prefix,
            infix,
            precedence,
        }
    }
}

struct Parser<'a> {
    scanner: Scanner<'a>,
    chunk: Chunk,
    current: Token<'a>,
    previous: Token<'a>,
    had_error: bool,
    panic_mode: bool,
    rules: HashMap<TokenType, ParseRule<'a>>,
}

impl<'a> Parser<'a> {
    fn new(code: &'a str) -> Parser<'a> {
        let t1 = Token {
            kind: TokenType::Eof,
            lexeme: "",
            line: 1,
        };

        let t2 = Token {
            kind: TokenType::Eof,
            lexeme: "",
            line: 1,
        };

        let mut rules = HashMap::new();

        let mut rule = |kind, prefix, infix, precedence| {
            rules.insert(kind, ParseRule::new(prefix, infix, precedence));
        };

        rule(
            TokenType::LeftParen,
            Some(Parser::grouping),
            None,
            Precedence::None,
        );
        rule(TokenType::RightParen, None, None, Precedence::None);
        rule(TokenType::LeftBrace, None, None, Precedence::None);
        rule(TokenType::RightBrace, None, None, Precedence::None);
        rule(TokenType::Comma, None, None, Precedence::None);
        rule(TokenType::Dot, None, None, Precedence::None);
        rule(
            TokenType::Minus,
            Some(Parser::unary),
            Some(Parser::binary),
            Precedence::Term,
        );
        rule(
            TokenType::Plus,
            None,
            Some(Parser::binary),
            Precedence::Term,
        );
        rule(TokenType::Semicolon, None, None, Precedence::None);
        rule(
            TokenType::Slash,
            None,
            Some(Parser::binary),
            Precedence::Factor,
        );
        rule(
            TokenType::Star,
            None,
            Some(Parser::binary),
            Precedence::Factor,
        );
        rule(TokenType::Bang, Some(Parser::unary), None, Precedence::None);
        rule(
            TokenType::BangEqual,
            None,
            Some(Parser::binary),
            Precedence::Equality,
        );
        rule(TokenType::Equal, None, None, Precedence::None);
        rule(
            TokenType::EqualEqual,
            None,
            Some(Parser::binary),
            Precedence::Equality,
        );
        rule(
            TokenType::Greater,
            None,
            Some(Parser::binary),
            Precedence::Comparison,
        );
        rule(
            TokenType::GreaterEqual,
            None,
            Some(Parser::binary),
            Precedence::Comparison,
        );
        rule(
            TokenType::Less,
            None,
            Some(Parser::binary),
            Precedence::Comparison,
        );
        rule(
            TokenType::LessEqual,
            None,
            Some(Parser::binary),
            Precedence::Comparison,
        );
        rule(TokenType::Identifier, None, None, Precedence::None);
        rule(
            TokenType::String,
            Some(Parser::string),
            None,
            Precedence::None,
        );
        rule(
            TokenType::Number,
            Some(Parser::number),
            None,
            Precedence::None,
        );
        rule(TokenType::And, None, None, Precedence::None);
        rule(TokenType::Class, None, None, Precedence::None);
        rule(TokenType::Else, None, None, Precedence::None);
        rule(
            TokenType::False,
            Some(Parser::literal),
            None,
            Precedence::None,
        );
        rule(TokenType::For, None, None, Precedence::None);
        rule(TokenType::Fun, None, None, Precedence::None);
        rule(TokenType::If, None, None, Precedence::None);
        rule(
            TokenType::Nil,
            Some(Parser::literal),
            None,
            Precedence::None,
        );
        rule(TokenType::Or, None, None, Precedence::None);
        rule(TokenType::Print, None, None, Precedence::None);
        rule(TokenType::Return, None, None, Precedence::None);
        rule(TokenType::Super, None, None, Precedence::None);
        rule(TokenType::This, None, None, Precedence::None);
        rule(
            TokenType::True,
            Some(Parser::literal),
            None,
            Precedence::None,
        );
        rule(TokenType::Var, None, None, Precedence::None);
        rule(TokenType::While, None, None, Precedence::None);
        rule(TokenType::Error, None, None, Precedence::None);
        rule(TokenType::Eof, None, None, Precedence::None);

        Parser {
            scanner: Scanner::new(code),
            chunk: Chunk::new(),
            current: t1,
            previous: t2,
            had_error: false,
            panic_mode: false,
            rules,
        }
    }

    fn compile(&mut self) -> Result<(), LoxError> {
        self.advance();
        self.expression();
        self.consume(TokenType::Eof, "Expect end of expression.");
        self.emit(Instruction::Return);

        #[cfg(debug_assertions)]
        if !self.had_error {
            self.chunk.disassemble("code");
        }

        if self.had_error {
            Err(LoxError::CompileError)
        } else {
            Ok(())
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn number(&mut self) {
        let value: f64 = self
            .previous
            .lexeme
            .parse()
            .expect("Parsed value is not a double");
        self.emit_constant(Value::Number(value));
    }

    fn string(&mut self) {
        let lexeme = self.previous.lexeme;
        let value = &lexeme[1..(lexeme.len() - 1)];
        self.emit_constant(Value::String(String::from(value)));
    }

    fn literal(&mut self) {
        match self.previous.kind {
            TokenType::False => self.emit(Instruction::False),
            TokenType::True => self.emit(Instruction::True),
            TokenType::Nil => self.emit(Instruction::Nil),
            _ => panic!("Unreachable literal"),
        }
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator = self.previous.kind;
        self.parse_precedence(Precedence::Unary);
        match operator {
            TokenType::Bang => self.emit(Instruction::Not),
            TokenType::Minus => self.emit(Instruction::Negate),
            _ => panic!("Invalid unary operator"),
        }
    }

    fn binary(&mut self) {
        let operator = self.previous.kind;
        let rule = self.get_rule(operator);
        self.parse_precedence(rule.precedence.next());
        match operator {
            TokenType::Plus => self.emit(Instruction::Add),
            TokenType::Minus => self.emit(Instruction::Substract),
            TokenType::Star => self.emit(Instruction::Multiply),
            TokenType::Slash => self.emit(Instruction::Divide),
            TokenType::BangEqual => self.emit_two(Instruction::Equal, Instruction::Not),
            TokenType::EqualEqual => self.emit(Instruction::Equal),
            TokenType::Greater => self.emit(Instruction::Greater),
            TokenType::GreaterEqual => self.emit_two(Instruction::Less, Instruction::Not),
            TokenType::Less => self.emit(Instruction::Less),
            TokenType::LessEqual => self.emit_two(Instruction::Greater, Instruction::Not),

            _ => panic!("Invalid unary operator"),
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        let prefix_rule = self.get_rule(self.previous.kind).prefix;

        // TODO: better alternative for this match?
        let prefix_rule = match prefix_rule {
            Some(rule) => rule,
            None => {
                self.error("Expect expression.");
                return;
            }
        };

        prefix_rule(self);

        while self.is_lower_precedence(precedence) {
            self.advance();
            let infix_rule = self.get_rule(self.previous.kind).infix.unwrap();
            infix_rule(self);
        }
    }

    fn is_lower_precedence(&self, precedence: Precedence) -> bool {
        let current_precedence = self.get_rule(self.current.kind).precedence;
        (precedence as u8) <= (current_precedence as u8)
    }

    fn consume(&mut self, expected: TokenType, msg: &str) {
        if self.current.kind == expected {
            self.advance();
            return;
        }

        self.error_at_current(msg);
    }

    fn advance(&mut self) {
        self.previous = self.current;

        loop {
            self.current = self.scanner.scan_token();
            if self.current.kind == TokenType::Error {
                self.error_at_current(self.current.lexeme);
            } else {
                break;
            }
        }
    }

    fn error_at_current(&mut self, msg: &str) {
        self.error_at(self.current, msg)
    }

    fn error(&mut self, msg: &str) {
        self.error_at(self.previous, msg)
    }

    fn error_at(&mut self, token: Token, msg: &str) {
        if self.panic_mode {
            return;
        }

        self.had_error = true;
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::Eof {
            eprint!(" at end");
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {}", msg);
    }

    fn emit(&mut self, instruction: Instruction) {
        self.chunk.write(instruction, self.previous.line);
    }

    fn emit_two(&mut self, i1: Instruction, i2: Instruction) {
        self.chunk.write(i1, self.previous.line);
        self.chunk.write(i2, self.previous.line);
    }

    fn emit_constant(&mut self, value: Value) {
        let index = self.chunk.add_constant(value);
        let index = match u8::try_from(index) {
            Ok(index) => index,
            Err(_) => {
                self.error("Too many constants in one chunk.");
                0
            }
        };
        self.emit(Instruction::Constant(index));
    }

    fn get_rule(&self, kind: TokenType) -> ParseRule<'a> {
        self.rules.get(&kind).cloned().unwrap()
    }
}

fn interpret(code: &str) -> Result<(), LoxError> {
    let mut parser = Parser::new(code);
    parser.compile()?;
    let mut vm = Vm::new(parser.chunk);
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
