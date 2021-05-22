use std::alloc;
use std::ptr::NonNull;
use std::{
    hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicUsize,
    usize,
};

use ahash::AHashMap;

use crate::{
    chunk::{Table, Value},
    objects::ObjectType,
};

struct GcHeader {
    marked: bool,
    next: Option<NonNull<GcHeader>>,
    object: ObjectType,
}
pub trait GcObject {
    fn into_object(self) -> ObjectType;
    fn unwrap_ref(obj: &ObjectType) -> &Self;
    fn unwrap_mut(obj: &mut ObjectType) -> &mut Self;
}

pub struct GcRef<T> {
    header: NonNull<GcHeader>,
    _marker: PhantomData<T>,
}

impl<T> GcRef<T> {
    pub fn dangling() -> GcRef<T> {
        GcRef {
            header: NonNull::dangling(),
            _marker: PhantomData,
        }
    }
}

impl<T: GcObject> Deref for GcRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { T::unwrap_ref(&self.header.as_ref().object) }
    }
}

impl<T: GcObject> DerefMut for GcRef<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { T::unwrap_mut(&mut self.header.as_mut().object) }
    }
}

impl<T> Copy for GcRef<T> {}

impl<T> Clone for GcRef<T> {
    fn clone(&self) -> GcRef<T> {
        *self
    }
}

impl<T> Eq for GcRef<T> {}

impl<T> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
    }
}

impl hash::Hash for GcRef<String> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.header.hash(state)
    }
}

#[cfg(feature = "debug_log_gc")]
fn short_type_name<T: std::any::Any>() -> &'static str {
    let full_name = std::any::type_name::<T>();
    full_name.split("::").last().unwrap()
}

pub struct Gc {
    next_gc: usize,
    first: Option<NonNull<GcHeader>>,
    strings: AHashMap<&'static str, GcRef<String>>,
    grey_stack: Vec<NonNull<GcHeader>>,
}

impl Gc {
    const HEAP_GROW_FACTOR: usize = 2;

    pub fn new() -> Self {
        Gc {
            next_gc: 1024 * 1024,
            first: None,
            strings: AHashMap::new(),
            grey_stack: Vec::new(),
        }
    }

    pub fn alloc<T: GcObject + 'static>(&mut self, object: T) -> GcRef<T> {
        unsafe {
            let header = Box::new(GcHeader {
                marked: false,
                next: self.first.take(),
                object: object.into_object(),
            });
            #[cfg(feature = "debug_log_gc")]
            let repr = format!("{}", header.object)
                .chars()
                .into_iter()
                .take(32)
                .collect::<String>();
            let header = NonNull::new_unchecked(Box::into_raw(header));
            self.first = Some(header);

            #[cfg(feature = "debug_log_gc")]
            println!(
                "alloc(adr:{:?} type:{} repr:{}, allocated bytes:{} next:{})",
                header,
                short_type_name::<T>(),
                repr,
                GLOBAL.bytes_allocated(),
                self.next_gc,
            );

            GcRef {
                header,
                _marker: PhantomData,
            }
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
        let before: isize = GLOBAL.bytes_allocated() as isize;

        self.trace_references();
        self.remove_white_strings();
        self.sweep();
        self.next_gc = GLOBAL.bytes_allocated() * Gc::HEAP_GROW_FACTOR;

        #[cfg(feature = "debug_log_gc")]
        println!(
            "collected(bytes:{} before:{} after:{} next:{})",
            before - GLOBAL.bytes_allocated() as isize,
            before,
            GLOBAL.bytes_allocated(),
            self.next_gc
        );
    }

    fn trace_references(&mut self) {
        while let Some(pointer) = self.grey_stack.pop() {
            self.blacken_object(pointer);
        }
    }

    fn blacken_object(&mut self, pointer: NonNull<GcHeader>) {
        let object = unsafe { &pointer.as_ref().object };
        #[cfg(feature = "debug_log_gc")]
        println!("blacken(adr:{:?})", pointer);

        match object {
            ObjectType::Function(function) => {
                self.mark_object(function.name);
                for &constant in &function.chunk.constants {
                    self.mark_value(constant);
                }
            }
            ObjectType::Closure(closure) => {
                self.mark_object(closure.function);
                for &upvalue in &closure.upvalues {
                    self.mark_object(upvalue);
                }
            }
            ObjectType::String(_) => {}
            ObjectType::Upvalue(upvalue) => {
                if let Some(obj) = upvalue.closed {
                    self.mark_value(obj)
                }
            }
            ObjectType::Class(class) => {
                self.mark_object(class.name);
                self.mark_table(&class.methods);
            }
            ObjectType::Instance(instance) => {
                self.mark_object(instance.class);
                self.mark_table(&instance.fields);
            }
            ObjectType::BoundMethod(method) => {
                self.mark_value(method.receiver);
                self.mark_object(method.method);
            }
        }
    }

    pub fn mark_value(&mut self, value: Value) {
        match value {
            Value::BoundMethod(value) => self.mark_object(value),
            Value::Class(value) => self.mark_object(value),
            Value::Closure(value) => self.mark_object(value),
            Value::Function(value) => self.mark_object(value),
            Value::Instance(value) => self.mark_object(value),
            Value::String(value) => self.mark_object(value),
            _ => (),
        }
    }

    pub fn mark_object<T: GcObject + 'static>(&mut self, mut reference: GcRef<T>) {
        unsafe {
            reference.header.as_mut().marked = true;
            self.grey_stack.push(reference.header);
            #[cfg(feature = "debug_log_gc")]
            println!(
                "mark(adr:{:?}, type:{}, val:{})",
                reference.header,
                short_type_name::<T>(),
                reference.header.as_ref().object
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
        GLOBAL.bytes_allocated() > self.next_gc
    }

    fn sweep(&mut self) {
        let mut previous: Option<NonNull<GcHeader>> = None;
        let mut current: Option<NonNull<GcHeader>> = self.first;
        while let Some(mut object) = current {
            unsafe {
                let object_ptr = object.as_mut();
                current = object_ptr.next;
                if object_ptr.marked {
                    object_ptr.marked = false;
                    previous = Some(object);
                } else {
                    if let Some(mut previous) = previous {
                        previous.as_mut().next = object_ptr.next
                    } else {
                        self.first = object_ptr.next
                    }
                    #[cfg(feature = "debug_log_gc")]
                    println!("free(adr:{:?})", object_ptr as *mut GcHeader);
                    Box::from_raw(object_ptr);
                }
            }
        }
    }

    fn remove_white_strings(&mut self) {
        self.strings
            .retain(|_k, v| unsafe { v.header.as_ref().marked });
    }
}

struct GlobalAllocator {
    bytes_allocated: AtomicUsize,
}

impl GlobalAllocator {
    fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

unsafe impl alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        self.bytes_allocated
            .fetch_add(layout.size(), std::sync::atomic::Ordering::Relaxed);
        alloc::System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: alloc::Layout) {
        alloc::System.dealloc(ptr, layout);
        self.bytes_allocated
            .fetch_sub(layout.size(), std::sync::atomic::Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: GlobalAllocator = GlobalAllocator {
    bytes_allocated: AtomicUsize::new(0),
};
