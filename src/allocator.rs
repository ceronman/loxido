use std::{any::type_name, collections::VecDeque, marker::PhantomData, mem};
use std::{any::Any, collections::HashMap, fmt, hash};

use fmt::Debug;

use crate::{
    chunk::{Instruction, Table, Value},
    closure::{Closure, ObjUpvalue},
    function::{LoxFunction, Upvalue},
};

pub trait Trace {
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result;
    fn size(&self) -> usize;
    fn trace(&self, allocator: &mut Allocator);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
pub struct TraceFormatter<'allocator, T: Trace> {
    allocator: &'allocator Allocator,
    object: T
}

impl<'allocator, T: Trace> TraceFormatter<'allocator, T> {
    pub fn new(object: T, allocator: &'allocator Allocator) -> Self {
        TraceFormatter {
            object,
            allocator
        }
    }
}

impl<'allocator, T: Trace> fmt::Display for TraceFormatter<'allocator, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.object.format(f, self.allocator)
    }
}

impl Trace for String {
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Allocator) -> fmt::Result {
        write!(f, "{}", self)
    }
    fn size(&self) -> usize {
        mem::size_of::<String>() + self.as_bytes().len()
    }
    fn trace(&self, _allocator: &mut Allocator) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Trace for ObjUpvalue {
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Allocator) -> fmt::Result {
        write!(f, "upvalue")
    }
    fn size(&self) -> usize {
        mem::size_of::<ObjUpvalue>()
    }
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
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let name = allocator.deref(self.name);
        if name.is_empty() {
            write!(f, "<script>")
        } else {
            write!(f, "<fn {}>", name)
        }
    }
    fn size(&self) -> usize {
        mem::size_of::<LoxFunction>()
            + self.upvalues.capacity() * mem::size_of::<Upvalue>()
            + self.chunk.code.capacity() * mem::size_of::<Instruction>()
            + self.chunk.constants.capacity() * mem::size_of::<Value>()
            + self.chunk.constants.capacity() * mem::size_of::<usize>()
    }
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
    fn format(&self, f: &mut fmt::Formatter, allocator: &Allocator) -> fmt::Result {
        let function = allocator.deref(self.function);
        function.format(f, allocator)
    }
    fn size(&self) -> usize {
        mem::size_of::<Closure>()
            + self.upvalues.capacity() * mem::size_of::<Reference<ObjUpvalue>>()
    }
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
    fn format(&self, f: &mut fmt::Formatter, _allocator: &Allocator) -> fmt::Result {
        write!(f, "<empty>")
    }
    fn size(&self) -> usize {
        0
    }
    fn trace(&self, _allocator: &mut Allocator) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// TODO: Make T: Trace?
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
    size: usize,
    obj: Box<dyn Trace>,
}

impl ObjHeader {
    fn empty() -> Self {
        ObjHeader {
            is_marked: false,
            size: 0,
            obj: Box::new(Empty {}),
        }
    }
}

pub struct Allocator {
    bytes_allocated: usize,
    next_gc: usize,
    free_slots: Vec<usize>,
    objects: Vec<ObjHeader>,
    strings: HashMap<String, Reference<String>>,
    grey_stack: VecDeque<usize>,
}

impl Allocator {
    const GC_HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Allocator {
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
            free_slots: Vec::new(),
            objects: Vec::new(),
            strings: HashMap::new(),
            grey_stack: VecDeque::new(), // TODO: Add proper capacities
        }
    }

    pub fn alloc<T: Trace + 'static + Debug>(&mut self, object: T) -> Reference<T> {
        #[cfg(feature = "debug_log_gc")]
        let repr = format!("{:?}", object)
            .chars()
            .into_iter()
            .take(32)
            .collect::<String>();
        let size = object.size() + mem::size_of::<ObjHeader>();
        self.bytes_allocated += size;
        let entry = ObjHeader {
            is_marked: false,
            size: size,
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
            "alloc(id:{}, type:{}: repr: {}, b:{}, t:{})",
            index,
            type_name::<T>(),
            repr,
            self.bytes_allocated,
            self.next_gc,
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
        // .expect(&format!("Reference {} not found", reference.index))
    }

    pub fn deref_mut<T: Any>(&mut self, reference: Reference<T>) -> &mut T {
        self.objects[reference.index]
            .obj
            .as_any_mut()
            .downcast_mut()
            .unwrap()
        // .expect(&format!("Reference {} not found", reference.index))
    }

    fn free(&mut self, index: usize) {
        #[cfg(feature = "debug_log_gc")]
        println!("free (id:{})", index,);
        let old = mem::replace(&mut self.objects[index], ObjHeader::empty());
        self.bytes_allocated -= old.size;
        self.free_slots.push(index)
    }

    pub fn collect_garbage(&mut self) {
        #[cfg(feature = "debug_log_gc")]
        let before = self.bytes_allocated;

        self.trace_references();
        self.remove_white_strings();
        self.sweep();
        self.next_gc = self.bytes_allocated * Allocator::GC_HEAP_GROW_FACTOR;

        #[cfg(feature = "debug_log_gc")]
        println!(
            "collected {} bytes (from {} to {}) next at {}\n",
            before - self.bytes_allocated,
            before,
            self.bytes_allocated,
            self.next_gc
        );
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
        value.trace(self);
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

    pub fn mark_table(&mut self, table: &Table) {
        for (&k, &v) in table {
            self.mark_object(k);
            self.mark_value(v);
        }
    }

    #[cfg(feature = "debug_stress_gc")]
    pub fn should_gc(&self) -> bool {
        true
    }

    #[cfg(not(feature = "debug_stress_gc"))]
    pub fn should_gc(&self) -> bool {
        self.bytes_allocated > self.next_gc
    }

    fn sweep(&mut self) {
        for i in 0..self.objects.len() {
            if let Some(_) = self.objects[i].obj.as_any().downcast_ref::<Empty>() {
                continue;
            }
            if self.objects[i].is_marked {
                self.objects[i].is_marked = false;
            } else {
                self.free(i)
            }
        }
    }

    fn remove_white_strings(&mut self) {
        let strings = &mut self.strings;
        let objects = &self.objects;
        strings.retain(|_k, v| objects[v.index].is_marked);
    }
}
