trait Greeter {
    fn greet(&self) -> String;
    fn farewell(&self) -> String; // added method
}

struct Person {
    name: String,
    age: u8, // added field
}

impl Greeter for Person {
    fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }

    fn farewell(&self) -> String {
        format!("Goodbye, {}!", self.name)
    }
}
