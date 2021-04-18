use std::{any::Any, fmt, mem};

use crate::{
    allocator::{Allocator, Reference, Trace},
    chunk::{Table, Value},
    closure::Closure,
};

#[derive(Debug)]
pub struct LoxClass {
    pub name: Reference<String>,
    pub methods: Table,
}

impl LoxClass {
    pub fn new(name: Reference<String>) -> Self {
        LoxClass {
            name,
            methods: Table::new(),
        }
    }
}

impl Trace for LoxClass {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let name = allocator.deref(self.name);
        write!(f, "{}", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<LoxClass>()
    }
    fn trace(&self, allocator: &mut Allocator) {
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
    pub class: Reference<LoxClass>,
    fields: Table,
}

impl Instance {
    pub fn new(class: Reference<LoxClass>) -> Self {
        Instance {
            class,
            fields: Table::new(),
        }
    }

    // TODO: Move these to Table
    pub fn get_property(&self, name: Reference<String>) -> Option<Value> {
        self.fields.get(&name).map(|&v| v)
    }

    pub fn set_property(&mut self, name: Reference<String>, value: Value) {
        self.fields.insert(name, value);
    }
}

impl Trace for Instance {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let class = allocator.deref(self.class);
        let name = allocator.deref(class.name);
        write!(f, "{} instance", name)
    }
    fn size(&self) -> usize {
        mem::size_of::<Instance>()
            + self.fields.capacity()
                * (mem::size_of::<Reference<String>>() + mem::size_of::<Value>())
    }
    fn trace(&self, allocator: &mut Allocator) {
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
    pub method: Reference<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Value, method: Reference<Closure>) -> Self {
        BoundMethod { receiver, method }
    }
}

impl Trace for BoundMethod {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let method = allocator.deref(self.method);
        method.format(f, allocator)
    }
    fn size(&self) -> usize {
        mem::size_of::<BoundMethod>()
    }
    fn trace(&self, allocator: &mut Allocator) {
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
