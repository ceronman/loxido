use std::{cell::RefCell, rc::Rc};

use crate::{chunk::Value, function::FunctionId};

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

pub struct Closure {
    pub function: FunctionId,
    pub upvalues: Vec<Rc<RefCell<ObjUpvalue>>>,
}

impl Closure {
    pub fn new(function: FunctionId) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        } // TODO: use .with_capacity
    }
}

pub type ClosureId = usize;

// TODO: Refactor into a generic
#[derive(Default)]
pub struct Closures {
    closures: Vec<Closure>,
}

impl Closures {
    pub fn lookup(&self, id: ClosureId) -> &Closure {
        &self.closures[id]
    }

    pub fn store(&mut self, closure: Closure) -> ClosureId {
        self.closures.push(closure);
        self.closures.len() - 1
    }
}
