use std::{any::Any, fmt, mem};

use crate::{
    chunk::Chunk,
    chunk::{Instruction, Table, Value},
    gc::{Gc, GcRef, GcTrace},
    vm::Vm,
};

impl GcTrace for String {
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Gc) -> fmt::Result {
        write!(f, "{}", self)
    }
    fn size(&self) -> usize {
        mem::size_of::<String>() + self.as_bytes().len()
    }
    fn trace(&self, _allocator: &mut Gc) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

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

#[derive(Debug)]
pub struct LoxFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: GcRef<String>,
    pub upvalues: Vec<Upvalue>,
}

impl LoxFunction {
    pub fn new(name: GcRef<String>) -> Self {
        Self {
            arity: 0,
            chunk: Chunk::new(),
            name,
            upvalues: Vec::new(),
        }
    }
}

impl GcTrace for LoxFunction {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
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
    fn trace(&self, allocator: &mut Gc) {
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

impl GcTrace for ObjUpvalue {
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Gc) -> fmt::Result {
        write!(f, "upvalue")
    }
    fn size(&self) -> usize {
        mem::size_of::<ObjUpvalue>()
    }
    fn trace(&self, allocator: &mut Gc) {
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
    pub function: GcRef<LoxFunction>,
    pub upvalues: Vec<GcRef<ObjUpvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<LoxFunction>) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        } // TODO: use .with_capacity
    }
}

impl GcTrace for Closure {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
        let function = allocator.deref(self.function);
        function.format(f, allocator)
    }
    fn size(&self) -> usize {
        mem::size_of::<Closure>() + self.upvalues.capacity() * mem::size_of::<GcRef<ObjUpvalue>>()
    }
    fn trace(&self, allocator: &mut Gc) {
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

#[derive(Debug)]
pub struct LoxClass {
    pub name: GcRef<String>,
    pub methods: Table,
}

impl LoxClass {
    pub fn new(name: GcRef<String>) -> Self {
        LoxClass {
            name,
            methods: Table::new(),
        }
    }
}

impl GcTrace for LoxClass {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
        let name = allocator.deref(self.name);
        write!(f, "{}", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<LoxClass>()
    }
    fn trace(&self, allocator: &mut Gc) {
        allocator.mark_object(self.name);
        allocator.mark_table(&self.methods);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Debug)]
pub struct Instance {
    pub class: GcRef<LoxClass>,
    fields: Table,
}

impl Instance {
    pub fn new(class: GcRef<LoxClass>) -> Self {
        Instance {
            class,
            fields: Table::new(),
        }
    }

    // TODO: Move these to Table
    pub fn get_property(&self, name: GcRef<String>) -> Option<Value> {
        self.fields.get(&name).copied()
    }

    pub fn set_property(&mut self, name: GcRef<String>, value: Value) {
        self.fields.insert(name, value);
    }
}

impl GcTrace for Instance {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
        let class = allocator.deref(self.class);
        let name = allocator.deref(class.name);
        write!(f, "{} instance", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<Instance>()
            + self.fields.capacity() * (mem::size_of::<GcRef<String>>() + mem::size_of::<Value>())
    }
    fn trace(&self, allocator: &mut Gc) {
        allocator.mark_object(self.class);
        allocator.mark_table(&self.fields);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Debug)]
pub struct BoundMethod {
    pub receiver: Value,
    pub method: GcRef<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Value, method: GcRef<Closure>) -> Self {
        BoundMethod { receiver, method }
    }
}

impl GcTrace for BoundMethod {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Gc) -> fmt::Result {
        let method = allocator.deref(self.method);
        method.format(f, allocator)
    }
    fn size(&self) -> usize {
        mem::size_of::<BoundMethod>()
    }
    fn trace(&self, allocator: &mut Gc) {
        allocator.mark_value(self.receiver);
        allocator.mark_object(self.method);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
