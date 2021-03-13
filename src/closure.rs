use std::{cell::RefCell, rc::Rc};

use crate::{allocator::Reference, chunk::Value, function::LoxFunction};

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
    pub function: Reference<LoxFunction>,
    pub upvalues: Vec<Rc<RefCell<ObjUpvalue>>>,
}

impl Closure {
    pub fn new(function: Reference<LoxFunction>) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        } // TODO: use .with_capacity
    }
}

pub struct OpenUpvalues(Vec<Rc<RefCell<ObjUpvalue>>>);

impl OpenUpvalues {
    pub fn new(capacity: usize) -> Self {
        OpenUpvalues(Vec::with_capacity(capacity))
    }
    pub fn capture(&mut self, location: usize) -> Rc<RefCell<ObjUpvalue>> {
        for upvalue in self.0.iter() {
            if upvalue.borrow().location == location {
                return Rc::clone(upvalue);
            }
        }
        let upvalue = ObjUpvalue::new(location);
        let upvalue = Rc::new(RefCell::new(upvalue));
        self.0.push(Rc::clone(&upvalue));
        upvalue
    }

    pub fn close_upvalues(&mut self, stack: &Vec<Value>, last: usize) {
        let mut i = 0;
        while i != self.0.len() {
            if self.0[i].borrow().location >= last {
                let upvalue = self.0.remove(i);
                let location = upvalue.borrow().location;
                upvalue.borrow_mut().closed = Some(stack[location]);
            } else {
                i += 1;
            }
        }
    }
}
