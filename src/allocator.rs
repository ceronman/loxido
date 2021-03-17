use std::marker::PhantomData;
use std::{any::Any, collections::HashMap, fmt, hash};

use crate::{chunk::Value, closure::OpenUpvalues};

pub struct Reference<T> {
    index: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Copy for Reference<T> {}
impl<T> Eq for Reference<T> {}

impl<T> Clone for Reference<T> {
    #[inline]
    fn clone(&self) -> Reference<T> {
        *self
    }
}

impl<T> Default for Reference<T> {
    fn default() -> Self {
        Reference {
            index: 0,
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Display for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ref({})", self.index)
    }
}

impl<T> PartialEq for Reference<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl hash::Hash for Reference<String> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state)
    }
}

struct Empty;

struct ObjHeader {
    #[allow(dead_code)]
    is_marked: bool,
    obj: Box<dyn Any>
}

impl ObjHeader {
    fn empty() -> Self {
        ObjHeader {
            is_marked: false,
            obj: Box::new(Empty {})
        }
    }
}

#[derive(Default)]
pub struct Allocator {
    free_slots: Vec<usize>,
    objects: Vec<ObjHeader>,
    strings: HashMap<String, Reference<String>>,
}

impl Allocator {
    pub fn alloc<T: Any>(&mut self, object: T) -> Reference<T> {
        let entry = ObjHeader {
            is_marked: false,
            obj: Box::new(object)
        };
        let index = match self.free_slots.pop() {
            Some(i) => {
                self.objects[i] = entry;
                i
            }
            None => {
                self.objects.push(entry);
                self.objects.len() - 1
            }
        };
        let reference = Reference {
            index,
            _marker: PhantomData,
        };
        reference
    }

    pub fn alloc_gc<T: Any>(
        &mut self,
        object: T,
        _stack: &Vec<Value>,
        _globals: &HashMap<Reference<String>, Value>,
        _open_upvalues: &OpenUpvalues,
    ) -> Reference<T> {
        self.alloc(object)
    }

    pub fn intern_gc(
        &mut self,
        name: &str,
        _stack: &Vec<Value>,
        _globals: &HashMap<Reference<String>, Value>,
        _open_upvalues: &OpenUpvalues,
    ) -> Reference<String> {
        self.intern(name)
    }

    pub fn intern_owned(&mut self, name: String) -> Reference<String> {
        if let Some(&value) = self.strings.get(&name) {
            value
        } else {
            let reference = self.alloc(name.clone());
            self.strings.insert(name, reference);
            reference
        }
    }

    pub fn intern(&mut self, name: &str) -> Reference<String> {
        self.intern_owned(name.to_owned())
    }

    pub fn deref<T: Any>(&self, reference: Reference<T>) -> &T {
        self.objects[reference.index].obj.downcast_ref().unwrap()
    }

    #[allow(dead_code)]
    fn free<T: Any>(&mut self, reference: Reference<T>) {
        self.objects[reference.index] = ObjHeader::empty();
        self.free_slots.push(reference.index)
    }
}
