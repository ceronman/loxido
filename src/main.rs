use std::env;

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
                },

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

fn interpret(chunk: Chunk) -> InterpreterResult {
    let mut vm = Vm::new(chunk);
    return vm.run();
}

fn main() {
    let args: Vec<String> = env::args().map(|s| format!("\"{}\"", s)).collect();
    println!("{}", args.join(", "));

    let mut chunk = Chunk::new();
    let index = chunk.add_constant(1.2);
    chunk.write(Instruction::Constant(index), 124);
    let index = chunk.add_constant(3.4);
    chunk.write(Instruction::Constant(index), 124);
    chunk.write(Instruction::Add, 124);
    let index = chunk.add_constant(5.6);
    chunk.write(Instruction::Constant(index), 124);
    chunk.write(Instruction::Divide, 124);
    chunk.write(Instruction::Negate, 124);
    chunk.write(Instruction::Return, 123);
    chunk.disassemble("test chunk");
    interpret(chunk);
}
