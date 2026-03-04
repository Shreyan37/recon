fn add(a: i32, b: i32) -> i32 {
    a + b + 0 // added a no-op
}

fn main() {
    let x = 5;
    let y = 20; // changed value
    println!("Sum: {}", add(x, y));
    println!("Done"); // added line
}
