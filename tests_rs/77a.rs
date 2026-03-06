const VERSION: &str = "1.0.0";
const MAX_ITEMS: usize = 100;

fn calculate(a: i32, b: i32) -> i32 {
    let mut result = a + b;
    for i in 0..10 {
        result += i;
    }
    result
}

struct Data<T> {
    items: Vec<T>,
    counter: usize,
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
        self.counter
    }
}

enum Status {
    Success,
    Failure(String),
    Pending,
}

trait Handler {
    fn handle(&self, event: &str);
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
    let x = 5;
    let y = 10;
    let sum = calculate(x, y);
    println!("Sum: {}", sum);

    let mut data = Data::<i32>::new();
    data.add(1);
    data.add(2);
    data.add(3);
    println!("Data length: {}", data.len());

    let status = Status::Pending;
    status.handle("startup");

    let numbers = vec![1, 2, 3, 4, 5];
    let doubled: Vec<i32> = numbers.iter().map(|&n| n * 2).collect();
    println!("Doubled: {:?}", doubled);
}
