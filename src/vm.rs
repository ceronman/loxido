use crate::{
    chunk::{Instruction, Value},
    compiler::Parser,
    error::LoxError,
    function::LoxFunction,
    strings::{LoxString, Strings},
};
use std::collections::HashMap;

struct CallFrame {
    function: LoxFunction,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(function: LoxFunction) -> Self {
        CallFrame {
            function,
            ip: 0,
            slot: 0,
        }
    }
}

const MAX_FRAMES: usize = 64;
const STACK_SIZE: usize = MAX_FRAMES * (std::u8::MAX as usize) + 1;

pub struct Vm {
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<LoxString, Value>,
    strings: Strings,
}

impl Vm {
    pub fn new() -> Vm {
        // TODO: Investigate using #[derive(Default)] to avoid this.
        Vm {
            frames: Vec::with_capacity(MAX_FRAMES),
            stack: Vec::with_capacity(STACK_SIZE),
            globals: HashMap::new(),
            strings: Strings::default(),
        }
    }

    pub fn interpret(&mut self, code: &str) -> Result<(), LoxError> {
        let parser = Parser::new(code, &mut self.strings);
        let function = parser.compile()?;
        self.frames.push(CallFrame::new(function));
        self.run()
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("Empty stack")
    }

    // FIXME: Ensure this is used in all the right places
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
                let frame = match self.frames.last() {
                    Some(f) => f,
                    None => panic!("No frames available"),
                };
                Vm::runtime_error(frame, "Operands must be numbers.");
                Err(LoxError::RuntimeError)
            }
        }
    }

    fn run(&mut self) -> Result<(), LoxError> {
        let mut frame = self.frames.pop().unwrap(); // TODO unwrap
        loop {
            let instruction = frame.function.chunk.code[frame.ip];

            #[cfg(debug_assertions)]
            {
                for value in self.stack.iter() {
                    print!("[{}]", value);
                }
                println!("");

                #[cfg(debug_assertions)]
                frame
                    .function
                    .chunk
                    .disassemble_instruction(&instruction, frame.ip);
            }

            frame.ip += 1;

            match instruction {
                Instruction::Add => {
                    let (b, a) = (self.pop(), self.pop());
                    match (&a, &b) {
                        (Value::Number(value_a), Value::Number(value_b)) => {
                            self.push(Value::Number(value_a + value_b));
                        }

                        (Value::String(value_a), Value::String(value_b)) => {
                            let s_a = self.strings.lookup(*value_a);
                            let s_b = self.strings.lookup(*value_b);
                            let result = format!("{}{}", s_a, s_b);
                            let s = self.strings.intern_onwed(result);
                            let value = Value::String(s);
                            self.push(value);
                        }

                        _ => {
                            self.push(a);
                            self.push(b);
                            Vm::runtime_error(&frame, "Operands must be numbers.");
                            return Err(LoxError::RuntimeError);
                        }
                    }
                }
                Instruction::Constant(index) => {
                    let value = frame.function.chunk.read_constant(index);
                    self.stack.push(value);
                }
                Instruction::DefineGlobal(index) => {
                    let s = frame.function.chunk.read_string(index);
                    let value = self.pop();
                    self.globals.insert(s, value);
                }
                Instruction::Divide => self.binary_op(|a, b| a / b, |n| Value::Number(n))?,
                Instruction::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::Bool(a == b));
                }
                Instruction::False => self.push(Value::Bool(false)),
                Instruction::GetGlobal(index) => {
                    let s = frame.function.chunk.read_string(index);
                    match self.globals.get(&s) {
                        Some(&value) => self.push(value),
                        None => {
                            let name = self.strings.lookup(s);
                            let msg = format!("Undefined variable '{}'.", name);
                            Vm::runtime_error(&frame, &msg);
                            return Err(LoxError::RuntimeError);
                        }
                    }
                }
                Instruction::GetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = self.stack[i];
                    self.push(value);
                }
                Instruction::Greater => self.binary_op(|a, b| a > b, |n| Value::Bool(n))?,
                Instruction::Jump(offset) => {
                    frame.ip += offset as usize;
                }
                Instruction::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsy() {
                        frame.ip += offset as usize;
                    }
                }
                Instruction::Less => self.binary_op(|a, b| a < b, |n| Value::Bool(n))?,
                Instruction::Loop(offset) => {
                    frame.ip -= offset as usize + 1;
                }
                Instruction::Multiply => self.binary_op(|a, b| a * b, |n| Value::Number(n))?,
                Instruction::Negate => {
                    if let Value::Number(value) = self.peek(0) {
                        self.pop();
                        self.push(Value::Number(-value));
                    } else {
                        Vm::runtime_error(&frame, "Operand must be a number.");
                        return Err(LoxError::RuntimeError);
                    }
                }
                Instruction::Nil => self.push(Value::Nil),
                Instruction::Not => {
                    let value = self.pop();
                    self.push(Value::Bool(value.is_falsy()));
                }
                Instruction::Pop => {
                    self.pop();
                }
                Instruction::Print => {
                    let value = self.pop();
                    if let Value::String(s) = value {
                        println!("{}", self.strings.lookup(s))
                    } else {
                        println!("{}", value);
                    }
                }
                Instruction::Return => {
                    return Ok(());
                }
                Instruction::SetGlobal(index) => {
                    // TODO: refactor long indirection?
                    let name = frame.function.chunk.read_string(index);
                    let value = self.peek(0);
                    if let None = self.globals.insert(name, value) {
                        self.globals.remove(&name);
                        let s = self.strings.lookup(name);
                        let msg = format!("Undefined variable '{}'.", s);
                        Vm::runtime_error(&frame, &msg);
                        return Err(LoxError::RuntimeError);
                    }
                }
                Instruction::SetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = self.peek(0);
                    self.stack[i] = value;
                }
                Instruction::Substract => self.binary_op(|a, b| a - b, |n| Value::Number(n))?,
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }

    // TODO: refactor this to return Err
    // TODO: refactor as part of frame
    fn runtime_error(frame: &CallFrame, msg: &str) {
        eprintln!("{}", msg);
        let line = frame.function.chunk.lines[frame.ip - 1]; // TODO: Encapsulate lines?
        eprintln!("[line {}] in script", line);
    }
}
