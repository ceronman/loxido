use std::fmt;

use crate::{allocator::Reference, chunk::Chunk, chunk::Value};

pub enum FunctionType {
    Function,
    Script,
}
#[derive(Clone, Copy)]
pub struct NativeFn(pub fn(&[Value]) -> Value);

impl fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<fn>")
    }
}

impl PartialEq for NativeFn {
    fn eq(&self, _other: &Self) -> bool {
        false // TODO: Implement
    }
}

pub struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

#[derive(Default)]
pub struct LoxFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Reference<String>,
    pub upvalues: Vec<Upvalue>,
}
