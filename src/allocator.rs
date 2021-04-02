#[cfg(feature = "debug_log_gc")]
use std::any::type_name;

use std::{marker::PhantomData};
use std::{any::Any, collections::HashMap, fmt, hash};

use crate::{chunk::Value, closure::ObjUpvalue, vm::CallFrame};

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
    obj: Box<dyn Any>,
}

impl ObjHeader {
    fn empty() -> Self {
        ObjHeader {
            is_marked: false,
            obj: Box::new(Empty {}),
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
        #[cfg(feature = "debug_log_gc")]
        println!("Allocating object {}", type_name::<T>());
        let entry = ObjHeader {
            is_marked: false,
            obj: Box::new(object),
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
        stack: &Vec<Value>,
        globals: &HashMap<Reference<String>, Value>,
        frames: &Vec<CallFrame>,
        open_upvalues: &Vec<Reference<ObjUpvalue>>,
    ) -> Reference<T> {
        #[cfg(feature = "debug_stress_gc")]
        self.collect_garbage(stack, globals, frames, open_upvalues);
        self.alloc(object)
    }

    pub fn intern_gc(
        &mut self,
        name: &str,
        stack: &Vec<Value>,
        globals: &HashMap<Reference<String>, Value>,
        frames: &Vec<CallFrame>,
        open_upvalues: &Vec<Reference<ObjUpvalue>>,
    ) -> Reference<String> {
        #[cfg(feature = "debug_stress_gc")]
        self.collect_garbage(stack, globals, frames, open_upvalues);
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

    pub fn deref_mut<T: Any>(&mut self, reference: Reference<T>) -> &mut T {
        self.objects[reference.index].obj.downcast_mut().unwrap()
    }

    #[allow(dead_code)]
    fn free<T: Any>(&mut self, reference: Reference<T>) {
        #[cfg(feature = "debug_log_gc")]
        println!("Freeing object {}", type_name::<T>());
        self.objects[reference.index] = ObjHeader::empty();
        self.free_slots.push(reference.index)
    }

    fn collect_garbage(&mut self,
        stack: &Vec<Value>,
        globals: &HashMap<Reference<String>, Value>,
        frames: &Vec<CallFrame>,
        open_upvalues: &Vec<Reference<ObjUpvalue>>) {
        
        #[cfg(feature = "debug_log_gc")]
        println!("-- gc begin");

        self.mark_roots(stack, globals, frames, open_upvalues);

        #[cfg(feature = "debug_log_gc")]
        println!("-- gc end");
    }

    fn mark_roots(
        &mut self,
        stack: &Vec<Value>,
        globals: &HashMap<Reference<String>, Value>,
        frames: &Vec<CallFrame>,
        open_upvalues: &Vec<Reference<ObjUpvalue>>,
    ) {
        for &value in stack {
            self.mark_value(value);
        }

        for frame in frames.iter() {
            self.mark_object(frame.closure)
        }

        for &upvalue in open_upvalues {
            self.mark_object(upvalue);
        }

        self.mark_table(globals);
    }

    fn mark_value(&mut self, value: Value) {
        match value {
            Value::String(r) => self.mark_object(r),
            Value::Closure(r) => self.mark_object(r),
            Value::Function(r) => self.mark_object(r),
            _ => (),
        }
    }

    fn mark_object<T>(&mut self, obj: Reference<T>) {
        #[cfg(feature = "debug_log_gc")]
        println!("Marking object {}", type_name::<T>());
        self.objects[obj.index].is_marked = true;
    }

    fn mark_table(&mut self, globals: &HashMap<Reference<String>, Value>) {
        for (&k, &v) in globals {
            self.mark_object(k);
            self.mark_value(v);
        }
    }
}
