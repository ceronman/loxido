use std::{any::Any, collections::HashMap, mem};

use crate::{
    allocator::{Allocator, Reference, Trace},
    chunk::Value,
};

#[derive(Debug)]
pub struct LoxClass {
    name: Reference<String>,
}

impl LoxClass {
    pub fn new(name: Reference<String>) -> Self {
        LoxClass { name }
    }
}

impl Trace for LoxClass {
    fn size(&self) -> usize {
        mem::size_of::<LoxClass>()
    }
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.name);
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
    class: Reference<LoxClass>,
    fields: HashMap<Reference<String>, Value>,
}

impl Instance {
    pub fn new(class: Reference<LoxClass>) -> Self {
        Instance {
            class,
            fields: HashMap::new(),
        }
    }
}

impl Trace for Instance {
    fn size(&self) -> usize {
        mem::size_of::<Instance>()
            + self.fields.capacity()
                * (mem::size_of::<Reference<String>>() + mem::size_of::<Value>())
    }
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.class);
        // TODO: Duplicated code with mark roots
        for (&k, &v) in &self.fields {
            allocator.mark_object(k);
            allocator.mark_value(v)
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
