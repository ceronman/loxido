use std::alloc::{alloc, dealloc, Layout};
use std::ptr::{null, null_mut};

use crate::chunk::Value;

struct LoxString {
    s: String,
    hash: usize,
}

impl LoxString {
    fn new(s: &str) -> Self {
        LoxString {
            s: s.to_owned(),
            hash: LoxString::hash_string(s),
        }
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

struct Entry {
    key: *const LoxString,
    value: Value,
}

struct Table {
    count: usize,
    capacity: usize,
    entries: *mut Entry,
}

impl Table {
    const MAX_LOAD: f32 = 0.75;

    fn new() -> Self {
        Table {
            count: 0,
            capacity: 0,
            entries: null_mut(),
            //            _marker: PhantomData
        }
    }

    fn set(&mut self, key: &LoxString, value: Value) -> bool {
        unsafe {
            if self.count + 1 > (self.capacity as f32 * Table::MAX_LOAD) as usize {
                let capacity = if self.capacity < 8 {
                    8
                } else {
                    self.capacity * 2
                };
                self.adjust_capacity(capacity);
            }
            let mut entry = Table::find_entry(self.entries, self.capacity, key);
            let is_new_key = (*entry).key.is_null();
            if is_new_key {
                if let Value::Nil = (*entry).value {
                    self.count += 1;
                }
            }
            (*entry).key = key;
            (*entry).value = value;
            is_new_key
        }
    }

    fn get(&self, key: &LoxString) -> Option<Value> {
        unsafe {
            if self.count == 0 {
                return None;
            }
            let entry = Table::find_entry(self.entries, self.capacity, key);
            if (*entry).key.is_null() {
                return None;
            }
            return Some((*entry).value);
        }
    }

    fn delete(&mut self, key: &LoxString) -> bool {
        unsafe {
            if self.count == 0 {
                return false;
            }
            let entry = Table::find_entry(self.entries, self.capacity, key);
            if (*entry).key.is_null() {
                return false;
            }
            (*entry).key = null_mut();
            (*entry).value = Value::Bool(true);
            true
        }
    }

    fn add_all(&mut self, other: &Table) {
        unsafe {
            for i in 0..(other.capacity as isize) {
                let entry = other.entries.offset(i);
                if !(*entry).key.is_null() {
                    self.set(&*(*entry).key, (*entry).value);
                }
            }
        }
    }

    unsafe fn find_string(&self, s: &str, hash: usize) -> *const LoxString {
        if self.count == 0 {
            return null();
        }
        let mut index = hash & (self.capacity - 1);
        loop {
            let entry = self.entries.offset(index as isize);
            if (*entry).key.is_null() {
                if let Value::Nil = (*entry).value {
                    return null();
                }
            } else if s == (&*(*entry).key).s {
                return (*entry).key;
            }
            index = (index + 1) & (self.capacity - 1);
        }
    }

    unsafe fn find_entry(
        entries: *mut Entry,
        capacity: usize,
        key: *const LoxString,
    ) -> *mut Entry {
        let mut index = (*key).hash & (capacity - 1);
        let mut tombstone: *mut Entry = null_mut();
        loop {
            let entry = entries.offset(index as isize);
            if (*entry).key.is_null() {
                if let Value::Nil = (*entry).value {
                    return if !tombstone.is_null() {
                        tombstone
                    } else {
                        entry
                    };
                } else if tombstone.is_null() {
                    tombstone = entry;
                }
            } else if (*entry).key == key {
                return entry;
            }
            index = (index + 1) & (capacity - 1);
        }
    }

    unsafe fn adjust_capacity(&mut self, capacity: usize) {
        let entries = alloc(Layout::array::<Entry>(capacity).unwrap()) as *mut Entry;
        for i in 0..(capacity as isize) {
            let entry = entries.offset(i);
            (*entry).key = null_mut();
            (*entry).value = Value::Nil
        }
        self.count = 0;
        for i in 0..(self.capacity as isize) {
            let entry = self.entries.offset(i);
            if (*entry).key.is_null() {
                continue;
            }
            let dest = Table::find_entry(entries, capacity, (*entry).key);
            (*dest).key = (*entry).key;
            (*dest).value = (*entry).value;
            self.count += 1;
        }
        dealloc(
            self.entries.cast(),
            Layout::array::<Entry>(self.capacity).unwrap(),
        );
        self.entries = entries;
        self.capacity = capacity;
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        unsafe {
            if !self.entries.is_null() {
                dealloc(
                    self.entries.cast(),
                    Layout::array::<Entry>(self.capacity).unwrap(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LoxString, Table};
    use crate::chunk::Value;
    #[test]
    fn basic() {
        let mut table = Table::new();
        let foo = LoxString::new("foo");

        table.set(&foo, Value::Number(10f64));

        if let Some(Value::Number(x)) = table.get(&foo) {
            assert_eq!(x, 10f64);
        } else {
            panic!("No value")
        }

        let bar = LoxString::new("bar");
        assert!(matches!(table.get(&bar), None));

        table.set(&bar, Value::Bool(false));
        assert!(matches!(table.get(&bar), Some(Value::Bool(false))));
    }

    #[test]
    fn delete() {
        let mut table = Table::new();
        let foo = LoxString::new("foo");
        table.set(&foo, Value::Bool(true));
        assert!(matches!(table.get(&foo), Some(Value::Bool(true))));
        table.delete(&foo);
        assert!(matches!(table.get(&foo), None));
    }

    #[test]
    fn set_twice() {
        let mut table = Table::new();
        let foo = LoxString::new("foo");
        table.set(&foo, Value::Bool(true));
        assert!(matches!(table.get(&foo), Some(Value::Bool(true))));
        table.set(&foo, Value::Nil);
        assert!(matches!(table.get(&foo), Some(Value::Nil)));
    }

    #[test]
    fn grow() {
        let mut table = Table::new();
        let mut keys: Vec<LoxString> = (0..64)
            .map(|i| LoxString::new(&format!("key {}", i)))
            .collect();

        for (i, key) in keys.iter().enumerate() {
            table.set(key, Value::Number(i as f64));
        }

        for (i, key) in keys.iter().enumerate() {
            if let Some(Value::Number(x)) = table.get(&key) {
                assert_eq!(x, i as f64);
            } else {
                panic!("No value")
            }
        }
    }

    #[test]
    fn add_all() {
        let mut table = Table::new();
        let mut keys: Vec<LoxString> = (0..64)
            .map(|i| LoxString::new(&format!("key {}", i)))
            .collect();

        for (i, key) in keys.iter().enumerate() {
            table.set(key, Value::Number(i as f64));
        }

        let mut table2 = Table::new();
        table2.add_all(&table);

        for (i, key) in keys.iter().enumerate() {
            if let Some(Value::Number(x)) = table2.get(&key) {
                assert_eq!(x, i as f64);
            } else {
                panic!("No value")
            }
        }
    }

    #[test]
    fn drop() {
        {
            for i in 0..100 {
                let mut table = Table::new();
                let key = LoxString::new(&format!("key {}", i));
                table.set(&key, Value::Bool(true));
            }
        }
    }

    #[test]
    fn find_string() {
        let mut table = Table::new();
        let foo = LoxString::new("foo");
        assert!(unsafe { table.find_string(&foo.s, foo.hash).is_null() });
        table.set(&foo, Value::Nil);
        assert_eq!(
            unsafe { table.find_string(&foo.s, foo.hash) },
            &foo as *const LoxString
        );
    }
}
