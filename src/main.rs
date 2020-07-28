use std::collections::HashMap;
use std::convert::TryFrom;
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
    Constant(u8),
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
        self.constants[index as usize]
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

#[derive(PartialEq, Debug, Copy, Clone)]
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

#[derive(Copy, Clone)]
struct Token<'a> {
    kind: TokenType,
    line: usize,
    lexeme: &'a str,
}

struct Scanner<'a> {
    keywords: HashMap<&'static str, TokenType>,
    code: &'a str,
    start: usize,
    current: usize,
    line: usize,
}

impl<'a> Scanner<'a> {
    fn new(code: &'a str) -> Scanner {
        // TODO: consider making this a lazy static
        let mut keywords = HashMap::with_capacity(16);
        keywords.insert("and", TokenType::And);
        keywords.insert("class", TokenType::Class);
        keywords.insert("else", TokenType::Else);
        keywords.insert("false", TokenType::False);
        keywords.insert("for", TokenType::For);
        keywords.insert("fun", TokenType::Fun);
        keywords.insert("if", TokenType::If);
        keywords.insert("nil", TokenType::Nil);
        keywords.insert("or", TokenType::Or);
        keywords.insert("print", TokenType::Print);
        keywords.insert("return", TokenType::Return);
        keywords.insert("super", TokenType::Super);
        keywords.insert("this", TokenType::This);
        keywords.insert("true", TokenType::True);
        keywords.insert("var", TokenType::Var);
        keywords.insert("while", TokenType::While);

        Scanner {
            keywords,
            code,
            start: 0,
            current: 0,
            line: 1,
        }
    }

    fn scan_token(&mut self) -> Token<'a> {
        self.skip_whitespace();
        self.start = self.current;
        if self.is_at_end() {
            return self.make_token(TokenType::Eof);
        }

        match self.advance() {
            b'(' => self.make_token(TokenType::LeftParen),
            b')' => self.make_token(TokenType::RightParen),
            b'{' => self.make_token(TokenType::LeftBrace),
            b'}' => self.make_token(TokenType::RightBrace),
            b';' => self.make_token(TokenType::Semicolon),
            b',' => self.make_token(TokenType::Comma),
            b'.' => self.make_token(TokenType::Dot),
            b'-' => self.make_token(TokenType::Minus),
            b'+' => self.make_token(TokenType::Plus),
            b'/' => self.make_token(TokenType::Slash),
            b'*' => self.make_token(TokenType::Star),
            b'!' if self.matches(b'=') => self.make_token(TokenType::BangEqual),
            b'!' => self.make_token(TokenType::Bang),
            b'=' if self.matches(b'=') => self.make_token(TokenType::EqualEqual),
            b'=' => self.make_token(TokenType::Equal),
            b'<' if self.matches(b'=') => self.make_token(TokenType::LessEqual),
            b'<' => self.make_token(TokenType::Less),
            b'>' if self.matches(b'=') => self.make_token(TokenType::GreaterEqual),
            b'>' => self.make_token(TokenType::Greater),
            b'"' => self.string(),
            c if is_digit(c) => self.number(),
            c if is_alpha(c) => self.identifier(),
            _ => self.error_token("Unexpected character."),
        }
    }

    fn is_at_end(&self) -> bool {
        return self.current == self.code.len() - 1;
    }

    fn lexeme(&self) -> &'a str {
        &self.code[self.start..self.current]
    }

    fn make_token(&self, kind: TokenType) -> Token<'a> {
        Token {
            kind,
            lexeme: self.lexeme(),
            line: self.line,
        }
    }

    fn peek(&self) -> u8 {
        self.code.as_bytes()[self.current]
    }

    fn peek_next(&self) -> u8 {
        if self.is_at_end() {
            b'\0'
        } else {
            self.code.as_bytes()[self.current + 1]
        }
    }

    fn error_token(&self, message: &'static str) -> Token<'static> {
        Token {
            kind: TokenType::Error,
            lexeme: message,
            line: self.line,
        }
    }

    fn advance(&mut self) -> u8 {
        let char = self.peek();
        self.current += 1;
        char
    }

    fn matches(&mut self, expected: u8) -> bool {
        if self.is_at_end() {
            false
        } else if self.peek() != expected {
            false
        } else {
            self.current += 1;
            true
        }
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                b' ' | b'\r' | b'\t' => {
                    self.advance();
                }
                b'\n' => {
                    self.line += 1;
                    self.advance();
                }
                b'/' if self.peek_next() == b'/' => {
                    while self.peek() != b'\n' && !self.is_at_end() {
                        self.advance();
                    }
                }
                _ => return,
            }
        }
    }

    fn string(&mut self) -> Token<'a> {
        while self.peek() != b'"' && !self.is_at_end() {
            self.advance();
        }

        if self.is_at_end() {
            self.error_token("Unterminated string.")
        } else {
            self.advance();
            self.make_token(TokenType::String)
        }
    }

    fn number(&mut self) -> Token<'a> {
        while is_digit(self.peek()) {
            self.advance();
        }

        if self.peek() == b'.' && is_digit(self.peek_next()) {
            self.advance();
            while is_digit(self.peek()) {
                self.advance();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn identifier(&mut self) -> Token<'a> {
        while is_alpha(self.peek()) || is_digit(self.peek()) {
            self.advance();
        }
        self.make_token(self.identifier_type())
    }

    fn identifier_type(&self) -> TokenType {
        self.keywords
            .get(self.lexeme())
            .cloned()
            .unwrap_or(TokenType::Identifier)
    }
}

