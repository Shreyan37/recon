const BUFFER_SIZE: usize = 1024;
const MAX_RETRIES: u32 = 3;

mod core {
    pub fn compute(x: i32) -> i32 {
        x * 2
    }

    pub fn process(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(data.len());
        for &b in data {
            result.push(b + 1);
        }
        result
    }
}

mod utils {
    pub fn format_name(first: &str, last: &str) -> String {
        format!("{}, {}", last, first)
    }

    pub fn trim_whitespace(s: &str) -> String {
        s.split_whitespace().collect()
    }
}

mod models {
    #[derive(Debug)]
    pub enum Status {
        Active,
        Inactive,
        Pending,
    }

    impl Status {
        pub fn is_active(&self) -> bool {
            match self {
                Status::Active => true,
                _ => false,
            }
        }
    }

    pub struct User {
        pub id: u64,
        pub name: String,
        pub status: Status,
    }

    impl User {
        pub fn new(id: u64, name: &str, status: Status) -> Self {
            User {
                id,
                name: name.to_string(),
                status,
            }
        }

        pub fn display(&self) -> String {
            format!("User {}: {} ({:?})", self.id, self.name, self.status)
        }
    }

    pub trait Notifier {
        fn notify(&self, message: &str);
    }

    impl Notifier for User {
        fn notify(&self, message: &str) {
            println!("Notifying {}: {}", self.name, message);
        }
    }
}

fn main() {
    let x = 10;
    let y = core::compute(x);
    println!("compute({}) = {}", x, y);

    let data = vec![1, 2, 3];
    let processed = core::process(&data);
    println!("processed: {:?}", processed);

    let full_name = utils::format_name("John", "Doe");
    println!("Formatted name: {}", full_name);

    let trimmed = utils::trim_whitespace("  hello   world  ");
    println!("Trimmed: '{}'", trimmed);

    use models::{Status, User};
    let user = User::new(42, "Alice", Status::Active);
    println!("{}", user.display());
    user.notify("Welcome!");

    let status = Status::Pending;
    println!("Status active? {}", status.is_active());

    let _unused = 100;
}
