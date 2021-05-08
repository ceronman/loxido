use std::{any::type_name, collections::VecDeque, marker::PhantomData, mem};
use std::{any::Any, collections::HashMap, fmt, hash};

use fmt::Debug;

use crate::chunk::{Table, Value};

pub trait GcTrace {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result;
    fn size(&self) -> usize;
    fn trace(&self, gc: &mut Gc);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
pub struct GcTraceFormatter<'gc, T: GcTrace> {
    gc: &'gc Gc,
    object: T,
}

impl<'gc, T: GcTrace> GcTraceFormatter<'gc, T> {
    pub fn new(object: T, gc: &'gc Gc) -> Self {
        GcTraceFormatter { object, gc }
    }
}

impl<'gc, T: GcTrace> fmt::Display for GcTraceFormatter<'gc, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.object.format(f, self.gc)
    }
}

pub struct GcRef<T: GcTrace> {
    index: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: GcTrace> Copy for GcRef<T> {}
impl<T: GcTrace> Eq for GcRef<T> {}

impl<T: GcTrace> Clone for GcRef<T> {
    #[inline]
    fn clone(&self) -> GcRef<T> {
        *self
    }
}

impl<T: GcTrace> Debug for GcRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let full_name = type_name::<T>();
        full_name.split("::").last().unwrap();
        write!(f, "ref({}:{})", self.index, full_name)
    }
}

impl<T: GcTrace> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl hash::Hash for GcRef<String> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state)
    }
}

struct GcObjectHeader {
    is_marked: bool,
    size: usize,
    obj: Box<dyn GcTrace>,
}

pub struct Gc {
    bytes_allocated: usize,
    next_gc: usize,
    free_slots: Vec<usize>,
    objects: Vec<Option<GcObjectHeader>>,
    strings: HashMap<String, GcRef<String>>,
    grey_stack: VecDeque<usize>,
}

impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Gc {
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
            free_slots: Vec::new(),
            objects: Vec::new(),
            strings: HashMap::new(),
            grey_stack: VecDeque::new(),
        }
    }

    pub fn alloc<T: GcTrace + 'static + Debug>(&mut self, object: T) -> GcRef<T> {
        #[cfg(feature = "debug_log_gc")]
        let repr = format!("{:?}", object)
            .chars()
            .into_iter()
            .take(32)
            .collect::<String>();
        let size = object.size() + mem::size_of::<GcObjectHeader>();
        self.bytes_allocated += size;
        let entry = GcObjectHeader {
            is_marked: false,
            size,
            obj: Box::new(object),
        };
        let index = match self.free_slots.pop() {
            Some(i) => {
                self.objects[i] = Some(entry);
                i
            }
            None => {
                self.objects.push(Some(entry));
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
        GcRef {
            index,
            _marker: PhantomData,
        }
    }

    pub fn intern(&mut self, name: String) -> GcRef<String> {
        if let Some(&value) = self.strings.get(&name) {
            value
        } else {
            let reference = self.alloc(name.clone());
            self.strings.insert(name, reference);
            reference
        }
    }

    pub fn deref<T: GcTrace + 'static>(&self, reference: GcRef<T>) -> &T {
        self.objects[reference.index]
            .as_ref()
            .unwrap()
            .obj
            .as_any()
            .downcast_ref()
            .unwrap_or_else(|| panic!("Reference {} not found", reference.index))
    }

    pub fn deref_mut<T: GcTrace + 'static>(&mut self, reference: GcRef<T>) -> &mut T {
        self.objects[reference.index]
            .as_mut()
            .unwrap()
            .obj
            .as_any_mut()
            .downcast_mut()
            .unwrap_or_else(|| panic!("Reference {} not found", reference.index))
    }

    fn free(&mut self, index: usize) {
        #[cfg(feature = "debug_log_gc")]
        println!("free (id:{})", index,);
        if let Some(old) = self.objects[index].take() {       
            self.bytes_allocated -= old.size;
            self.free_slots.push(index)
        } else {
            panic!("Double free on {}", index)
        }
    }

    pub fn collect_garbage(&mut self) {
        #[cfg(feature = "debug_log_gc")]
        let before = self.bytes_allocated;

        self.trace_references();
        self.remove_white_strings();
        self.sweep();
        self.next_gc = self.bytes_allocated * Gc::HEAP_GROW_FACTOR;

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

        // Hack to trick the borrow checker to be able to call trace on an element.
        let object = self.objects[index].take();
        object.as_ref().unwrap().obj.trace(self);
        self.objects[index] = object;
    }

    pub fn mark_value(&mut self, value: Value) {
        value.trace(self);
    }

    pub fn mark_object<T: GcTrace>(&mut self, obj: GcRef<T>) {
        if let Some(object) = self.objects[obj.index].as_mut() {
            if object.is_marked {
                return;
            }
    
            #[cfg(feature = "debug_log_gc")]
            println!(
                "mark(id:{}, type:{}, val:{:?})",
                obj.index,
                type_name::<T>(),
                obj
            );
            object.is_marked = true;
            self.grey_stack.push_back(obj.index);
        } else {
            panic!("Marking already disposed object {}", obj.index)
        }
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
            if let Some(mut object) = self.objects[i].as_mut() {
                if object.is_marked {
                    object.is_marked = false;
                } else {
                    self.free(i);
                }
            }
        }
    }

    fn remove_white_strings(&mut self) {
        let strings = &mut self.strings;
        let objects = &self.objects;
        strings.retain(|_k, v| objects[v.index].as_ref().unwrap().is_marked);
    }
}