fn is_digit(c: u8) -> bool {
    c >= b'0' && c <= b'9'
}

fn is_alpha(c: u8) -> bool {
    (c >= b'a' && c <= b'z') || (c >= b'A' && c <= b'Z') || c == b'_'
}

enum Precedence {
    None,
    Assignment,  // =
    Or,          // or
    And,         // and
    Equality,    // == !=
    Comparison,  // < > <= >=
    Term,        // + -
    Factor,      // * /
    Unary,       // ! -
    Call,        // . ()
    Primary
}

impl Precedence {
    fn next(&self) -> Precedence {
        match self {
            Precedence::None =>  Precedence::Assignment,
            Precedence::Assignment =>  Precedence::Or,
            Precedence::Or =>  Precedence::And,
            Precedence::And =>  Precedence::Equality,
            Precedence::Equality =>  Precedence::Comparison,
            Precedence::Comparison =>  Precedence::Term,
            Precedence::Term =>  Precedence::Factor,
            Precedence::Factor =>  Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call =>  Precedence::Primary,
            Precedence::Primary =>  Precedence::None,
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

        Parser {
            scanner: Scanner::new(code),
            chunk: Chunk::new(),
            current: t1,
            previous: t2,
            had_error: false,
            panic_mode: false,
        }
    }

    fn compile(&mut self) -> bool {
        self.advance();
        self.expression();
        self.consume(TokenType::Eof, "Expect end of expression.");
        self.emit(Instruction::Return);
        return self.had_error;
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
        self.emit_constant(value);
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self) {
        let operator = self.previous.kind;
        self.parse_precedence(Precedence::Unary);
        match operator {
            TokenType::Minus => self.emit(Instruction::Negate),
            _ => panic!("Invalid unary operator")
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {

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
            eprint!("at '{}'", token.lexeme);
        }
        eprintln!(": {}", msg);
    }

    fn emit(&mut self, instruction: Instruction) {
        self.chunk.write(instruction, self.previous.line);
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
}

fn interpret(code: &str) -> InterpreterResult {
    let mut parser = Parser::new(code);
    let result = parser.compile();
    if !result {
        return InterpreterResult::CompileError;
    }
    let mut vm = Vm::new(parser.chunk);
    return vm.run();
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
        interpret(&line);
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
