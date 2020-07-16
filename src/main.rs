use std::env;

type Value = f64;

enum Instruction {
    Constant(u8),
    Return,
}

struct Chunk {
    code: Vec<Instruction>,
    constants: Vec<Value>,
    lines: Vec<usize>
}

impl Chunk {
    fn new() -> Chunk {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
        }
    }

    fn write(&mut self, instruction: Instruction, line: usize) {
        self.code.push(instruction);
        self.lines.push(line);
    }

    fn add_constant(&mut self, value: Value) -> u8 {
        self.constants.push(value);
        (self.constants.len() - 1) as u8
    }

    fn disassemble(&self, name: &str) {
        println!("== {} ==", name);
        for (offset, instruction) in self.code.iter().enumerate() {
            self.disassemble_instruction(instruction, offset);
        }
    }
    
    fn disassemble_instruction(&self, instruction: &Instruction, offset: usize) {
        print!("{:04} ", offset);
        let line = self.lines[offset];
        if offset > 0 && line == self.lines[offset -1] {
            print!("   | ");
        } else {
            print!("{:>4} ", line);
        }
        match instruction {
            Instruction::Return => println!("OP_RETURN"),
            Instruction::Constant(index) => {
                let i = *index;
                let i =i as usize;
                let value = self.constants[i];
                println!("{:<16} {:4} {}", "OP_CONSTANT", index, value);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args()
        .map(|s| format!("\"{}\"", s))
        .collect();
    println!("{}", args.join(", "));

    let mut chunk = Chunk::new();
    chunk.write(Instruction::Return, 123);
    let index = chunk.add_constant(1.2);
    chunk.write(Instruction::Constant(index), 124);
    chunk.disassemble("test chunk");
}
