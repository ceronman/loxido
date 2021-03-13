use cpu_time::ProcessTime;

use crate::{
    allocator::{Allocator, Reference},
    chunk::{Chunk, Instruction, Value},
    closure::Closure,
    closure::OpenUpvalues,
    compiler::Parser,
    error::LoxError,
    function::{LoxFunction, NativeFn},
};
use std::collections::HashMap;

struct CallFrame {
    function: Reference<LoxFunction>,
    closure: Reference<Closure>,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(function: Reference<LoxFunction>, closure: Reference<Closure>) -> Self {
        CallFrame {
            function,
            closure,
            ip: 0,
            slot: 0,
        }
    }
}

const MAX_FRAMES: usize = 64;
const STACK_SIZE: usize = MAX_FRAMES * (std::u8::MAX as usize) + 1;

lazy_static! {
    static ref BEGIN_OF_PROGRAM: ProcessTime = ProcessTime::now();
}

fn clock(_allocator: &Allocator, _args: &[Value]) -> Value {
    Value::Number(BEGIN_OF_PROGRAM.elapsed().as_secs_f64())
}

fn lox_panic(allocator: &Allocator, args: &[Value]) -> Value {
    let mut terms: Vec<String> = vec![];

    for arg in args.iter() {
        let term = if let Value::String(s) = *arg {
            format!("{}", allocator.deref(s))
        } else {
            format!("{}", arg)
        };
        terms.push(term);
    }

    panic!("panic: {}", terms.join(", "))
}

