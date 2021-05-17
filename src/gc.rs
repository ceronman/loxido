use std::{
    fmt::{self, Display},
    hash,
};
use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use ahash::AHashMap;
use fmt::Debug;

use crate::chunk::{Table, Value};

pub trait GcTrace {
    fn size(&self) -> usize;
    fn trace(&self, gc: &mut Gc);
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

impl<T: GcTrace> GcRef<T> {
    pub fn dangling() -> GcRef<T> {
        GcRef {
            pointer: NonNull::dangling(),
        }
    }
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

impl<T: GcTrace + Display> Display for GcRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe { write!(f, "{}", self.pointer.as_ref().value) }
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
    strings: AHashMap<&'static str, GcRef<String>>,
    grey_stack: Vec<NonNull<GcBox<dyn GcTrace>>>,
}

impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Gc {
            bytes_allocated: 0,
            next_gc: 1024 * 1024,
            first: None,
            strings: AHashMap::new(),
            grey_stack: Vec::new(),
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
        while let Some(pointer) = self.grey_stack.pop() {
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
            self.grey_stack.push(reference.pointer);
            #[cfg(feature = "debug_log_gc")]
            println!(
                "mark(adr:{:?}, type:{}, val:{:?})",
                reference.pointer,
                short_type_name::<T>()
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
        let mut previous: Option<NonNull<GcBox<dyn GcTrace>>> = None;
        let mut current: Option<NonNull<GcBox<dyn GcTrace>>> = self.first;
        while let Some(mut object) = current {
            unsafe {
                let object_ptr = object.as_mut();
                current = object_ptr.next;
                if object_ptr.is_marked {
                    object_ptr.is_marked = false;
                    previous = Some(object);
                } else {
                    if let Some(mut previous) = previous {
                        previous.as_mut().next = object_ptr.next
                    } else {
                        self.first = object_ptr.next
                    }
                    let boxed = Box::from_raw(object_ptr);
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
