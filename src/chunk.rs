use crate::{
    gc::{Gc, GcRef, GcTrace},
    objects::{BoundMethod, Closure, Instance, LoxClass, LoxFunction, NativeFn},
};
use std::{any::Any, collections::HashMap, fmt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    BoundMethod(GcRef<BoundMethod>),
    Class(GcRef<LoxClass>),
    Closure(GcRef<Closure>),
    Function(GcRef<LoxFunction>),
    Instance(GcRef<Instance>),
    NativeFunction(NativeFn),
    Nil,
    Number(f64),
    String(GcRef<String>),
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Nil => true,
            Value::Bool(value) => !value,
            _ => false,
        }
    }
}

impl GcTrace for Value {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
        match self {
            Value::Bool(value) => write!(f, "{}", value),
            Value::BoundMethod(value) => allocator.deref(*value).format(f, allocator),
            Value::Class(value) => allocator.deref(*value).format(f, allocator),
            Value::Closure(value) => allocator.deref(*value).format(f, allocator),
            Value::Function(value) => allocator.deref(*value).format(f, allocator),
            Value::Instance(value) => allocator.deref(*value).format(f, allocator),
            Value::NativeFunction(_) => write!(f, "<native fn>"),
            Value::Nil => write!(f, "nil"),
            Value::Number(value) => {
                // Hack to be able to print -0.0 as -0. Check https://github.com/rust-lang/rfcs/issues/1074
                if *value == 0.0f64 && value.is_sign_negative() {
                    write!(f, "-{}", value)
                } else {
                    write!(f, "{}", value)
                }
            }
            Value::String(value) => allocator.deref(*value).format(f, allocator),
        }
    }
    fn size(&self) -> usize {
        0
    }
    fn trace(&self, allocator: &mut Gc) {
        match self {
            Value::BoundMethod(value) => allocator.mark_object(*value),
            Value::Class(value) => allocator.mark_object(*value),
            Value::Closure(value) => allocator.mark_object(*value),
            Value::Function(value) => allocator.mark_object(*value),
            Value::Instance(value) => allocator.mark_object(*value),
            Value::String(value) => allocator.mark_object(*value),
            _ => (),
        }
    }
    fn as_any(&self) -> &dyn Any {
        panic!("Value should not be allocated")
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!("Value should not be allocated")
    }
}

pub type Table = HashMap<GcRef<String>, Value>;

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    Add,
    Call(u8),
    Class(u8),
    CloseUpvalue,
    Closure(u8),
    Constant(u8),
    DefineGlobal(u8),
    Divide,
    Equal,
    False,
    GetGlobal(u8),
    GetLocal(u8),
    GetProperty(u8),
    GetSuper(u8),
    GetUpvalue(u8),
    Greater,
    Inherit,
    Invoke((u8, u8)),
    Jump(u16),
    JumpIfFalse(u16),
    Less,
    Loop(u16),
    Method(u8),
    Multiply,
    Negate,
    Nil,
    Not,
    Pop,
    Print,
    Return,
    SetGlobal(u8),
    SetLocal(u8),
    SetProperty(u8),
    SetUpvalue(u8),
    Substract,
    SuperInvoke((u8, u8)),
    True,
}

#[derive(Debug)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }
    pub fn write(&mut self, instruction: Instruction, line: usize) -> usize {
        self.code.push(instruction);
        self.lines.push(line);
        self.code.len() - 1
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn read_constant(&self, index: u8) -> Value {
        self.constants[index as usize]
    }

    pub fn read_string(&self, index: u8) -> GcRef<String> {
        if let Value::String(s) = self.read_constant(index) {
            s
        } else {
            panic!("Constant is not String!")
        }
    }
}

#[cfg(feature = "debug_trace_execution")]
pub struct Disassembler<'vm> {
    allocator: &'vm Allocator,
    chunk: &'vm Chunk,
    stack: Option<&'vm Vec<Value>>,
}

#[cfg(feature = "debug_trace_execution")]
impl<'vm> Disassembler<'vm> {
    pub fn new(
        allocator: &'vm Allocator,
        chunk: &'vm Chunk,
        stack: Option<&'vm Vec<Value>>,
    ) -> Self {
        Disassembler {
            allocator,
            chunk,
            stack,
        }
    }

    pub fn disassemble(&self, name: &str) {
        println!("== BEGIN {} ==", name);
        for (offset, instruction) in self.chunk.code.iter().enumerate() {
            self.instruction(instruction, offset);
        }
        println!("== END {} ==", name);
        println!();
    }

