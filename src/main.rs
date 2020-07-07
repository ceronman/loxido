use std::io;
use std::cmp::Ordering;

fn main() {
    let number = 46;

    loop {
        println!("Guess a number: ");

        let mut buf = String::new();

        io::stdin()
            .read_line(&mut buf)
            .expect("Error reading guess");

        let guess: i32 = buf.trim().parse().expect("Please type a number");

        match guess.cmp(&number) {
            Ordering::Greater => println!("Greater"),
            Ordering::Less => println!("Less"),
            Ordering::Equal => {
                println!("Got it");
                break;
            }
        }
    }
}
