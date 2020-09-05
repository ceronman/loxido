use crate::{chunk::Chunk, strings::LoxString};

pub enum FunctionType {
    Function,
    Script,
}

pub type FunctionId = usize;

pub struct LoxFunction {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: LoxString,
}

impl LoxFunction {
    // TODO: default?
    pub fn new() -> Self {
        LoxFunction {
            arity: 0,
            chunk: Chunk::new(),
            name: 0,
        }
    }
}
#[derive(Default)]
pub struct Functions {
    functions: Vec<LoxFunction>
}

impl Functions {
    pub fn lookup(&self, id: FunctionId) -> &LoxFunction {
        &self.functions[id]
    }

    pub fn store(&mut self, function: LoxFunction) -> FunctionId {
        self.functions.push(function);
        self.functions.len() - 1
    }
}
