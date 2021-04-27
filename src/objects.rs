use std::{any::Any, fmt, mem};

use crate::{
    chunk::Chunk,
    chunk::{Instruction, Table, Value},
    gc::{Gc, GcRef, GcTrace},
    vm::Vm,
};

impl GcTrace for String {
    fn format(&self, f: &mut fmt::Formatter, _gc: &Gc) -> fmt::Result {
        write!(f, "{}", self)
    }
    fn size(&self) -> usize {
        mem::size_of::<String>() + self.as_bytes().len()
    }
    fn trace(&self, _gc: &mut Gc) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone, Copy)]
pub struct NativeFunction(pub fn(&Vm, &[Value]) -> Value);

impl fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<fn>")
    }
}

impl PartialEq for NativeFunction {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FunctionUpvalue {
    pub index: u8,
    pub is_local: bool,
}

#[derive(Debug)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: GcRef<String>,
    pub upvalues: Vec<FunctionUpvalue>,
}

impl Function {
    pub fn new(name: GcRef<String>) -> Self {
        Self {
            arity: 0,
            chunk: Chunk::new(),
            name,
            upvalues: Vec::new(),
        }
    }
}

impl GcTrace for Function {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result {
        let name = gc.deref(self.name);
        if name.is_empty() {
            write!(f, "<script>")
        } else {
            write!(f, "<fn {}>", name)
        }
    }
    fn size(&self) -> usize {
        mem::size_of::<Function>()
            + self.upvalues.capacity() * mem::size_of::<FunctionUpvalue>()
            + self.chunk.code.capacity() * mem::size_of::<Instruction>()
            + self.chunk.constants.capacity() * mem::size_of::<Value>()
            + self.chunk.constants.capacity() * mem::size_of::<usize>()
    }
    fn trace(&self, gc: &mut Gc) {
        gc.mark_object(self.name);
        for &constant in &self.chunk.constants {
            gc.mark_value(constant);
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
pub struct Upvalue {
    pub location: usize,
    pub closed: Option<Value>,
}

impl Upvalue {
    pub fn new(location: usize) -> Self {
        Upvalue {
            location,
            closed: None,
        }
    }
}

impl GcTrace for Upvalue {
    fn format(&self, f: &mut fmt::Formatter, _gc: &Gc) -> fmt::Result {
        write!(f, "upvalue")
    }
    fn size(&self) -> usize {
        mem::size_of::<Upvalue>()
    }
    fn trace(&self, gc: &mut Gc) {
        if let Some(obj) = self.closed {
            gc.mark_value(obj)
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
    pub function: GcRef<Function>,
    pub upvalues: Vec<GcRef<Upvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<Function>) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        }
    }
}

impl GcTrace for Closure {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result {
        let function = gc.deref(self.function);
        function.format(f, gc)
    }
    fn size(&self) -> usize {
        mem::size_of::<Closure>() + self.upvalues.capacity() * mem::size_of::<GcRef<Upvalue>>()
    }
    fn trace(&self, gc: &mut Gc) {
        gc.mark_object(self.function);
        for &upvalue in &self.upvalues {
            gc.mark_object(upvalue);
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
pub struct Class {
    pub name: GcRef<String>,
    pub methods: Table,
}

impl Class {
    pub fn new(name: GcRef<String>) -> Self {
        Class {
            name,
            methods: Table::new(),
        }
    }
}

impl GcTrace for Class {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result {
        let name = gc.deref(self.name);
        write!(f, "{}", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<Class>()
    }
    fn trace(&self, gc: &mut Gc) {
        gc.mark_object(self.name);
        gc.mark_table(&self.methods);
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
    pub class: GcRef<Class>,
    pub fields: Table,
}

impl Instance {
    pub fn new(class: GcRef<Class>) -> Self {
        Instance {
            class,
            fields: Table::new(),
        }
    }
}

impl GcTrace for Instance {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result {
        let class = gc.deref(self.class);
        let name = gc.deref(class.name);
        write!(f, "{} instance", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<Instance>()
            + self.fields.capacity() * (mem::size_of::<GcRef<String>>() + mem::size_of::<Value>())
    }
    fn trace(&self, gc: &mut Gc) {
        gc.mark_object(self.class);
        gc.mark_table(&self.fields);
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
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result {
        let method = gc.deref(self.method);
        method.format(f, gc)
    }
    fn size(&self) -> usize {
        mem::size_of::<BoundMethod>()
    }
    fn trace(&self, gc: &mut Gc) {
        gc.mark_value(self.receiver);
        gc.mark_object(self.method);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