    pub fn instruction(&self, instruction: &Instruction, offset: usize) {
        self.stack();
        print!("{:04} ", offset);
        let line = self.chunk.lines[offset];
        if offset > 0 && line == self.chunk.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Add => println!("OP_ADD"),
            Instruction::Class(c) => self.const_instruction("OP_CLASS", *c),
            Instruction::CloseUpvalue => println!("OP_CLOSE_UPVALUE"),
            Instruction::Closure(c) => self.const_instruction("OP_CLOSURE", *c),
            Instruction::Constant(c) => self.const_instruction("OP_CONSTANT", *c),
            Instruction::Call(args) => println!("{:<16} {:4}", "OP_CALL", *args),
            Instruction::DefineGlobal(c) => self.const_instruction("OP_DEFINE_GLOBAL", *c),
            Instruction::Divide => println!("OP_DIVIDE"),
            Instruction::Equal => println!("OP_EQUAL"),
            Instruction::False => println!("OP_FALSE"),
            Instruction::GetGlobal(c) => self.const_instruction("OP_GET_GLOBAL", *c),
            Instruction::GetLocal(s) => self.slot_instruction("OP_GET_LOCAL", *s),
            Instruction::GetProperty(c) => self.const_instruction("OP_GET_PROPERTY", *c),
            Instruction::GetSuper(c) => self.const_instruction("OP_GET_SUPER", *c),
            Instruction::GetUpvalue(s) => self.slot_instruction("OP_GET_UPVALUE", *s),
            Instruction::Greater => println!("OP_GREATER"),
            Instruction::Invoke((c, args)) => self.invoke_instruction("OP_INVOKE", *c, *args),
            Instruction::Inherit => println!("OP_INHERIT"),
            Instruction::Jump(offset) => self.jump_instruction("OP_JUMP", *offset),
            Instruction::JumpIfFalse(offset) => self.jump_instruction("OP_JUMP_IF_FALSE", *offset),
            Instruction::Less => println!("OP_LESS"),
            Instruction::Loop(offset) => self.jump_instruction("OP_LOOP", *offset),
            Instruction::Method(c) => self.const_instruction("OP_METHOD", *c),
            Instruction::Multiply => println!("OP_MULTIPLY"),
            Instruction::Negate => println!("OP_NEGATE"),
            Instruction::Not => println!("OP_NOT"),
            Instruction::Nil => println!("OP_NIL"),
            Instruction::Pop => println!("OP_POP"),
            Instruction::Print => println!("OP_PRINT"),
            Instruction::Return => println!("OP_RETURN"),
            Instruction::SetGlobal(c) => self.const_instruction("OP_SET_GLOBAL", *c),
            Instruction::SetLocal(s) => self.slot_instruction("OP_SET_LOCAL", *s),
            Instruction::SetProperty(c) => self.const_instruction("OP_SET_PROPERTY", *c),
            Instruction::SetUpvalue(s) => self.slot_instruction("OP_SET_UPVALUE", *s),
            Instruction::Substract => println!("OP_SUBSTRACT"),
            Instruction::SuperInvoke((c, args)) => {
                self.invoke_instruction("OP_SUPER_INVOKE", *c, *args)
            }
            Instruction::True => println!("OP_TRUE"),
        }
    }

    fn const_instruction(&self, instruction: &str, constant_index: u8) {
        let value = self.chunk.constants[constant_index as usize];
        println!(
            "{:<16} {:4} ({})",
            instruction,
            constant_index,
            crate::allocator::TraceFormatter::new(value, self.allocator)
        );
    }

    fn slot_instruction(&self, instruction: &str, slot: u8) {
        println!("{:<16} {:4}", instruction, slot);
    }

    fn jump_instruction(&self, instruction: &str, offset: u16) {
        println!("{:<16} {:4}", instruction, offset);
    }

    fn invoke_instruction(&self, instruction: &str, constant_index: u8, args: u8) {
        let value = self.chunk.constants[constant_index as usize];
        println!(
            "{:<16} {:4} ({}) {}",
            instruction,
            constant_index,
            crate::allocator::TraceFormatter::new(value, self.allocator),
            args
        );
    }

    fn stack(&self) {
        if let Some(stack) = self.stack {
            print!(" S: ");
            for &value in stack.iter() {
                print!(
                    "[{}]",
                    crate::allocator::TraceFormatter::new(value, self.allocator)
                );
            }
            println!();
        }
    }
}
