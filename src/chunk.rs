use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
}

impl Value {
    pub fn is_falsy(&self) -> bool {
        match self {
            Value::Nil | Value::Number(_) | Value::String(_) => true,
            Value::Bool(value) => !value,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(value) => write!(f, "{}", value),
            Value::Number(value) => write!(f, "{}", value),
            Value::String(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    Add,
    Constant(u8),
    Divide,
    Equal,
    False,
    Greater,
    Less,
    Multiply,
    Negate,
    Nil,
    Not,
    Return,
    Substract,
    True,
}
pub struct Chunk {
    pub code: Vec<Instruction>,
    constants: Vec<Value>,
    pub lines: Vec<usize>,
}

impl Chunk {
    pub fn new() -> Chunk {
        // TODO: use from capacity!
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn write(&mut self, instruction: Instruction, line: usize) {
        self.code.push(instruction);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    pub fn read_constant(&self, index: u8) -> Value {
        self.constants[index as usize].clone()
    }

    #[cfg(debug_assertions)]
    pub fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, instruction) in self.code.iter().enumerate() {
            self.disassemble_instruction(instruction, offset);
        }
    }

    #[cfg(debug_assertions)]
    pub fn disassemble_instruction(&self, instruction: &Instruction, offset: usize) {
        print!("{:04} ", offset);
        let line = self.lines[offset];
        if offset > 0 && line == self.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Constant(index) => {
                let i = *index;
                let i = i as usize;
                let value = self.constants[i].clone();
                println!("{:<16} {:4} {}", "OP_CONSTANT", index, value);
            }
            Instruction::Add => println!("OP_ADD"),
            Instruction::Divide => println!("OP_DIVIDE"),
            Instruction::Equal => println!("OP_EQUAL"),
            Instruction::False => println!("OP_FALSE"),
            Instruction::Greater => println!("OP_GREATER"),
            Instruction::Less => println!("OP_LESS"),
            Instruction::Multiply => println!("OP_MULTIPLY"),
            Instruction::Negate => println!("OP_NEGATE"),
            Instruction::Not => println!("OP_NOT"),
            Instruction::Nil => println!("OP_NIL"),
            Instruction::Return => println!("OP_RETURN"),
            Instruction::Substract => println!("OP_SUBSTRACT"),
            Instruction::True => println!("OP_TRUE"),
        }
    }
}
