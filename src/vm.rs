use crate::{
    chunk::{Chunk, Instruction, Value},
    error::LoxError,
    strings::LoxString,
};
use std::collections::HashMap;

pub struct Vm {
    chunk: Chunk,
    ip: usize,
    stack: Vec<Value>,
    globals: HashMap<LoxString, Value>,
}

impl Vm {
    pub fn new(chunk: Chunk) -> Vm {
        Vm {
            chunk,
            ip: 0,
            stack: Vec::with_capacity(256),
            globals: HashMap::new(),
        }
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
                self.runtime_error("Operands must be numbers.");
                Err(LoxError::RuntimeError)
            }
        }
    }

    pub fn run(&mut self) -> Result<(), LoxError> {
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
                            let s_a = self.chunk.strings.lookup(*value_a);
                            let s_b = self.chunk.strings.lookup(*value_b);
                            let result = format!("{}{}", s_a, s_b);
                            let s = self.chunk.strings.intern_onwed(result);
                            let value = Value::String(s);
                            self.push(value);
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
                Instruction::DefineGlobal(index) => {
                    if let Value::String(s) = self.chunk.read_constant(index) {
                        let value = self.pop();
                        self.globals.insert(s, value);
                    }
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
                Instruction::Pop => {
                    self.pop();
                }
                Instruction::Print => {
                    let value = self.pop();
                    if let Value::String(s) = value {
                        println!("{}", self.chunk.strings.lookup(s))
                    } else {
                        println!("{}", value);
                    }
                }
                Instruction::Return => {
                    return Ok(());
                }
                Instruction::Substract => self.binary_op(|a, b| a - b, |n| Value::Number(n))?,
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }

    fn next_instruction(&mut self) -> Instruction {
        let instruction = self.chunk.code[self.ip]; // TODO: Encapsulate code?
        self.ip += 1;
        instruction
    }

    fn runtime_error(&mut self, msg: &str) {
        eprintln!("{}", msg);
        let line = self.chunk.lines[self.ip - 1]; // TODO: Encapsulate lines?
        eprintln!("[line {}] in script", line);
    }
}
