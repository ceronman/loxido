use cpu_time::ProcessTime;
use fmt::Debug;

use crate::{
    allocator::{Allocator, Reference, Trace, TraceFormatter},
    chunk::{Chunk, Instruction, Table, Value},
    compiler::compile,
    error::LoxError,
    objects::{BoundMethod, Closure, Instance, LoxClass, NativeFn, ObjUpvalue},
};
use std::fmt;

pub struct Vm {
    allocator: Allocator,
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: Table,
    open_upvalues: Vec<Reference<ObjUpvalue>>,
    init_string: Reference<String>,
    start_time: ProcessTime,
}

impl Vm {
    const MAX_FRAMES: usize = 64;
    const STACK_SIZE: usize = Vm::MAX_FRAMES * (std::u8::MAX as usize) + 1;

    pub fn new() -> Self {
        let mut allocator = Allocator::new();
        let init_string = allocator.intern("init".to_owned());

        let mut vm = Self {
            allocator,
            frames: Vec::with_capacity(Vm::MAX_FRAMES),
            stack: Vec::with_capacity(Vm::STACK_SIZE),
            globals: Table::new(),
            open_upvalues: Vec::with_capacity(Vm::STACK_SIZE),
            init_string,
            start_time: ProcessTime::now(),
        };
        vm.define_native("clock", NativeFn(clock));
        vm.define_native("panic", NativeFn(lox_panic));
        vm
    }

    pub fn interpret(&mut self, code: &str) -> Result<(), LoxError> {
        let function = compile(code, &mut self.allocator)?;
        self.push(Value::Function(function));
        let closure = self.alloc(Closure::new(function));
        self.frames.push(CallFrame::new(closure, 0));
        self.run()
    }

    fn push(&mut self, v: Value) {
        self.stack.push(v);
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("Empty stack")
    }

    fn peek(&self, n: usize) -> Value {
        let size = self.stack.len();
        self.stack[size - 1 - n]
    }

    fn set_at(&mut self, n: usize, value: Value) {
        let size = self.stack.len();
        self.stack[size - 1 - n] = value;
    }

    fn define_native(&mut self, name: &str, native: NativeFn) {
        let name = self.allocator.intern(name.to_owned());
        self.globals.insert(name, Value::NativeFunction(native));
    }

    fn runtime_error(&self, msg: &str) -> Result<(), LoxError> {
        let frame = self.current_frame();
        eprintln!("{}", msg);
        let chunk = self.current_chunk();
        let line = chunk.lines[frame.ip - 1];
        eprintln!("[line {}] in script", line);
        Err(LoxError::RuntimeError)
    }

    // PERF: Investigate macros for this
    fn binary_op<T>(&mut self, f: fn(f64, f64) -> T, r: fn(T) -> Value) -> Result<(), LoxError> {
        let operands = (self.pop(), self.pop());
        match operands {
            (Value::Number(value_b), Value::Number(value_a)) => {
                self.push(r(f(value_a, value_b)));
                Ok(())
            }
            _ => self.runtime_error("Operands must be numbers."),
        }
    }

    // PERF: Investigate making frames fixed array.
    // PERF: Investigate making CallFrame Copy
    fn current_frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn current_frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    // PERF: Investigate unsafe
    fn current_chunk(&self) -> &Chunk {
        let closure = self.current_closure();
        let function = self.allocator.deref(closure.function);
        &function.chunk
    }

    // PERF: Investigate unsafe
    fn current_closure(&self) -> &Closure {
        let closure = self.current_frame().closure;
        let closure = self.allocator.deref(closure);
        &closure
    }

