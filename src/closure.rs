use crate::function::FunctionId;

pub struct Closure {
    pub function: FunctionId,
}

impl Closure {
    pub fn new(function: FunctionId) -> Self {
        Closure { function }
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
