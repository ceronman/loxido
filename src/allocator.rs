use std::marker::PhantomData;
use std::{any::Any, collections::HashMap, fmt, hash};

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

impl<T> fmt::Debug for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reference")
            .field("index", &self.index)
            .finish()
    }
}

impl<T> fmt::Display for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reference")
            .field("index", &self.index)
            .finish()
    }
}

impl<T> PartialEq for Reference<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> hash::Hash for Reference<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state)
    }
}

#[derive(Default)]
pub struct Allocator {
    free_slots: Vec<usize>,
    objects: Vec<Option<Box<dyn Any>>>,
    strings: HashMap<String, Reference<String>>,
}

impl Allocator {
    fn alloc<T: Any>(&mut self, object: T) -> Reference<T> {
        let entry: Option<Box<dyn Any>> = Some(Box::new(object));
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

    pub fn deref<T: Any>(&self, reference: &Reference<T>) -> &T {
        self.objects[reference.index]
            .as_ref()
            .unwrap()
            .downcast_ref()
            .unwrap()
    }

    #[allow(dead_code)]
    fn free<T: Any>(&mut self, reference: &Reference<T>) {
        self.objects[reference.index] = None;
        self.free_slots.push(reference.index)
    }
}
