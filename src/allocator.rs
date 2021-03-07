use std::marker::PhantomData;
use std::any::Any;


#[derive(Clone, Copy)]
struct Reference<T> {
    index: usize,
    _marker: std::marker::PhantomData<T>
}

#[derive(Default)]
struct Allocator {
    objects: Vec<Option<Box<dyn Any>>>
}

impl Allocator {
    fn alloc<T: Any>(&mut self, object: T) -> Reference<T> {
        let reference = Reference {
            index: self.objects.len(),
            _marker: PhantomData
        };
        self.objects.push(Some(Box::new(object)));
        reference
    }

    fn deref<T: Any>(&self, reference: &Reference<T>) -> &T {
        self.objects[reference.index].as_ref().unwrap().downcast_ref().unwrap()
    }

    fn free<T: Any>(&mut self, reference: &Reference<T>) {
        self.objects[reference.index] = None
    }
}

#[derive(Debug)]
struct Function {
    a: usize,
    b: bool
}

impl Drop for Function {
    fn drop(&mut self) {
        println!("Destroying function {:?}", self);
    }
}

struct Closure  {
    a: f64,
    b: f64,
    c: usize
}

pub fn alloc_test() {
    println!("Size of GC: {}", std::mem::size_of::<Reference<Function>>());
    let mut allocator = Allocator::default();

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

    // {
    //     println!("f: {}", allocator.deref(&f).a);
    // }
}