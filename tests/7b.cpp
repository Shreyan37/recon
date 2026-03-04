macro_rules! say_hello {
    () => {
        println!("Hello, world!"); // changed message
    };
    ($name:expr) => { // added arm
        println!("Hello, {}!", $name);
    };
}

fn main() {
    say_hello!();
    say_hello!("Alice");
}
