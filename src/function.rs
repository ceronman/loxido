use std::fmt;

use crate::{
    allocator::{Allocator, Reference},
    chunk::Chunk,
    chunk::Value,
};

pub enum FunctionType {
    Function,
    Script,
}

#[derive(Clone, Copy)]
pub struct NativeFn(pub fn(&Allocator, &[Value]) -> Value);

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

// TODO: Only needed because of clone() done in Closure instruction
#[derive(Copy, Clone, Debug)]
pub struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

#[derive(Debug, Default)]
pub struct LoxFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: Reference<String>,
    pub upvalues: Vec<Upvalue>,
}
