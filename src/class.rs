use std::{any::Any, mem};

use crate::allocator::{Allocator, Reference, Trace};

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
