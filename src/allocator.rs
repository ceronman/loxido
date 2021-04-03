use std::{any::type_name, collections::VecDeque, marker::PhantomData, mem};
use std::{any::Any, collections::HashMap, fmt, hash};

use fmt::Debug;

use crate::{
    chunk::Value,
    closure::{Closure, ObjUpvalue},
    function::LoxFunction,
};

pub trait Trace {
    fn trace(&self, allocator: &mut Allocator);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Trace for String {
    fn trace(&self, _allocator: &mut Allocator) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Trace for ObjUpvalue {
    fn trace(&self, allocator: &mut Allocator) {
        if let Some(obj) = self.closed {
            allocator.mark_value(obj)
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Trace for LoxFunction {
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.name);
        for &constant in &self.chunk.constants {
            allocator.mark_value(constant);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Trace for Closure {
    fn trace(&self, allocator: &mut Allocator) {
        allocator.mark_object(self.function);
        for &upvalue in &self.upvalues {
            allocator.mark_object(upvalue);
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Trace for Empty {
    fn trace(&self, _allocator: &mut Allocator) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

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

impl<T: Any> fmt::Display for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ref({}:{})", self.index, short_type_name::<T>())
    }
}

impl<T: Any> Debug for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ref({}:{})", self.index, short_type_name::<T>())
    }
}

fn short_type_name<T: Any>() -> &'static str {
    let full_name = type_name::<T>();
    full_name.split("::").last().unwrap()
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
    is_marked: bool,
    obj: Box<dyn Trace>,
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
    grey_stack: VecDeque<usize>, // TODO: Add proper capacity
}

impl Allocator {
    pub fn alloc<T: Trace + 'static + Debug>(&mut self, object: T) -> Reference<T> {
        #[cfg(feature = "debug_log_gc")]
        let repr = format!("{:?}", object);
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
        #[cfg(feature = "debug_log_gc")]
        println!(
            "alloc(id:{}, type:{}, val:{})",
            index,
            type_name::<T>(),
            repr
        );
        let reference = Reference {
            index,
            _marker: PhantomData,
        };
        reference
    }

    pub fn intern(&mut self, name: String) -> Reference<String> {
        if let Some(&value) = self.strings.get(&name) {
            value
        } else {
            let reference = self.alloc(name.clone());
            self.strings.insert(name, reference);
            reference
        }
    }

    pub fn deref<T: Any>(&self, reference: Reference<T>) -> &T {
        self.objects[reference.index]
            .obj
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    pub fn deref_mut<T: Any>(&mut self, reference: Reference<T>) -> &mut T {
        self.objects[reference.index]
            .obj
            .as_any_mut()
            .downcast_mut()
            .unwrap()
    }

    fn free(&mut self, index: usize) {
        #[cfg(feature = "debug_log_gc")]
        println!("free (id:{})", index,);
        self.objects[index] = ObjHeader::empty();
        self.free_slots.push(index)
    }

    pub fn collect_garbage(&mut self) {
        self.trace_references();
        self.sweep();
    }

    fn trace_references(&mut self) {
        while let Some(index) = self.grey_stack.pop_back() {
            self.blacken_object(index);
        }
    }

    fn blacken_object(&mut self, index: usize) {
        #[cfg(feature = "debug_log_gc")]
        println!("blacken(id:{})", index);

        // TODO: Think how to avoid this trick to please the borrow checker mig
        let header = mem::replace(&mut self.objects[index], ObjHeader::empty());
        header.obj.trace(self);
        self.objects[index] = header;
    }

    pub fn mark_value(&mut self, value: Value) {
        match value {
            Value::String(r) => self.mark_object(r),
            Value::Closure(r) => self.mark_object(r),
            Value::Function(r) => self.mark_object(r),
            _ => (),
        }
    }

    pub fn mark_object<T: Any + Debug>(&mut self, obj: Reference<T>) {
        if self.objects[obj.index].is_marked {
            return;
        }

        #[cfg(feature = "debug_log_gc")]
        println!(
            "mark(id:{}, type:{}, val:{:?})",
            obj.index,
            type_name::<T>(),
            obj
        );
        self.objects[obj.index].is_marked = true;
        self.grey_stack.push_back(obj.index);
    }

    fn sweep(&mut self) {
        for i in 0..self.objects.len() {
            if self.objects[i].is_marked {
                self.objects[i].is_marked = false;
            } else {
                self.free(i)
            }
        }
    }
}
