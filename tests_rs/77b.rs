const VERSION: &str = "1.0.0";
const MAX_ITEMS: usize = 200;

fn calculate(a: i32, b: i32) -> i32 {
    let mut result = a + b + 1;
    let mut i = 0;
    while i < 10 {
        result += i;
        i += 1;
    }
    result
}

struct Data<T> {
    counter: usize,
    items: Vec<T>,
}

impl<T> Data<T> {
    fn new() -> Self {
        Data {
            items: Vec::new(),
            counter: 0,
        }
    }

    fn add(&mut self, item: T) {
        self.items.push(item);
        self.counter += 1;
    }

    fn len(&self) -> usize {
        self.items.len()
    }
}

enum Status {
    Pending,
    Success,
    Failure(String),
}

trait Handler {
    fn handle(&self, event: &str);
    fn log(&self, event: &str) {
        println!("[LOG] {}", event);
    }
}

impl Handler for Status {
    fn handle(&self, event: &str) {
        match self {
            Status::Success => println!("Success: {}", event),
            Status::Failure(msg) => println!("Failure: {} – {}", msg, event),
            Status::Pending => println!("Pending: {}", event),
        }
    }
}

fn main() {
    let val = 5;
    let y = 10;
    let sum = calculate(val, y);
    println!("Sum: {}", sum);

    let mut data = Data::<i32>::new();
    data.add(1);
    data.add(2);
    data.add(3);
    println!("Data length: {}", data.len());

    let status = Status::Pending;
    status.handle("startup");
    status.log("startup");

    let numbers = vec![1, 2, 3, 4, 5];
    let doubled: Vec<i32> = numbers.iter().map(|&n| n * 2).collect();
    println!("Doubled: {:?}", doubled);
}
