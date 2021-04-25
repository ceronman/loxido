use crate::{
    allocator::{Allocator, Reference, Trace},
    function::{BoundMethod, Closure, Instance, LoxClass, LoxFunction, NativeFn},
};
use std::{any::Any, collections::HashMap, fmt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    BoundMethod(Reference<BoundMethod>),
    Class(Reference<LoxClass>),
    Closure(Reference<Closure>),
    Function(Reference<LoxFunction>),
    Instance(Reference<Instance>),
    NativeFunction(NativeFn), // TODO: Make it garbage collected?
    Nil,
    Number(f64),
    String(Reference<String>),
}

// TODO: Use From<> or To<> to implement this to boolean?
impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Nil => true,
            Value::Bool(value) => !value,
            _ => false,
        }
    }
}

impl Trace for Value {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
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
    fn trace(&self, allocator: &mut Allocator) {
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

// TODO: maybe use type aliases for the inner values. e.g. u8 -> constant
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

#[derive(Debug, Default)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub lines: Vec<usize>,
}

impl Chunk {
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

    pub fn read_string(&self, index: u8) -> Reference<String> {
        if let Value::String(s) = self.read_constant(index) {
            s
        } else {
            panic!("Constant is not String!")
        }
    }

    #[cfg(feature = "debug_trace_execution")]
    pub fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, instruction) in self.code.iter().enumerate() {
            self.disassemble_instruction(instruction, offset);
        }
    }

    #[cfg(feature = "debug_trace_execution")]
    pub fn disassemble_instruction(&self, instruction: &Instruction, offset: usize) {
        print!("{:04} ", offset);
        let line = self.lines[offset];
        if offset > 0 && line == self.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Add => println!("OP_ADD"),
            Instruction::Class(i) => println!("OP_CLASS {}", i),
            Instruction::CloseUpvalue => println!("OP_CLOSE_UPVALUE"), // TODO: implement
            Instruction::Closure(i) => println!("OP_CLOSURE {}", i),   // TODO: implement
            Instruction::Constant(i) => self.disassemble_constant("OP_CONSTANT", *i),
            Instruction::Call(i) => println!("OP_CALL {}", i), // TODO: implement
            Instruction::DefineGlobal(i) => self.disassemble_constant("OP_DEFINE_GLOBAL", *i),
            Instruction::Divide => println!("OP_DIVIDE"),
            Instruction::Equal => println!("OP_EQUAL"),
            Instruction::False => println!("OP_FALSE"),
            Instruction::GetGlobal(i) => self.disassemble_constant("OP_GET_GLOBAL", *i),
            Instruction::GetLocal(i) => println!("OP_GET_LOCAL {}", i),
            Instruction::GetProperty(i) => println!("OP_GET_PROPERTY {}", i),
            Instruction::GetSuper(i) => println!("OP_GET_SUPER {}", i),
            Instruction::GetUpvalue(i) => println!("OP_GET_UPVALUE {}", i),
            Instruction::Greater => println!("OP_GREATER"),
            Instruction::Invoke((i, c)) => println!("OP_INVOKE {} {}", i, c),
            Instruction::Inherit => println!("OP_INHERIT"),
            Instruction::Jump(offset) => println!("OP_JUMP {}", offset), // TODO:
            Instruction::JumpIfFalse(offset) => println!("OP_JUMP_IF_FALSE {}", offset), // TODO:
            Instruction::Less => println!("OP_LESS"),
            Instruction::Loop(offset) => println!("OP_LOOP {}", offset), // TODO:
            Instruction::Method(i) => println!("OP_METHOD {}", i),
            Instruction::Multiply => println!("OP_MULTIPLY"),
            Instruction::Negate => println!("OP_NEGATE"),
            Instruction::Not => println!("OP_NOT"),
            Instruction::Nil => println!("OP_NIL"),
            Instruction::Pop => println!("OP_POP"),
            Instruction::Print => println!("OP_PRINT"),
            Instruction::Return => println!("OP_RETURN"),
            Instruction::SetGlobal(i) => self.disassemble_constant("OP_SET_GLOBAL", *i),
            Instruction::SetLocal(i) => println!("OP_SET_LOCAL {}", i), // TODO: implement
            Instruction::SetProperty(i) => println!("OP_SET_PROPERTY {}", i),
            Instruction::SetUpvalue(i) => println!("OP_SET_UPVALUE {}", i),
            Instruction::Substract => println!("OP_SUBSTRACT"),
            Instruction::SuperInvoke((i, c)) => println!("OP_SUPER_INVOKE {} {}", i, c),
            Instruction::True => println!("OP_TRUE"),
        }
    }

    #[cfg(feature = "debug_trace_execution")]
    fn disassemble_constant(&self, name: &str, index: u8) {
        let i = index as usize;
        let value = self.constants[i].clone();
        println!("{:<16} {:4} {}", name, index, value);
    }
}

pub type Table = HashMap<Reference<String>, Value>;
