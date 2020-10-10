use cpu_time::ProcessTime;

use crate::{
    chunk::{Instruction, Value},
    closure::Closure,
    closure::ClosureId,
    closure::Closures,
    closure::ObjUpvalue,
    compiler::Parser,
    error::LoxError,
    function::NativeFn,
    function::{FunctionId, Functions},
    strings::{LoxString, Strings},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

struct CallFrame {
    function: FunctionId,
    closure: ClosureId,
    ip: usize,
    slot: usize,
}

impl CallFrame {
    fn new(function: FunctionId, closure: ClosureId) -> Self {
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

pub struct ExecutionState {
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<LoxString, Value>,
    closures: Closures,
    open_upvalues: Vec<Rc<RefCell<ObjUpvalue>>>,
}

lazy_static! {
    static ref BEGIN_OF_PROGRAM: ProcessTime = ProcessTime::now();
}

fn clock(_args: &[Value]) -> Value {
    Value::Number(BEGIN_OF_PROGRAM.elapsed().as_secs_f64())
}

impl ExecutionState {
    pub fn new(strings: &mut Strings) -> Self {
        let mut state = Self {
            frames: Vec::with_capacity(MAX_FRAMES),
            stack: Vec::with_capacity(STACK_SIZE),
            globals: HashMap::new(),
            closures: Closures::default(),
            open_upvalues: Vec::with_capacity(STACK_SIZE),
        };
        state.define_native(strings, "clock", NativeFn(clock));
        state
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

    fn define_native(&mut self, strings: &mut Strings, name: &str, native: NativeFn) {
        let name_id = strings.intern(name);
        self.globals.insert(name_id, Value::NativeFunction(native));
    }
}

#[derive(Default)]
pub struct Vm {
    strings: Strings,
    functions: Functions,
}

impl Vm {
    pub fn new_state(&mut self) -> ExecutionState {
        ExecutionState::new(&mut self.strings)
    }

    pub fn interpret(&mut self, code: &str, state: &mut ExecutionState) -> Result<(), LoxError> {
        let parser = Parser::new(code, &mut self.strings, &mut self.functions);
        let function = parser.compile()?;
        let closure = Closure::new(function);
        let closure_id = state.closures.store(closure);
        state.frames.push(CallFrame::new(function, closure_id));
        self.run(state)
    }

    // TODO: Investigate macros for this
    fn binary_op<T>(
        &self,
        frame: &CallFrame,
        state: &mut ExecutionState,
        f: fn(f64, f64) -> T,
        r: fn(T) -> Value,
    ) -> Result<(), LoxError> {
        let operands = (state.pop(), state.pop());
        match operands {
            (Value::Number(value_b), Value::Number(value_a)) => {
                state.push(r(f(value_a, value_b)));
                Ok(())
            }
            _ => Err(self.runtime_error(&frame, "Operands must be numbers.")),
        }
    }

    fn run(&mut self, state: &mut ExecutionState) -> Result<(), LoxError> {
        let mut frame = state.frames.pop().unwrap();

        // TODO: Maybe get rid of this references and use only frame
        let mut chunk = {
            let closure = state.closures.lookup(frame.closure);
            &self.functions.lookup(closure.function).chunk
        };

        loop {
            let instruction = chunk.code[frame.ip];

            #[cfg(debug_assertions)]
            {
                for value in state.stack.iter() {
                    print!("[{}]", value);
                }
                println!("");

                #[cfg(debug_assertions)]
                chunk.disassemble_instruction(&instruction, frame.ip);
            }

            frame.ip += 1;

            match instruction {
                Instruction::Add => {
                    let (b, a) = (state.pop(), state.pop());
                    match (&a, &b) {
                        (Value::Number(value_a), Value::Number(value_b)) => {
                            state.push(Value::Number(value_a + value_b));
                        }

                        (Value::String(value_a), Value::String(value_b)) => {
                            let s_a = self.strings.lookup(*value_a);
                            let s_b = self.strings.lookup(*value_b);
                            let result = format!("{}{}", s_a, s_b);
                            let s = self.strings.intern_onwed(result);
                            let value = Value::String(s);
                            state.push(value);
                        }

                        _ => {
                            state.push(a);
                            state.push(b);
                            return Err(self.runtime_error(&frame, "Operands must be numbers."));
                        }
                    }
                }
                Instruction::CloseUpvalue => {
                    let stack_top = state.stack.len() - 1;
                    self.close_upvalues(state, stack_top);
                    state.pop();
                }
                Instruction::Closure(index) => {
                    let c = chunk.read_constant(index);
                    if let Value::Function(function_id) = c {
                        let function = self.functions.lookup(function_id);
                        let mut new_closure = Closure::new(function_id);

                        for upvalue in function.upvalues.iter() {
                            let obj_upvalue = if upvalue.is_local {
                                // TODO: unify u8 vs usize everywhere
                                self.capture_value(state, frame.slot + upvalue.index as usize)
                            } else {
                                let current_closure = state.closures.lookup(frame.closure);
                                current_closure.upvalues[upvalue.index as usize].clone()
                            };
                            new_closure.upvalues.push(obj_upvalue)
                        }

                        let closure_id = state.closures.store(new_closure);
                        state.push(Value::Closure(closure_id));
                    }
                }
                Instruction::Call(arg_count) => {
                    // TODO: Unify duplicated functionality also in return
                    frame = self.call_value(frame, state, arg_count)?;
                    chunk = &self.functions.lookup(frame.function).chunk;
                }
                Instruction::Constant(index) => {
                    let value = chunk.read_constant(index);
                    state.push(value);
                }
                Instruction::DefineGlobal(index) => {
                    let s = chunk.read_string(index);
                    let value = state.pop();
                    state.globals.insert(s, value);
                }
                Instruction::Divide => {
                    self.binary_op(&frame, state, |a, b| a / b, |n| Value::Number(n))?
                }
                Instruction::Equal => {
                    let a = state.pop();
                    let b = state.pop();
                    state.push(Value::Bool(a == b));
                }
                Instruction::False => state.push(Value::Bool(false)),
                Instruction::GetGlobal(index) => {
                    let s = chunk.read_string(index);
                    match state.globals.get(&s) {
                        Some(&value) => state.push(value),
                        None => {
                            let name = self.strings.lookup(s);
                            let msg = format!("Undefined variable '{}'.", name);
                            return Err(self.runtime_error(&frame, &msg));
                        }
                    }
                }
                Instruction::GetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = state.stack[i];
                    state.push(value);
                }
                Instruction::GetUpvalue(slot) => {
                    let value = {
                        let current_closure = state.closures.lookup(frame.closure);
                        let upvalue = current_closure.upvalues[slot as usize].borrow();
                        if let Some(value) = upvalue.closed {
                            value
                        } else {
                            state.stack[upvalue.location]
                        }
                    };
                    state.push(value);
                }
                Instruction::Greater => {
                    self.binary_op(&frame, state, |a, b| a > b, |n| Value::Bool(n))?
                }
                Instruction::Jump(offset) => {
                    frame.ip += offset as usize;
                }
                Instruction::JumpIfFalse(offset) => {
                    if state.peek(0).is_falsy() {
                        frame.ip += offset as usize;
                    }
                }
                Instruction::Less => {
                    self.binary_op(&frame, state, |a, b| a < b, |n| Value::Bool(n))?
                }
                Instruction::Loop(offset) => {
                    frame.ip -= offset as usize + 1;
                }
                Instruction::Multiply => {
                    self.binary_op(&frame, state, |a, b| a * b, |n| Value::Number(n))?
                }
                Instruction::Negate => {
                    if let Value::Number(value) = state.peek(0) {
                        state.pop();
                        state.push(Value::Number(-value));
                    } else {
                        return Err(self.runtime_error(&frame, "Operand must be a number."));
                    }
                }
                Instruction::Nil => state.push(Value::Nil),
                Instruction::Not => {
                    let value = state.pop();
                    state.push(Value::Bool(value.is_falsy()));
                }
                Instruction::Pop => {
                    state.pop();
                }
                Instruction::Print => {
                    let value = state.pop();
                    if let Value::String(s) = value {
                        println!("{}", self.strings.lookup(s))
                    } else {
                        println!("{}", value);
                    }
                }
                Instruction::Return => {
                    let value = state.pop();
                    self.close_upvalues(state, frame.slot);
                    match state.frames.pop() {
                        Some(f) => {
                            state.stack.truncate(frame.slot);
                            state.push(value);
                            frame = f;
                            chunk = &self.functions.lookup(frame.function).chunk;
                        }
                        None => {
                            return Ok(());
                        }
                    }
                }
                Instruction::SetGlobal(index) => {
                    // TODO: refactor long indirection?
                    let name = chunk.read_string(index);
                    let value = state.peek(0);
                    if let None = state.globals.insert(name, value) {
                        state.globals.remove(&name);
                        let s = self.strings.lookup(name);
                        let msg = format!("Undefined variable '{}'.", s);
                        return Err(self.runtime_error(&frame, &msg));
                    }
                }
                Instruction::SetLocal(slot) => {
                    let i = slot as usize + frame.slot;
                    let value = state.peek(0);
                    state.stack[i] = value;
                }
                Instruction::SetUpvalue(slot) => {
                    // TODO: current_closure dance is repeated a lot.
                    let current_closure = state.closures.lookup(frame.closure);
                    let mut upvalue = current_closure.upvalues[slot as usize].borrow_mut();
                    let value = state.peek(0);
                    if let None = upvalue.closed {
                        state.stack[upvalue.location] = value;
                    } else {
                        upvalue.closed = Some(value);
                    }
                }
                Instruction::Substract => {
                    self.binary_op(&frame, state, |a, b| a - b, |n| Value::Number(n))?
                }
                Instruction::True => state.push(Value::Bool(true)),
            };
        }
    }

    fn close_upvalues(&self, state: &mut ExecutionState, last: usize) {
        let mut i = 0;
        while i != state.open_upvalues.len() {
            if state.open_upvalues[i].borrow().location >= last {
                let upvalue = state.open_upvalues.remove(i);
                let location = upvalue.borrow().location;
                upvalue.borrow_mut().closed = Some(state.stack[location]);
            } else {
                i += 1;
            }
        }
    }

    fn capture_value(
        &self,
        state: &mut ExecutionState,
        location: usize,
    ) -> Rc<RefCell<ObjUpvalue>> {
        for upvalue in state.open_upvalues.iter() {
            if upvalue.borrow().location == location {
                return Rc::clone(upvalue);
            }
        }
        let upvalue = ObjUpvalue::new(location);
        let upvalue = Rc::new(RefCell::new(upvalue));
        state.open_upvalues.push(Rc::clone(&upvalue));
        upvalue
    }

    fn call_value(
        &self,
        frame: CallFrame,
        state: &mut ExecutionState,
        arg_count: u8,
    ) -> Result<CallFrame, LoxError> {
        let callee = state.peek(arg_count as usize);
        match callee {
            Value::Closure(cid) => self.call(frame, state, cid, arg_count),
            Value::NativeFunction(native) => {
                let left = state.stack.len() - arg_count as usize;
                let result = native.0(&state.stack[left..]);
                state.push(result);
                Ok(frame)
            }
            _ => Err(self.runtime_error(&frame, "Can only call functions and classes.")),
        }
    }

    fn call(
        &self,
        frame: CallFrame,
        state: &mut ExecutionState,
        closure_id: ClosureId,
        arg_count: u8,
    ) -> Result<CallFrame, LoxError> {
        let closure = state.closures.lookup(closure_id);
        // TODO: Inefficient double lookup;
        let f = self.functions.lookup(closure.function);
        if (arg_count as usize) != f.arity {
            let msg = format!("Expected {} arguments but got {}.", f.arity, arg_count);
            Err(self.runtime_error(&frame, &msg))
        } else if state.frames.len() == MAX_FRAMES {
            Err(self.runtime_error(&frame, "Stack overflow."))
        } else {
            state.frames.push(frame);
            // TODO this looks cleaner with a constructor
            let mut frame = CallFrame::new(closure.function, closure_id);
            frame.slot = state.stack.len() - (arg_count as usize) - 1;
            Ok(frame)
        }
    }

    // TODO: Maybe return Err(RuntimeError) directly?
    fn runtime_error(&self, frame: &CallFrame, msg: &str) -> LoxError {
        eprintln!("{}", msg);
        let chunk = &self.functions.lookup(frame.function).chunk;
        let line = chunk.lines[frame.ip - 1];
        eprintln!("[line {}] in script", line);
        LoxError::RuntimeError
    }
}
