use std::{
    fmt::{self, Debug, Display},
    hint::unreachable_unchecked,
    ops::Deref,
};

use crate::{
    chunk::{Chunk, Value},
    gc::{GcObject, GcRef},
    table::Table,
    vm::Vm,
};

pub enum ObjectType {
    Function(Function),
    Closure(Closure),
    LoxString(LoxString),
    Upvalue(Upvalue),
    Class(Class),
    Instance(Instance),
    BoundMethod(BoundMethod),
}
pub struct LoxString {
    pub s: String,
    pub hash: usize,
}

impl LoxString {
    pub fn from_string(s: String) -> Self {
        let hash = LoxString::hash_string(&s);
        LoxString { s, hash }
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

impl GcObject for LoxString {
    fn into_object(self) -> ObjectType {
        ObjectType::LoxString(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::LoxString(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::LoxString(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

impl Display for LoxString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.s)
    }
}

impl Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::BoundMethod(value) => write!(f, "{}", value.method.function.deref()),
            ObjectType::Class(value) => write!(f, "{}", value.name.deref()),
            ObjectType::Closure(value) => write!(f, "{}", value.function.deref()),
            ObjectType::Function(value) => write!(f, "{}", value.name.deref()),
            ObjectType::Instance(value) => write!(f, "{} instance", value.class.name.deref()),
            ObjectType::LoxString(value) => write!(f, "{}", value.deref()),
            ObjectType::Upvalue(_) => write!(f, "upvalue"),
        }
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

pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: GcRef<LoxString>,
    pub upvalues: Vec<FunctionUpvalue>,
}

impl Function {
    pub fn new(name: GcRef<LoxString>) -> Self {
        Self {
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

impl GcObject for Function {
    fn into_object(self) -> ObjectType {
        ObjectType::Function(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::Function(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::Function(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

pub struct Upvalue {
    pub location: usize,
    pub closed: Option<Value>,
}

impl Upvalue {
    pub fn new(location: usize) -> Self {
        Upvalue {
            location,
            closed: None,
        }
    }
}

impl GcObject for Upvalue {
    fn into_object(self) -> ObjectType {
        ObjectType::Upvalue(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::Upvalue(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::Upvalue(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

pub struct Closure {
    pub function: GcRef<Function>,
    pub upvalues: Vec<GcRef<Upvalue>>,
}

impl Closure {
    pub fn new(function: GcRef<Function>) -> Self {
        Closure {
            function,
            upvalues: Vec::new(),
        }
    }
}

impl GcObject for Closure {
    fn into_object(self) -> ObjectType {
        ObjectType::Closure(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::Closure(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::Closure(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

pub struct Class {
    pub name: GcRef<LoxString>,
    pub methods: Table,
}

impl Class {
    pub fn new(name: GcRef<LoxString>) -> Self {
        Class {
            name,
            methods: Table::new(),
        }
    }
}

impl GcObject for Class {
    fn into_object(self) -> ObjectType {
        ObjectType::Class(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::Class(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::Class(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

pub struct Instance {
    pub class: GcRef<Class>,
    pub fields: Table,
}

impl Instance {
    pub fn new(class: GcRef<Class>) -> Self {
        Instance {
            class,
            fields: Table::new(),
        }
    }
}

impl GcObject for Instance {
    fn into_object(self) -> ObjectType {
        ObjectType::Instance(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::Instance(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::Instance(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

pub struct BoundMethod {
    pub receiver: Value,
    pub method: GcRef<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Value, method: GcRef<Closure>) -> Self {
        BoundMethod { receiver, method }
    }
}

impl GcObject for BoundMethod {
    fn into_object(self) -> ObjectType {
        ObjectType::BoundMethod(self)
    }

    fn unwrap_ref(obj: &ObjectType) -> &Self {
        match obj {
            ObjectType::BoundMethod(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self {
        match obj {
            ObjectType::BoundMethod(f) => f,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}
