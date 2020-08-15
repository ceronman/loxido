use std::collections::HashMap;

pub type LoxString = usize;

#[derive(Default)]
pub struct Strings {
    map: HashMap<String, LoxString>,
    vec: Vec<String>,
}

impl Strings {
    pub fn intern_onwed(&mut self, name: String) -> LoxString {
        if let Some(&value) = self.map.get(&name) {
            value
        } else {
            let value = self.vec.len();
            self.vec.push(name.clone());
            self.map.insert(name.clone(), value);
            value
        }
    }

    pub fn intern(&mut self, name: &str) -> LoxString {
        if let Some(&value) = self.map.get(name) {
            value
        } else {
            let value = self.vec.len();
            self.vec.push(name.to_owned());
            self.map.insert(name.to_owned(), value);
            value
        }
    }

    pub fn lookup(&self, s: LoxString) -> &str {
        self.vec[s].as_str()
    }
}
