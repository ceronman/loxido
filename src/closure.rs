use crate::{allocator::Reference, chunk::Value, function::LoxFunction};

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
