use std::io;
use std::fs;
use std::collections::HashMap;

fn main() {
    let user_name = "Alice";
    let count = 5;
    let data = vec![1, 2, 3];
    
    println!("Hello, {}", user_name);
    println!("Count: {}", count);
    
    for item in data {
        println!("{}", item);
    }
    
    let result = calculate(10, 20);
    println!("Result: {}", result);
}

fn calculate(a: i32, b: i32) -> i32 {
    a + b
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
