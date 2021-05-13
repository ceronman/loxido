use std::{collections::HashMap, fmt, hash};
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use fmt::Debug;

use crate::chunk::{Table, Value};

pub trait GcTrace {
    fn format(&self, f: &mut fmt::Formatter, gc: &Gc) -> fmt::Result;
    fn size(&self) -> usize;
    fn trace(&self, gc: &mut Gc);
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

struct GcBox<T: GcTrace + ?Sized + 'static> {
    is_marked: bool,
    next: Option<NonNull<GcBox<dyn GcTrace>>>,
    size: usize,
    value: T,
}

pub struct GcRef<T: GcTrace + ?Sized + 'static> {
    pointer: NonNull<GcBox<T>>,
}

impl<T: GcTrace> Copy for GcRef<T> {}

impl<T: GcTrace> Clone for GcRef<T> {
    fn clone(&self) -> GcRef<T> {
        *self
    }
}

impl<T: GcTrace> Deref for GcRef<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &self.pointer.as_ref().value }
    }
}

impl<T: GcTrace> DerefMut for GcRef<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut self.pointer.as_mut().value }
    }
}

impl<T: GcTrace> Eq for GcRef<T> {}

impl<T: GcTrace> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.pointer == other.pointer
    }
}

impl hash::Hash for GcRef<String> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.pointer.hash(state)
    }
}

impl<T: GcTrace + Debug> Debug for GcRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { self.pointer.as_ref().value.fmt(f) }
    }
}

#[cfg(feature = "debug_log_gc")]
fn short_type_name<T: std::any::Any>() -> &'static str {
    let full_name = std::any::type_name::<T>();
    full_name.split("::").last().unwrap()
}

pub struct Gc {
    bytes_allocated: usize,
    next_gc: usize,
    first: Option<NonNull<GcBox<dyn GcTrace>>>,
    strings: HashMap<&'static str, GcRef<String>>,
    grey_stack: VecDeque<NonNull<GcBox<dyn GcTrace>>>,
}

impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Gc {
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
            first: None,
            strings: HashMap::new(),
            grey_stack: VecDeque::new(),
        }
    }

    pub fn alloc<T: GcTrace + Debug>(&mut self, obj: T) -> GcRef<T> {
        #[cfg(feature = "debug_log_gc")]
        let repr = format!("{:?}", obj)
            .chars()
            .into_iter()
            .take(32)
            .collect::<String>();
        let size = obj.size();
        unsafe {
            let boxed = Box::new(GcBox {
                is_marked: false,
                next: self.first.take(),
                size,
                value: obj,
            });
            self.bytes_allocated += size;
            let pointer = NonNull::new_unchecked(Box::into_raw(boxed));
            self.first = Some(pointer);

            #[cfg(feature = "debug_log_gc")]
            println!(
                "alloc(adr:{:?} type:{} repr:{}, size:{} total:{} next:{})",
                pointer,
                short_type_name::<T>(),
                repr,
                size,
                self.bytes_allocated,
                self.next_gc,
            );

            GcRef { pointer }
        }
    }

    pub fn intern(&mut self, s: String) -> GcRef<String> {
        if let Some(&value) = self.strings.get(&s as &str) {
            value
        } else {
            let reference = self.alloc(s);
            let key = unsafe { &*(reference.deref() as *const String) };
            self.strings.insert(key, reference);
            reference
        }
    }

    pub fn deref<T: GcTrace>(&self, reference: GcRef<T>) -> &T {
        unsafe { &(*reference.pointer.as_ptr()).value }
    }

    pub fn deref_mut<T: GcTrace>(&mut self, reference: GcRef<T>) -> &mut T {
        unsafe { &mut (*reference.pointer.as_ptr()).value }
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
            "collected(bytes:{} before:{} after:{} next:{})",
            before - self.bytes_allocated,
            before,
            self.bytes_allocated,
            self.next_gc
        );
    }

    fn trace_references(&mut self) {
        while let Some(pointer) = self.grey_stack.pop_back() {
            self.blacken_object(pointer);
        }
    }

    fn blacken_object(&mut self, pointer: NonNull<GcBox<dyn GcTrace>>) {
        let object = unsafe { &pointer.as_ref().value };
        #[cfg(feature = "debug_log_gc")]
        println!("blacken(ptr:{:?})", pointer);
        object.trace(self);
    }

    pub fn mark_value(&mut self, value: Value) {
        value.trace(self);
    }

    pub fn mark_object<T: GcTrace + Debug>(&mut self, mut reference: GcRef<T>) {
        unsafe {
            reference.pointer.as_mut().is_marked = true;
            self.grey_stack.push_front(reference.pointer);
            #[cfg(feature = "debug_log_gc")]
            println!(
                "mark(adr:{:?}, type:{}, val:{:?})",
                reference.pointer,
                short_type_name::<T>(),
                &reference.pointer.as_ref().value
            );
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
        while let Some(mut object) = self.first {
            unsafe {
                let object = object.as_mut();
                self.first = object.next;
                if object.is_marked {
                    object.is_marked = false;
                } else {
                    let boxed = Box::from_raw(object);
                    self.bytes_allocated -= boxed.size;
                }
            }
        }
    }

    fn remove_white_strings(&mut self) {
        self.strings
            .retain(|_k, v| unsafe { v.pointer.as_ref().is_marked });
    }
}
