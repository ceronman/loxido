use std::marker::PhantomData;
use std::any::Any;


#[derive(Clone, Copy)]
struct Reference<T> {
    index: usize,
    _marker: std::marker::PhantomData<T>
}

struct Allocator {
    free_slots: Vec<usize>,
    objects: Vec<Option<Box<dyn Any>>>
}

impl Allocator {
    fn new() -> Self {
        Allocator {
            free_slots: vec![],
            objects: vec![]
        }
    }
    fn alloc<T: Any>(&mut self, object: T) -> Reference<T> {
        let entry: Option<Box<dyn Any>> = Some(Box::new(object));
        let index = match self.free_slots.pop() {
            Some(i) => {
                self.objects[i] = entry;
                i
            },
            None => {
                self.objects.push(entry);
                self.objects.len() - 1
            }
        };
        let reference = Reference {
            index,
            _marker: PhantomData
        };
        reference
    }

    fn deref<T: Any>(&self, reference: &Reference<T>) -> &T {
        self.objects[reference.index].as_ref().unwrap().downcast_ref().unwrap()
    }

    fn free<T: Any>(&mut self, reference: &Reference<T>) {
        self.objects[reference.index] = None;
        self.free_slots.push(reference.index)
    }
}

#[derive(Debug)]
struct Function {
    a: usize,
    b: bool
}

impl Drop for Function {
    fn drop(&mut self) {
        // println!("Destroying function {:?}", self);
    }
}

struct Closure  {
    a: f64,
    b: f64,
    c: usize
}

pub fn alloc_test() {
    println!("Size of GC: {}", std::mem::size_of::<Reference<Function>>());
    let mut allocator = Allocator::new();

    let f = {
        let test_function = Function { a: 1, b: true};
        allocator.alloc(test_function)
    };

    let c = {
        let test_closure = Closure { a: 28.0, b: 2.0, c: 100 };
        allocator.alloc(test_closure)
    };


    {
        println!("f: {}", allocator.deref(&f).a);
        println!("f: {}", allocator.deref(&f).a);
        println!("c: {}", allocator.deref(&c).c);
    }

    allocator.free(&f);

    let f2 = {
        let test_function = Function { a: 5, b: false};
        allocator.alloc(test_function)
    };

    {
        println!("c: {}", allocator.deref(&c).c);
        println!("f2: {}", allocator.deref(&f2).a);
    }

    let mut total = 0;
    for x in 0..100_000_000 {
        let f = allocator.alloc(Function { a: x, b: true});
        total += allocator.deref(&f).a;
        allocator.free(&f);
    }
    println!("total {}", total);
}