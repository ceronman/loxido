use crate::{chunk::Chunk, strings::LoxString};

pub enum FunctionType {
    Function,
    Script,
}

pub struct LoxFunction {
    arity: u8,
    pub chunk: Chunk,
    name: LoxString,
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