    fn run(&mut self) -> Result<(), LoxError> {
        loop {
            let instruction = self.current_chunk().code[self.current_frame().ip];

            #[cfg(feature = "debug_trace_execution")]
            {
                for value in self.stack.iter() {
                    print!("[{}]", value);
                }
                println!();

                self.current_chunk()
                    .disassemble_instruction(&instruction, self.current_frame().ip);
            }

            self.current_frame_mut().ip += 1;

            match instruction {
                Instruction::Add => {
                    let (b, a) = (self.pop(), self.pop());
                    match (&a, &b) {
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(Value::Number(a + b));
                        }

                        (Value::String(a), Value::String(b)) => {
                            let a = self.allocator.deref(*a);
                            let b = self.allocator.deref(*b);
                            let result = format!("{}{}", a, b);
                            let result = self.intern(result);
                            let value = Value::String(result);
                            self.push(value);
                        }

                        _ => {
                            self.push(a);
                            self.push(b);
                            return self
                                .runtime_error("Operands must be two numbers or two strings.");
                        }
                    }
                }
                Instruction::Class(constant) => {
                    let class_name = self.current_chunk().read_string(constant);
                    let class = LoxClass::new(class_name);
                    let class = self.alloc(class);
                    self.push(Value::Class(class));
                }
                Instruction::CloseUpvalue => {
                    let stack_top = self.stack.len() - 1;
                    self.close_upvalues(stack_top);
                    self.pop();
                }
                Instruction::Closure(constant) => {
                    let function = self.current_chunk().read_constant(constant);
                    if let Value::Function(function) = function {
                        let upvalue_count = self.allocator.deref(function).upvalues.len();
                        let mut closure = Closure::new(function);

                        for i in 0..upvalue_count {
                            let upvalue = self.allocator.deref(function).upvalues[i];
                            let obj_upvalue = if upvalue.is_local {
                                let location = self.current_frame().slot + upvalue.index as usize;
                                self.capture_upvalue(location)
                            } else {
                                self.current_closure().upvalues[upvalue.index as usize]
                            };
                            closure.upvalues.push(obj_upvalue)
                        }

                        let closure = self.alloc(closure);
                        self.push(Value::Closure(closure));
                    } else {
                        panic!("Closure instruction without function value");
                    }
                }
                Instruction::Call(arg_count) => {
                    self.call_value(arg_count as usize)?;
                }
                Instruction::Constant(constant) => {
                    let value = self.current_chunk().read_constant(constant);
                    self.push(value);
                }
                Instruction::DefineGlobal(constant) => {
                    let global_name = self.current_chunk().read_string(constant);
                    let value = self.pop();
                    self.globals.insert(global_name, value);
                }
                Instruction::Divide => self.binary_op(|a, b| a / b, Value::Number)?,
                Instruction::Equal => {
                    let a = self.pop();
                    let b = self.pop();
                    self.push(Value::Bool(a == b));
                }
                Instruction::False => self.push(Value::Bool(false)),
                Instruction::GetGlobal(constant) => {
                    let global_name = self.current_chunk().read_string(constant);
                    match self.globals.get(&global_name) {
                        Some(&value) => self.push(value),
                        None => {
                            let global_name = self.allocator.deref(global_name);
                            let msg = format!("Undefined variable '{}'.", global_name);
                            return self.runtime_error(&msg);
                        }
                    }
                }
                Instruction::GetLocal(slot) => {
                    let i = slot as usize + self.current_frame().slot;
                    let value = self.stack[i];
                    self.push(value);
                }
                Instruction::GetProperty(constant) => {
                    if let Value::Instance(instance) = self.peek(0) {
                        let instance = self.allocator.deref(instance);
                        let class = instance.class;
                        let property_name = self.current_chunk().read_string(constant);
                        let value = instance.get_property(property_name);
                        match value {
                            Some(value) => {
                                self.pop();
                                self.push(value);
                            }
                            None => {
                                self.bind_method(class, property_name)?;
                            }
                        }
                    } else {
                        return self.runtime_error("Only instances have properties.");
                    }
                }
                Instruction::GetSuper(constant) => {
                    let method_name = self.current_chunk().read_string(constant);
                    if let Value::Class(superclass) = self.pop() {
                        self.bind_method(superclass, method_name)?
                    } else {
                        panic!("super found no class");
                    }
                }
                Instruction::GetUpvalue(slot) => {
                    let value = {
                        let upvalue = self.current_closure().upvalues[slot as usize];
                        let upvalue = self.allocator.deref(upvalue);
                        if let Some(value) = upvalue.closed {
                            value
                        } else {
                            self.stack[upvalue.location]
                        }
                    };
                    self.push(value);
                }
                Instruction::Greater => self.binary_op(|a, b| a > b, Value::Bool)?,
                Instruction::Inherit => {
                    let pair = (self.peek(0), self.peek(1));
                    if let (Value::Class(subclass), Value::Class(superclass)) = pair {
                        let superclass = self.allocator.deref(superclass);
                        let methods = superclass.methods.clone();
                        let mut subclass = self.allocator.deref_mut(subclass);
                        subclass.methods = methods;
                        self.pop();
                    } else {
                        return self.runtime_error("Superclass must be a class.");
                    }
                }
                Instruction::Invoke((constant, arg_count)) => {
                    let name = self.current_chunk().read_string(constant);
                    self.invoke(name, arg_count as usize)?;
                }
                Instruction::Jump(offset) => {
                    self.current_frame_mut().ip += offset as usize;
                }
                Instruction::JumpIfFalse(offset) => {
                    if self.peek(0).is_falsey() {
                        self.current_frame_mut().ip += offset as usize;
                    }
                }
                Instruction::Less => self.binary_op(|a, b| a < b, Value::Bool)?,
                Instruction::Loop(offset) => {
                    self.current_frame_mut().ip -= offset as usize + 1;
                }
                Instruction::Method(constant) => {
                    let method_name = self.current_chunk().read_string(constant);
                    self.define_method(method_name);
                }
                Instruction::Multiply => self.binary_op(|a, b| a * b, Value::Number)?,
                Instruction::Negate => {
                    if let Value::Number(value) = self.peek(0) {
                        self.pop();
                        self.push(Value::Number(-value));
                    } else {
                        return self.runtime_error("Operand must be a number.");
                    }
                }
                Instruction::Nil => self.push(Value::Nil),
                Instruction::Not => {
                    let value = self.pop();
                    self.push(Value::Bool(value.is_falsey()));
                }
                Instruction::Pop => {
                    self.pop();
                }
                Instruction::Print => {
                    let value = self.pop();
                    let formatter = TraceFormatter::new(value, &self.allocator);
                    println!("{}", formatter);
                }
                Instruction::Return => {
                    let frame = self.frames.pop().unwrap();
                    let return_value = self.pop();
                    self.close_upvalues(frame.slot);

                    if self.frames.is_empty() {
                        return Ok(());
                    } else {
                        self.stack.truncate(frame.slot);
                        self.push(return_value);
                    }
                }
                Instruction::SetGlobal(constant) => {
                    let global_name = self.current_chunk().read_string(constant);
                    let value = self.peek(0);
                    if self.globals.insert(global_name, value).is_none() {
                        self.globals.remove(&global_name);
                        let s = self.allocator.deref(global_name);
                        let msg = format!("Undefined variable '{}'.", s);
                        return self.runtime_error(&msg);
                    }
                }
                Instruction::SetLocal(slot) => {
                    let i = slot as usize + self.current_frame().slot;
                    let value = self.peek(0);
                    self.stack[i] = value;
                }
                Instruction::SetProperty(constant) => {
                    if let Value::Instance(instance) = self.peek(1) {
                        let property_name = self.current_chunk().read_string(constant);
                        let value = self.pop();
                        let instance = self.allocator.deref_mut(instance);
                        instance.set_property(property_name, value);
                        self.pop();
                        self.push(value);
                    } else {
                        return self.runtime_error("Only instances have fields.");
                    }
                }
                Instruction::SetUpvalue(slot) => {
                    let upvalue = self.current_closure().upvalues[slot as usize];
                    let value = self.peek(0);
                    let mut upvalue = self.allocator.deref_mut(upvalue);
                    if upvalue.closed.is_none() {
                        self.stack[upvalue.location] = value;
                    } else {
                        upvalue.closed = Some(value);
                    }
                }
                Instruction::Substract => self.binary_op(|a, b| a - b, Value::Number)?,
                Instruction::SuperInvoke((constant, arg_count)) => {
                    let method_name = self.current_chunk().read_string(constant);
                    if let Value::Class(class) = self.pop() {
                        self.invoke_from_class(class, method_name, arg_count as usize)?;
                    } else {
                        panic!("super invoke with no class");
                    }
                }
                Instruction::True => self.push(Value::Bool(true)),
            };
        }
    }
    fn call_value(&mut self, arg_count: usize) -> Result<(), LoxError> {
        let callee = self.peek(arg_count);
        match callee {
            Value::BoundMethod(bound) => {
                let bound = self.allocator.deref(bound);
                let method = bound.method;
                let receiver = bound.receiver;
                self.set_at(arg_count, receiver);
                self.call(method, arg_count)
            }
            Value::Class(class) => {
                let instance = Instance::new(class);
                let instance = self.alloc(instance);
                self.set_at(arg_count, Value::Instance(instance));
                let class = self.allocator.deref(class);
                if let Some(&initializer) = class.methods.get(&self.init_string) {
                    if let Value::Closure(initializer) = initializer {
                        return self.call(initializer, arg_count);
                    }
                    return self.runtime_error("Initializer is not closure");
                } else if arg_count != 0 {
                    let msg = format!("Expected 0 arguments but got {}.", arg_count);
                    return self.runtime_error(&msg);
                }
                Ok(())
            }
            Value::Closure(closure) => self.call(closure, arg_count),
            Value::NativeFunction(native) => {
                let left = self.stack.len() - arg_count;
                let result = native.0(&self, &self.stack[left..]);
                self.stack.truncate(left - 1);
                self.push(result);
                Ok(())
            }
            _ => self.runtime_error("Can only call functions and classes."),
        }
    }

    fn call(&mut self, closure_ref: Reference<Closure>, arg_count: usize) -> Result<(), LoxError> {
        let closure = self.allocator.deref(closure_ref);
        let function = self.allocator.deref(closure.function);
        if arg_count != function.arity {
            let msg = format!(
                "Expected {} arguments but got {}.",
                function.arity, arg_count
            );
            self.runtime_error(&msg)
        } else if self.frames.len() == Vm::MAX_FRAMES {
            self.runtime_error("Stack overflow.")
        } else {
            let frame = CallFrame::new(closure_ref, self.stack.len() - arg_count - 1);
            self.frames.push(frame);
            Ok(())
        }
    }

    fn invoke(&mut self, name: Reference<String>, arg_count: usize) -> Result<(), LoxError> {
        let receiver = self.peek(arg_count);
        if let Value::Instance(instance) = receiver {
            let instance = self.allocator.deref(instance);
            if let Some(field) = instance.get_property(name) {
                self.set_at(arg_count, field);
                self.call_value(arg_count)
            } else {
                let class = instance.class;
                self.invoke_from_class(class, name, arg_count)
            }
        } else {
            self.runtime_error("Only instances have methods.")
        }
    }

    fn invoke_from_class(
        &mut self,
        class: Reference<LoxClass>,
        name: Reference<String>,
        arg_count: usize,
    ) -> Result<(), LoxError> {
        let class = self.allocator.deref(class);
        if let Some(&method) = class.methods.get(&name) {
            if let Value::Closure(closure) = method {
                self.call(closure, arg_count)
            } else {
                panic!("Got method that is not closure!")
            }
        } else {
            let name = self.allocator.deref(name);
            let msg = format!("Undefined property '{}'.", name);
            self.runtime_error(&msg)
        }
    }

    fn bind_method(
        &mut self,
        class: Reference<LoxClass>,
        name: Reference<String>,
    ) -> Result<(), LoxError> {
        let class = self.allocator.deref(class);
        if let Some(method) = class.methods.get(&name) {
            let receiver = self.peek(0);
            let method = match method {
                Value::Closure(closure) => *closure,
                _ => panic!("Inconsistent state. Method is not closure"),
            };
            let bound = BoundMethod::new(receiver, method);
            let bound = self.alloc(bound);
            self.pop();
            self.push(Value::BoundMethod(bound));
            Ok(())
        } else {
            let name = self.allocator.deref(name);
            let msg = format!("Undefined property '{}'.", name);
            self.runtime_error(&msg)
        }
    }

    fn capture_upvalue(&mut self, location: usize) -> Reference<ObjUpvalue> {
        for &upvalue_ref in &self.open_upvalues {
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
                // PERF: Remove is expensive
                self.open_upvalues.remove(i);
                let location = upvalue.location;
                upvalue.closed = Some(self.stack[location]);
            } else {
                i += 1;
            }
        }
    }

    fn define_method(&mut self, name: Reference<String>) {
        let method = self.peek(0);
        if let Value::Class(class) = self.peek(1) {
            let class = self.allocator.deref_mut(class);
            class.methods.insert(name, method);
            self.pop();
        } else {
            panic!("Invalid state: trying to define a method of non class");
        }
    }

    fn alloc<T: Trace + 'static + Debug>(&mut self, object: T) -> Reference<T> {
        self.mark_and_sweep();
        self.allocator.alloc(object)
    }

    fn intern(&mut self, name: String) -> Reference<String> {
        self.mark_and_sweep();
        self.allocator.intern(name)
    }

    fn mark_and_sweep(&mut self) {
        if self.allocator.should_gc() {
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

        self.allocator.mark_table(&self.globals);
        self.allocator.mark_object(self.init_string);
    }
}

struct CallFrame {
    closure: Reference<Closure>,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(closure: Reference<Closure>, slot: usize) -> Self {
        CallFrame {
            closure,
            ip: 0,
            slot,
        }
    }
}

fn clock(vm: &Vm, _args: &[Value]) -> Value {
    let time = vm.start_time.elapsed().as_secs_f64();
    Value::Number(time)
}

fn lox_panic(vm: &Vm, args: &[Value]) -> Value {
    let mut terms: Vec<String> = vec![];

    for &arg in args.iter() {
        let formatter = TraceFormatter::new(arg, &vm.allocator);
        let term = format!("{}", formatter);
        terms.push(term);
    }

    panic!("panic: {}", terms.join(", "))
}
