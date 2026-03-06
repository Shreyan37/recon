use std::fs;
use std::io;
use std::collections::HashSet;

fn main() {
    let username = "Alice";
    let total = 10;
    let data = vec![1, 2, 3,];
    
    println!("Hello, {}", username);
    println!("Total: {}", total);
    
    for item in data {
        println!("{}", item);
    }
    
    let result = calculate(10, 30);
    println!("Result: {}", result);
}

fn calculate(a: i32, b: i32) -> i32 {
    a * b
}

struct Config {
    name: String,
    value: i32,
}

fn process(items: Vec<i32>) {
    for i in items {
        println!("{}", i);
    }
}
