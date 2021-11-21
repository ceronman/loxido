use std::{
    fmt::{self, Debug, Display},
    ops::Deref,
};

use crate::{
    chunk::{Chunk, Value},
    gc::{GcObject, GcRef},
    table::Table,
    vm::Vm,
};

#[derive(Debug)]
pub enum ObjectType {
    Function,
    Closure,
    LoxString,
    Upvalue,
    Class,
    Instance,
    BoundMethod,
}

#[repr(C)]
pub struct LoxString {
    pub header: GcObject,
    pub s: String,
    pub hash: usize,
}

impl LoxString {
    pub fn from_string(s: String) -> Self {
        let hash = LoxString::hash_string(&s);
        LoxString {
            header: GcObject::new(ObjectType::LoxString),
            s,
            hash,
        }
    }

    fn hash_string(s: &str) -> usize {
        let mut hash: usize = 2166136261;
        for b in s.bytes() {
            hash ^= b as usize;
            hash = hash.wrapping_mul(16777619);
        }
        hash
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.s)
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

#[repr(C)]
pub struct Function {
    pub header: GcObject,
    pub arity: usize,
    pub chunk: Chunk,
    pub name: GcRef<LoxString>,
    pub upvalues: Vec<FunctionUpvalue>,
}

impl Function {
    pub fn new(name: GcRef<LoxString>) -> Self {
        Self {
            header: GcObject::new(ObjectType::Function),
            arity: 0,
            chunk: Chunk::new(),
            name,
            upvalues: Vec::new(),
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name.deref().s == "script" {
            write!(f, "<script>")
        } else {
            write!(f, "<fn {}>", self.name.deref())
        }
    }
}

#[repr(C)]
pub struct Upvalue {
    pub header: GcObject,
    pub location: usize,
    pub closed: Option<Value>,
}

impl Upvalue {
    pub fn new(location: usize) -> Self {
        Upvalue {
            header: GcObject::new(ObjectType::Upvalue),
            location,
            closed: None,
        }
    }
}

impl Display for Upvalue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "upvalue")
    }
}

#[repr(C)]
pub struct Closure {
    pub header: GcObject,
    pub function: GcRef<Function>,
    pub upvalues: Vec<GcRef<Upvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<Function>) -> Self {
        Closure {
            header: GcObject::new(ObjectType::Closure),
            function,
            upvalues: Vec::new(),
        }
    }
}

impl Display for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.function.deref())
    }
}

#[repr(C)]
pub struct Class {
    pub header: GcObject,
    pub name: GcRef<LoxString>,
    pub methods: Table,
}

impl Class {
    pub fn new(name: GcRef<LoxString>) -> Self {
        Class {
            header: GcObject::new(ObjectType::Class),
            name,
            methods: Table::new(),
        }
    }
}

impl Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name.deref())
    }
}

#[repr(C)]
pub struct Instance {
    pub header: GcObject,
    pub class: GcRef<Class>,
    pub fields: Table,
}

impl Instance {
    pub fn new(class: GcRef<Class>) -> Self {
        Instance {
            header: GcObject::new(ObjectType::Instance),
            class,
            fields: Table::new(),
        }
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.class.name.deref())
    }
}

#[repr(C)]
pub struct BoundMethod {
    pub header: GcObject,
    pub receiver: Value,
    pub method: GcRef<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Value, method: GcRef<Closure>) -> Self {
        BoundMethod {
            header: GcObject::new(ObjectType::BoundMethod),
            receiver,
            method,
        }
    }
}

impl Display for BoundMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.method.function.deref())
    }
}
