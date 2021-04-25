use std::fmt;

use crate::{allocator::Reference, chunk::Chunk, chunk::Value, vm::Vm};

#[derive(Clone, Copy)]
pub struct NativeFn(pub fn(&Vm, &[Value]) -> Value);

impl fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<fn>")
    }
}

impl PartialEq for NativeFn {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

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

#[derive(Debug)]
pub struct ObjUpvalue {
    pub location: usize, // TODO: Make this a proper type
    pub closed: Option<Value>,
}

impl ObjUpvalue {
    pub fn new(location: usize) -> Self {
        ObjUpvalue {
            location,
            closed: None,
        }
    }
}

#[derive(Debug)]
pub struct Closure {
    pub function: Reference<LoxFunction>,
    pub upvalues: Vec<Reference<ObjUpvalue>>,
}

impl Closure {
    pub fn new(function: Reference<LoxFunction>) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        } // TODO: use .with_capacity
    }
}
