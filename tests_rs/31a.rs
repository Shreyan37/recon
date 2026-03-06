// =============================================================================
// medium_a.rs – Original version
// =============================================================================

// Global constants
const SCALE_FACTOR: i32 = 2;
const APP_NAME: &str = "MyApp";

// -----------------------------------------------------------------------------
// Module: math_ops
// -----------------------------------------------------------------------------
mod math_ops {
    use super::SCALE_FACTOR;

    // Public function: adds two numbers and scales the result
    pub fn add_scaled(a: i32, b: i32) -> i32 {
        (a + b) * SCALE_FACTOR
    }

    // Public function: computes factorial (iterative)
    pub fn factorial(n: u64) -> u64 {
        let mut result = 1;
        for i in 1..=n {
            result *= i;
        }
        result
    }

    // Private helper
    fn internal_usage(x: i32) -> i32 {
        x * x
    }
}

// -----------------------------------------------------------------------------
// Module: string_utils
// -----------------------------------------------------------------------------
mod string_utils {
    // A simple struct to hold a string and its length
    pub struct Text {
        content: String,
        length: usize,
    }

    impl Text {
        pub fn new(s: &str) -> Self {
            let len = s.len();
            Text {
                content: s.to_string(),
                length: len,
            }
        }

        pub fn content(&self) -> &str {
            &self.content
        }

        pub fn length(&self) -> usize {
            self.length
        }

        // Returns a new Text with the string repeated `count` times
        pub fn repeat(&self, count: usize) -> Self {
            let repeated = self.content.repeat(count);
            Text::new(&repeated)
        }
    }

    // Public function: concatenates two strings with a separator
    pub fn join_with_sep(a: &str, b: &str, sep: &str) -> String {
        format!("{}{}{}", a, sep, b)
    }
}

// -----------------------------------------------------------------------------
// Module: data_models
// -----------------------------------------------------------------------------
mod data_models {
    // An enum representing different user roles
    #[derive(Debug)]
    pub enum Role {
        Admin,
        Editor,
        Viewer,
    }

    impl Role {
        pub fn can_write(&self) -> bool {
            match self {
                Role::Admin | Role::Editor => true,
                Role::Viewer => false,
            }
        }
    }

    // A struct for user information
    pub struct User {
        pub id: u32,
        pub name: String,
        pub role: Role,
    }

    impl User {
        pub fn new(id: u32, name: &str, role: Role) -> Self {
            User {
                id,
                name: name.to_string(),
                role,
            }
        }

        pub fn display(&self) -> String {
            format!("#{}: {} ({:?})", self.id, self.name, self.role)
        }
    }
}

// -----------------------------------------------------------------------------
// Main function – exercises the above modules
// -----------------------------------------------------------------------------
fn main() {
    println!("Starting {} (original)", APP_NAME);

    // Math ops
    let sum = math_ops::add_scaled(3, 4);
    println!("add_scaled(3,4) = {}", sum);

    let fact = math_ops::factorial(5);
    println!("factorial(5) = {}", fact);

    // String utils
    let t = string_utils::Text::new("hello");
    println!("Text: '{}', length: {}", t.content(), t.length());

    let joined = string_utils::join_with_sep("left", "right", " <=> ");
    println!("Joined: '{}'", joined);

    let repeated = t.repeat(3);
    println!("Repeated 3 times: '{}'", repeated.content());

    // Data models
    use data_models::{Role, User};

    let admin = User::new(1, "Alice", Role::Admin);
    let viewer = User::new(2, "Bob", Role::Viewer);

    println!("Admin: {}", admin.display());
    println!("Viewer can write? {}", viewer.role.can_write());

    // Extra: a local variable that will be renamed in modified version
    let temp_value = 42;
    println!("Temp value: {}", temp_value);
}
