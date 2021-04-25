use std::{any::Any, fmt, mem};

use crate::{
    allocator::{Allocator, Reference, Trace},
    chunk::Chunk,
    chunk::{Instruction, Value},
    vm::Vm,
};

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

impl Trace for LoxFunction {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let name = allocator.deref(self.name);
        if name.is_empty() {
            write!(f, "<script>")
        } else {
            write!(f, "<fn {}>", name)
        }
    }
    fn size(&self) -> usize {
        mem::size_of::<LoxFunction>()
            + self.upvalues.capacity() * mem::size_of::<Upvalue>()
            + self.chunk.code.capacity() * mem::size_of::<Instruction>()
            + self.chunk.constants.capacity() * mem::size_of::<Value>()
            + self.chunk.constants.capacity() * mem::size_of::<usize>()
    }
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.name);
        for &constant in &self.chunk.constants {
            allocator.mark_value(constant);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
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

impl Trace for ObjUpvalue {
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Allocator) -> fmt::Result {
        write!(f, "upvalue")
    }
    fn size(&self) -> usize {
        mem::size_of::<ObjUpvalue>()
    }
    fn trace(&self, allocator: &mut Allocator) {
        if let Some(obj) = self.closed {
            allocator.mark_value(obj)
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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

impl Trace for Closure {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let function = allocator.deref(self.function);
        function.format(f, allocator)
    }
    fn size(&self) -> usize {
        mem::size_of::<Closure>()
            + self.upvalues.capacity() * mem::size_of::<Reference<ObjUpvalue>>()
    }
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.function);
        for &upvalue in &self.upvalues {
            allocator.mark_object(upvalue);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
