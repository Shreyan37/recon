const BUFFER_SIZE: usize = 2048;               // SEMANTIC: value changed
const MAX_RETRIES: u32 = 3;                    // unchanged

mod core {
    pub fn compute(x: i32) -> i32 {
        x * 2 + 1                               // SEMANTIC: added +1
    }

    pub fn process(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(data.len());
        let mut i = 0;                           // COSMETIC: added local variable
        while i < data.len() {                    // SEMANTIC: for → while
            result.push(data[i] + 1);
            i += 1;
        }
        result
    }
}

mod utils {
    pub fn format_name(first: &str, last: &str) -> String {
        format!("{}, {}", last , first )         // COSMETIC: extra spaces
    }

    pub fn trim_whitespace(s: &str) -> String {
        s.split_whitespace().collect()
    }
}

mod models {
    #[derive(Debug)]
    pub enum Status {
        Active,
        Pending,                                  // COSMETIC: reordered
        Inactive,
    }

    impl Status {
        pub fn is_active(&self) -> bool {
            match self {
                Status::Active => true,
                Status::Pending => false,         // SEMANTIC: now Pending returns false (same as before, but order changed, still same logic? Actually pending already returned false, but we added explicit arm; it's still semantic because we added a match arm)
                Status::Inactive => false,
            }
        }
    }

    pub struct User {
        pub name: String,                         // COSMETIC: reordered fields
        pub id: u64,
        pub status: Status,
        pub retries: u32,                          // SEMANTIC: added field
    }

    impl User {
        pub fn new(id: u64, name: &str, status: Status) -> Self {
            User {
                name: name.to_string(),
                id,
                status,
                retries: 0,                         // SEMANTIC: initialize new field
            }
        }

        pub fn display(&self) -> String {
            format!("User {}: {} ({:?})", self.id, self.name, self.status)
        }

        pub fn increment_retries(&mut self) {        // SEMANTIC: new method
            self.retries += 1;
        }
    }

    pub trait Notifier {
        fn notify(&self, message: &str);
        fn notify_urgent(&self, message: &str) {     // SEMANTIC: new trait method with default impl
            self.notify(&format!("URGENT: {}", message));
        }
    }

    impl Notifier for User {
        fn notify(&self, message: &str) {
            println!("Notifying {}: {}", self.name, message);
        }

        // uses default notify_urgent
    }
}

fn main() {
    let val = 10;                                     // COSMETIC: renamed from x
    let y = core::compute(val);
    println!("compute({}) = {}", val, y);

    let data = vec![1, 2, 3];
    let processed = core::process(&data);
    println!("processed: {:?}", processed);

    let full_name = utils::format_name("John", "Doe");
    println!("Formatted name: {}", full_name);

    let trimmed = utils::trim_whitespace("  hello   world  ");
    println!("Trimmed: '{}'", trimmed);

    use models::{Status, User};
    let mut user = User::new(42, "Alice", Status::Active);   // now mutable
    println!("{}", user.display());
    user.notify("Welcome!");
    user.notify_urgent("Urgent message");                     // using new trait method
    user.increment_retries();                                  // using new method

    let status = Status::Pending;
    println!("Status active? {}", status.is_active());

    let _unused = 100;                                        // unchanged
}