pub struct Vm {
    allocator: Allocator,
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<Reference<String>, Value>,
    open_upvalues: OpenUpvalues,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            allocator: Allocator::default(),
            frames: Vec::with_capacity(MAX_FRAMES),
            stack: Vec::with_capacity(STACK_SIZE),
            globals: HashMap::new(),
            open_upvalues: OpenUpvalues::new(STACK_SIZE),
        };
        vm.define_native("clock", NativeFn(clock));
        vm.define_native("panic", NativeFn(lox_panic));
        vm
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
        self.stack[size - 1 - n]
    }

    fn define_native(&mut self, name: &str, native: NativeFn) {
        let name_id = self.allocator.intern(name);
        self.globals.insert(name_id, Value::NativeFunction(native));
    }

    fn chunk_for(&self, frame: &CallFrame) -> &Chunk {
        let closure = self.allocator.deref(frame.closure);
        let function = self.allocator.deref(closure.function);
        &function.chunk
    }

    // TODO: Maybe return Err(RuntimeError) directly?
    fn runtime_error(&self, frame: &CallFrame, msg: &str) -> LoxError {
        eprintln!("{}", msg);
        let chunk = &self.allocator.deref(frame.function).chunk;
        let line = chunk.lines[frame.ip - 1];
        eprintln!("[line {}] in script", line);
        LoxError::RuntimeError
    }

    pub fn interpret(&mut self, code: &str) -> Result<(), LoxError> {
        let parser = Parser::new(code, &mut self.allocator);
        let function = parser.compile()?;
        let closure = Closure::new(function);
        let closure_id = self.allocator.alloc(closure);
        self.frames.push(CallFrame::new(function, closure_id));
        self.run()
    }

    // TODO: Investigate macros for this
    fn binary_op<T>(
        &mut self,
        frame: &CallFrame,
        f: fn(f64, f64) -> T,
        r: fn(T) -> Value,
    ) -> Result<(), LoxError> {
        let operands = (self.pop(), self.pop());
        match operands {
            (Value::Number(value_b), Value::Number(value_a)) => {
                self.push(r(f(value_a, value_b)));
                Ok(())
            }
            _ => Err(self.runtime_error(&frame, "Operands must be numbers.")),
        }
    }

    fn run(&mut self) -> Result<(), LoxError> {
        let mut frame = self.frames.pop().unwrap();

        loop {
            let instruction = self.chunk_for(&frame).code[frame.ip];

            #[cfg(debug_assertions)]
            {
                for value in self.stack.iter() {
                    print!("[{}]", value);
                }
                println!("");

                #[cfg(debug_assertions)]
                self.chunk_for(&frame)
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
                            let s_a = self.allocator.deref(*value_a);
                            let s_b = self.allocator.deref(*value_b);
                            let result = format!("{}{}", s_a, s_b);
                            let s = self.allocator.intern_owned(result);
                            let value = Value::String(s);
                            self.push(value);
                        }

                        _ => {
                            self.push(a);
                            self.push(b);
                            return Err(self.runtime_error(&frame, "Operands must be numbers."));
                        }
                    }
                }
                Instruction::CloseUpvalue => {
                    let stack_top = self.stack.len() - 1;
                    self.open_upvalues.close_upvalues(&self.stack, stack_top);
                    self.pop();
                }
                Instruction::Closure(index) => {
                    let c = self.chunk_for(&frame).read_constant(index);
                    if let Value::Function(function_id) = c {
                        let upvalues = self.allocator.deref(function_id).upvalues.iter();
                        let mut new_closure = Closure::new(function_id);

                        for upvalue in upvalues {
                            let obj_upvalue = if upvalue.is_local {
                                // TODO: unify u8 vs usize everywhere
                                self.open_upvalues.capture(upvalue.index as usize)
                            } else {
                                let current_closure = self.allocator.deref(frame.closure);
                                current_closure.upvalues[upvalue.index as usize].clone()
                            };
                            new_closure.upvalues.push(obj_upvalue)
                        }

                        let closure_id = self.allocator.alloc(new_closure);
                        self.push(Value::Closure(closure_id));
                    }
                }
                Instruction::Call(arg_count) => {
                    // TODO: Unify duplicated functionality also in return
                    frame = self.call_value(frame, arg_count)?;
                }
                Instruction::Constant(index) => {
                    let value = self.chunk_for(&frame).read_constant(index);
                    self.push(value);
                }
                Instruction::DefineGlobal(index) => {
                    let s = self.chunk_for(&frame).read_string(index);
                    let value = self.pop();
                    self.globals.insert(s, value);
                }
                Instruction::Divide => {
                    self.binary_op(&frame, |a, b| a / b, |n| Value::Number(n))?
                }
                Instruction::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::Bool(a == b));
                }
                Instruction::False => self.push(Value::Bool(false)),
                Instruction::GetGlobal(index) => {
                    let s = self.chunk_for(&frame).read_string(index);
                    match self.globals.get(&s) {
                        Some(&value) => self.push(value),
                        None => {
                            let name = self.allocator.deref(s);
                            let msg = format!("Undefined variable '{}'.", name);
                            return Err(self.runtime_error(&frame, &msg));
                        }
                    }
                }
                Instruction::GetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = self.stack[i];
                    self.push(value);
                }
                Instruction::GetUpvalue(slot) => {
                    let value = {
                        let current_closure = self.allocator.deref(frame.closure);
                        let upvalue = current_closure.upvalues[slot as usize].borrow();
                        if let Some(value) = upvalue.closed {
                            value
                        } else {
                            self.stack[upvalue.location]
                        }
                    };
                    self.push(value);
                }
                Instruction::Greater => self.binary_op(&frame, |a, b| a > b, |n| Value::Bool(n))?,
                Instruction::Jump(offset) => {
                    frame.ip += offset as usize;
                }
                Instruction::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsy() {
                        frame.ip += offset as usize;
                    }
                }
                Instruction::Less => self.binary_op(&frame, |a, b| a < b, |n| Value::Bool(n))?,
                Instruction::Loop(offset) => {
                    frame.ip -= offset as usize + 1;
                }
                Instruction::Multiply => {
                    self.binary_op(&frame, |a, b| a * b, |n| Value::Number(n))?
                }
                Instruction::Negate => {
                    if let Value::Number(value) = self.peek(0) {
                        self.pop();
                        self.push(Value::Number(-value));
                    } else {
                        return Err(self.runtime_error(&frame, "Operand must be a number."));
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
                        println!("{}", self.allocator.deref(s))
                    } else {
                        println!("{}", value);
                    }
                }
                Instruction::Return => {
                    let value = self.pop();
                    self.open_upvalues.close_upvalues(&self.stack, frame.slot);
                    match self.frames.pop() {
                        Some(f) => {
                            self.stack.truncate(frame.slot);
                            self.push(value);
                            frame = f;
                        }
                        None => {
                            return Ok(());
                        }
                    }
                }
                Instruction::SetGlobal(index) => {
                    // TODO: refactor long indirection?
                    let name = self.chunk_for(&frame).read_string(index);
                    let value = self.peek(0);
                    if let None = self.globals.insert(name, value) {
                        self.globals.remove(&name);
                        let s = self.allocator.deref(name);
                        let msg = format!("Undefined variable '{}'.", s);
                        return Err(self.runtime_error(&frame, &msg));
                    }
                }
                Instruction::SetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = self.peek(0);
                    self.stack[i] = value;
                }
                Instruction::SetUpvalue(slot) => {
                    // TODO: current_closure dance is repeated a lot.
                    let current_closure = self.allocator.deref(frame.closure);
                    let mut upvalue = current_closure.upvalues[slot as usize].borrow_mut();
                    let value = self.peek(0);
                    if let None = upvalue.closed {
                        self.stack[upvalue.location] = value;
                    } else {
                        upvalue.closed = Some(value);
                    }
                }
                Instruction::Substract => {
                    self.binary_op(&frame, |a, b| a - b, |n| Value::Number(n))?
                }
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }

    fn call_value(&mut self, frame: CallFrame, arg_count: u8) -> Result<CallFrame, LoxError> {
        let callee = self.peek(arg_count as usize);
        match callee {
            Value::Closure(cid) => self.call(frame, cid, arg_count),
            Value::NativeFunction(native) => {
                let left = self.stack.len() - arg_count as usize;
                let result = native.0(&self.allocator, &self.stack[left..]);
                self.stack
                    .truncate(self.stack.len() - arg_count as usize - 1);
                self.push(result);
                Ok(frame)
            }
            _ => Err(self.runtime_error(&frame, "Can only call functions and classes.")),
        }
    }

    fn call(
        &mut self,
        frame: CallFrame,
        closure_id: Reference<Closure>,
        arg_count: u8,
    ) -> Result<CallFrame, LoxError> {
        let closure = self.allocator.deref(closure_id);
        // TODO: Inefficient double lookup;
        let f = self.allocator.deref(closure.function);
        if (arg_count as usize) != f.arity {
            let msg = format!("Expected {} arguments but got {}.", f.arity, arg_count);
            Err(self.runtime_error(&frame, &msg))
        } else if self.frames.len() == MAX_FRAMES {
            Err(self.runtime_error(&frame, "Stack overflow."))
        } else {
            self.frames.push(frame);
            // TODO this looks cleaner with a constructor
            let mut frame = CallFrame::new(closure.function, closure_id);
            frame.slot = self.stack.len() - (arg_count as usize) - 1;
            Ok(frame)
        }
    }
}
