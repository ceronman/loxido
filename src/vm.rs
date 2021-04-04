use cpu_time::ProcessTime;
use fmt::Debug;

use crate::{
    allocator::{Allocator, Reference, Trace},
    chunk::{Chunk, Instruction, Value},
    closure::Closure,
    closure::ObjUpvalue,
    compiler::Parser,
    error::LoxError,
    function::NativeFn,
};
use std::{collections::HashMap, fmt};

pub struct CallFrame {
    pub closure: Reference<Closure>,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(closure: Reference<Closure>) -> Self {
        CallFrame {
            closure,
            ip: 0,
            slot: 0,
        }
    }
}

const MAX_FRAMES: usize = 64;
const STACK_SIZE: usize = MAX_FRAMES * (std::u8::MAX as usize) + 1;
const MAX_TEMP_ROOTS: usize = 64;

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
    open_upvalues: Vec<Reference<ObjUpvalue>>,
    temp_roots: Vec<Value>,
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            allocator: Allocator::default(),
            frames: Vec::with_capacity(MAX_FRAMES),
            stack: Vec::with_capacity(STACK_SIZE),
            globals: HashMap::new(),
            open_upvalues: Vec::with_capacity(STACK_SIZE),
            temp_roots: Vec::with_capacity(MAX_TEMP_ROOTS),
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
        let name_id = self.allocator.intern(name.to_owned());
        self.globals.insert(name_id, Value::NativeFunction(native));
    }

    // TODO: Maybe return Err(RuntimeError) directly?
    fn runtime_error(&self, msg: &str) -> LoxError {
        let frame = self.current_frame();
        eprintln!("{}", msg);
        let closure = self.allocator.deref(frame.closure);
        let function = self.allocator.deref(closure.function);
        let chunk = &function.chunk;
        let line = chunk.lines[frame.ip - 1];
        eprintln!("[line {}] in script", line);
        LoxError::RuntimeError
    }

    pub fn interpret(&mut self, code: &str) -> Result<(), LoxError> {
        let parser = Parser::new(code, &mut self.allocator);
        let function = parser.compile()?;
        self.push_temp_root(Value::Function(function));
        let closure = self.alloc(Closure::new(function));
        self.frames.push(CallFrame::new(closure));
        self.pop_temp_root();
        self.run()
    }

    // TODO: Investigate macros for this
    fn binary_op<T>(&mut self, f: fn(f64, f64) -> T, r: fn(T) -> Value) -> Result<(), LoxError> {
        let operands = (self.pop(), self.pop());
        match operands {
            (Value::Number(value_b), Value::Number(value_a)) => {
                self.push(r(f(value_a, value_b)));
                Ok(())
            }
            _ => Err(self.runtime_error("Operands must be numbers.")),
        }
    }

    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn current_chunk(&self) -> &Chunk {
        let closure = self.allocator.deref(self.current_frame().closure);
        let function = self.allocator.deref(closure.function);
        &function.chunk
    }

    fn run(&mut self) -> Result<(), LoxError> {
        loop {
            let instruction = self.current_chunk().code[self.current_frame().ip];

            #[cfg(debug_assertions)]
            {
                for value in self.stack.iter() {
                    print!("[{}]", value);
                }
                println!("");

                #[cfg(debug_assertions)]
                self.current_chunk()
                    .disassemble_instruction(&instruction, self.current_frame().ip);
            }

            self.current_frame_mut().ip += 1;

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
                            let s = self.intern(result);
                            let value = Value::String(s);
                            self.push(value);
                        }

                        _ => {
                            self.push(a);
                            self.push(b);
                            return Err(self.runtime_error("Operands must be numbers."));
                        }
                    }
                }
                Instruction::CloseUpvalue => {
                    let stack_top = self.stack.len() - 1;
                    self.close_upvalues(stack_top);
                    self.pop();
                }
                Instruction::Closure(index) => {
                    let c = self.current_chunk().read_constant(index);
                    if let Value::Function(function_id) = c {
                        let upvalue_count = self.allocator.deref(function_id).upvalues.len();
                        let mut new_closure = Closure::new(function_id);

                        for i in 0..upvalue_count {
                            let upvalue = self.allocator.deref(function_id).upvalues[i];
                            let obj_upvalue = if upvalue.is_local {
                                // TODO: unify u8 vs usize everywhere
                                self.capture_upvalue(upvalue.index as usize)
                            } else {
                                let current_closure =
                                    self.allocator.deref(self.current_frame().closure);
                                current_closure.upvalues[upvalue.index as usize].clone()
                            };
                            new_closure.upvalues.push(obj_upvalue)
                        }

                        let closure_id = self.alloc(new_closure);
                        self.push(Value::Closure(closure_id));
                    }
                }
                Instruction::Call(arg_count) => {
                    // TODO: Unify duplicated functionality also in return
                    self.call_value(arg_count)?;
                }
                Instruction::Constant(index) => {
                    let value = self.current_chunk().read_constant(index);
                    self.push(value);
                }
                Instruction::DefineGlobal(index) => {
                    let s = self.current_chunk().read_string(index);
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
                    let s = self.current_chunk().read_string(index);
                    match self.globals.get(&s) {
                        Some(&value) => self.push(value),
                        None => {
                            let name = self.allocator.deref(s);
                            let msg = format!("Undefined variable '{}'.", name);
                            return Err(self.runtime_error(&msg));
                        }
                    }
                }
                Instruction::GetLocal(slot) => {
                    let i = slot as usize + self.current_frame().slot;
                    let value = self.stack[i];
                    self.push(value);
                }
                Instruction::GetUpvalue(slot) => {
                    let value = {
                        let current_closure = self.allocator.deref(self.current_frame().closure);
                        let upvalue_ref = current_closure.upvalues[slot as usize];
                        let upvalue = self.allocator.deref(upvalue_ref);
                        if let Some(value) = upvalue.closed {
                            value
                        } else {
                            self.stack[upvalue.location]
                        }
                    };
                    self.push(value);
                }
                Instruction::Greater => self.binary_op(|a, b| a > b, |n| Value::Bool(n))?,
                Instruction::Jump(offset) => {
                    self.current_frame_mut().ip += offset as usize;
                }
                Instruction::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsy() {
                        self.current_frame_mut().ip += offset as usize;
                    }
                }
                Instruction::Less => self.binary_op(|a, b| a < b, |n| Value::Bool(n))?,
                Instruction::Loop(offset) => {
                    self.current_frame_mut().ip -= offset as usize + 1;
                }
                Instruction::Multiply => self.binary_op(|a, b| a * b, |n| Value::Number(n))?,
                Instruction::Negate => {
                    if let Value::Number(value) = self.peek(0) {
                        self.pop();
                        self.push(Value::Number(-value));
                    } else {
                        return Err(self.runtime_error("Operand must be a number."));
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
                    let frame = self.frames.pop().unwrap();
                    let value = self.pop();
                    self.close_upvalues(frame.slot);
                    self.frames.pop();

                    if self.frames.is_empty() {
                        return Ok(());
                    } else {
                        self.stack.truncate(frame.slot);
                        self.push(value);
                    }
                }
                Instruction::SetGlobal(index) => {
                    // TODO: refactor long indirection?
                    let name = self.current_chunk().read_string(index);
                    let value = self.peek(0);
                    if let None = self.globals.insert(name, value) {
                        self.globals.remove(&name);
                        let s = self.allocator.deref(name);
                        let msg = format!("Undefined variable '{}'.", s);
                        return Err(self.runtime_error(&msg));
                    }
                }
                Instruction::SetLocal(slot) => {
                    let i = slot as usize + self.current_frame().slot;
                    let value = self.peek(0);
                    self.stack[i] = value;
                }
                Instruction::SetUpvalue(slot) => {
                    // TODO: current_closure dance is repeated a lot.
                    let current_closure = self.allocator.deref(self.current_frame().closure);
                    let upvalue_ref = current_closure.upvalues[slot as usize];
                    let value = self.peek(0);
                    let mut upvalue = self.allocator.deref_mut(upvalue_ref);
                    if let None = upvalue.closed {
                        self.stack[upvalue.location] = value;
                    } else {
                        upvalue.closed = Some(value);
                    }
                }
                Instruction::Substract => self.binary_op(|a, b| a - b, |n| Value::Number(n))?,
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }

    fn call_value(&mut self, arg_count: u8) -> Result<(), LoxError> {
        let callee = self.peek(arg_count as usize);
        match callee {
            Value::Closure(cid) => self.call(cid, arg_count),
            Value::NativeFunction(native) => {
                let left = self.stack.len() - arg_count as usize;
                let result = native.0(&self.allocator, &self.stack[left..]);
                self.stack
                    .truncate(self.stack.len() - arg_count as usize - 1);
                self.push(result);
                Ok(())
            }
            _ => Err(self.runtime_error("Can only call functions and classes.")),
        }
    }

    fn capture_upvalue(&mut self, location: usize) -> Reference<ObjUpvalue> {
        for &upvalue_ref in self.open_upvalues.iter() {
            let upvalue = self.allocator.deref(upvalue_ref);
            if upvalue.location == location {
                return upvalue_ref;
            }
        }
        let upvalue = ObjUpvalue::new(location);
        let upvalue = self.alloc(upvalue);
        self.open_upvalues.push(upvalue);
        upvalue
    }

    fn close_upvalues(&mut self, last: usize) {
        let mut i = 0;
        while i != self.open_upvalues.len() {
            let upvalue = self.open_upvalues[i];
            let upvalue = self.allocator.deref_mut(upvalue);
            if upvalue.location >= last {
                // TODO: Might be optimization oportunities for this. Maybe deque.
                self.open_upvalues.remove(i);
                let location = upvalue.location;
                upvalue.closed = Some(self.stack[location]);
            } else {
                i += 1;
            }
        }
    }

    fn call(&mut self, closure_id: Reference<Closure>, arg_count: u8) -> Result<(), LoxError> {
        let closure = self.allocator.deref(closure_id);
        // TODO: Inefficient double lookup;
        let f = self.allocator.deref(closure.function);
        if (arg_count as usize) != f.arity {
            let msg = format!("Expected {} arguments but got {}.", f.arity, arg_count);
            Err(self.runtime_error(&msg))
        } else if self.frames.len() == MAX_FRAMES {
            Err(self.runtime_error("Stack overflow."))
        } else {
            // TODO this looks cleaner with a constructor
            let mut frame = CallFrame::new(closure_id);
            frame.slot = self.stack.len() - (arg_count as usize) - 1;
            self.frames.push(frame);
            Ok(())
        }
    }

    pub fn alloc<T: Trace + 'static + Debug>(&mut self, object: T) -> Reference<T> {
        #[cfg(feature = "debug_log_gc")]
        println!("- begin allocation(val:{:?})", object);

        self.mark_and_sweep();
        let reference = self.allocator.alloc(object);

        #[cfg(feature = "debug_log_gc")]
        println!("- end allocation");

        reference
    }

    pub fn intern(&mut self, name: String) -> Reference<String> {
        self.mark_and_sweep();
        self.allocator.intern(name)
    }

    fn mark_and_sweep(&mut self) {
        #[cfg(feature = "debug_stress_gc")]
        {
            #[cfg(feature = "debug_log_gc")]
            println!("-- gc begin");

            self.mark_roots();
            self.allocator.collect_garbage();

            #[cfg(feature = "debug_log_gc")]
            println!("-- gc end");
        }
    }

    fn mark_roots(&mut self) {
        for &value in &self.stack {
            self.allocator.mark_value(value);
        }

        for frame in &self.frames {
            self.allocator.mark_object(frame.closure)
        }

        for &upvalue in &self.open_upvalues {
            self.allocator.mark_object(upvalue);
        }

        for (&k, &v) in &self.globals {
            self.allocator.mark_object(k);
            self.allocator.mark_value(v);
        }

        for &value in &self.temp_roots {
            self.allocator.mark_value(value);
        }
    }

    fn push_temp_root(&mut self, v: Value) {
        self.temp_roots.push(v);
    }

    fn pop_temp_root(&mut self) {
        self.temp_roots.pop();
    }
}
