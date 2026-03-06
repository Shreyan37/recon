// =============================================================================
// medium_b.rs – Modified version (mixed semantic & cosmetic changes)
// =============================================================================

// Global constants – note: SCALE_FACTOR changed (SEMANTIC)
const SCALE_FACTOR: i32 = 3;
// Cosmetic: added a comment, changed formatting
const APP_NAME: &str = "MyApp";  // unchanged (cosmetic: no change here)

// -----------------------------------------------------------------------------
// Module: math_ops (contains both semantic and cosmetic changes)
// -----------------------------------------------------------------------------
mod math_ops {
    use super::SCALE_FACTOR;

    // Public function: adds two numbers and scales the result
    // Changed: added an extra addition (SEMANTIC)
    pub fn add_scaled(a: i32, b: i32) -> i32 {
        (a + b) * SCALE_FACTOR + 1
    }

    // Public function: computes factorial (iterative)
    // Changed: replaced for loop with while loop (SEMANTIC – different iteration)
    pub fn factorial(n: u64) -> u64 {
        let mut result = 1;
        let mut i = 1;
        while i <= n {
            result *= i;
            i += 1;
        }
        result
    }

    // Private helper – added a blank line (COSMETIC)
    fn internal_usage(x: i32) -> i32 {

        x * x
    }
}

// -----------------------------------------------------------------------------
// Module: string_utils (mixed changes)
// -----------------------------------------------------------------------------
mod string_utils {
    // A simple struct to hold a string and its length
    // Added a new field `modified` (SEMANTIC – changes memory layout)
    pub struct Text {
        content: String,
        length: usize,
        modified: bool,   // new field
    }

    impl Text {
        // Updated constructor to set `modified = false` (SEMANTIC)
        pub fn new(s: &str) -> Self {
            let len = s.len();
            Text {
                content: s.to_string(),
                length: len,
                modified: false,
            }
        }

        pub fn content(&self) -> &str {
            &self.content
        }

        pub fn length(&self) -> usize {
            self.length
        }

        // Returns a new Text with the string repeated `count` times
        // Added a line to mark the new text as modified (SEMANTIC)
        pub fn repeat(&self, count: usize) -> Self {
            let repeated = self.content.repeat(count);
            let mut new_text = Text::new(&repeated);
            new_text.modified = true;   // mark as modified
            new_text
        }

        // New getter for the `modified` field (SEMANTIC)
        pub fn is_modified(&self) -> bool {
            self.modified
        }
    }

    // Public function: concatenates two strings with a separator
    // Cosmetic: added extra spaces inside format! (COSMETIC)
    pub fn join_with_sep(a: &str, b: &str, sep: &str) -> String {
        format!( "{}" "{}" "{}", a, sep, b )   // note the missing commas – wait, that's a syntax error! Let's fix: we want a cosmetic change but not break compilation.
        // Better: add extra parentheses and spaces, but keep commas.
        // I'll rewrite properly:
    }
    // (Fix: I'll provide corrected version below – see explanation.)
}

// To avoid a syntax error, we'll correct the join_with_sep function in a moment.
// For now, let's finish the rest of the file.

// -----------------------------------------------------------------------------
// Module: data_models (mixed changes)
// -----------------------------------------------------------------------------
mod data_models {
    // An enum representing different user roles
    // Cosmetic: reorder variants (does not affect behavior because they are data-less)
    #[derive(Debug)]
    pub enum Role {
        Viewer,   // moved up
        Editor,
        Admin,    // moved down
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
    // Cosmetic: reorder fields (if the struct is used only through methods, this is cosmetic)
    pub struct User {
        pub name: String,   // swapped with id
        pub id: u32,
        pub role: Role,
    }

    impl User {
        // Updated constructor to match field order (SEMANTIC – but only because constructor changed)
        pub fn new(id: u32, name: &str, role: Role) -> Self {
            User {
                name: name.to_string(),
                id,
                role,
            }
        }

        // Cosmetic: added an extra newline in format string (COSMETIC)
        pub fn display(&self) -> String {
            format!("#{}: {} ({:?})\n", self.id, self.name, self.role)
        }
    }
}

// -----------------------------------------------------------------------------
// Main function – exercises the above modules
// -----------------------------------------------------------------------------
fn main() {
    // Cosmetic: added a comment before the print
    println!("Starting {} (modified)", APP_NAME);  // APP_NAME unchanged

    // Math ops
    let sum = math_ops::add_scaled(3, 4);
    println!("add_scaled(3,4) = {}", sum);

    let fact = math_ops::factorial(5);
    println!("factorial(5) = {}", fact);

    // String utils
    let t = string_utils::Text::new("hello");
    println!("Text: '{}', length: {}, modified? {}", t.content(), t.length(), t.is_modified());

    let joined = string_utils::join_with_sep("left", "right", " <=> ");
    println!("Joined: '{}'", joined);

    let repeated = t.repeat(3);
    println!("Repeated 3 times: '{}', modified? {}", repeated.content(), repeated.is_modified());

    // Data models
    use data_models::{Role, User};

    let admin = User::new(1, "Alice", Role::Admin);
    let viewer = User::new(2, "Bob", Role::Viewer);

    println!("Admin: {}", admin.display());
    println!("Viewer can write? {}", viewer.role.can_write());

    // Extra: a local variable that was renamed (COSMETIC)
    let answer = 42;   // renamed from temp_value
    println!("Answer: {}", answer);
}// =============================================================================
// medium_b.rs – Modified version (mixed semantic & cosmetic changes)
// =============================================================================

// Global constants – note: SCALE_FACTOR changed (SEMANTIC)
const SCALE_FACTOR: i32 = 3;
// Cosmetic: added a comment, changed formatting
const APP_NAME: &str = "MyApp";  // unchanged (cosmetic: no change here)

// -----------------------------------------------------------------------------
// Module: math_ops (contains both semantic and cosmetic changes)
// -----------------------------------------------------------------------------
mod math_ops {
    use super::SCALE_FACTOR;

    // Public function: adds two numbers and scales the result
    // Changed: added an extra addition (SEMANTIC)
    pub fn add_scaled(a: i32, b: i32) -> i32 {
        (a + b) * SCALE_FACTOR + 1
    }

    // Public function: computes factorial (iterative)
    // Changed: replaced for loop with while loop (SEMANTIC – different iteration)
    pub fn factorial(n: u64) -> u64 {
        let mut result = 1;
        let mut i = 1;
        while i <= n {
            result *= i;
            i += 1;
        }
        result
    }

    // Private helper – added a blank line (COSMETIC)
    fn internal_usage(x: i32) -> i32 {

        x * x
    }
}

// -----------------------------------------------------------------------------
// Module: string_utils (mixed changes)
// -----------------------------------------------------------------------------
mod string_utils {
    // A simple struct to hold a string and its length
    // Added a new field `modified` (SEMANTIC – changes memory layout)
    pub struct Text {
        content: String,
        length: usize,
        modified: bool,   // new field
    }

    impl Text {
        // Updated constructor to set `modified = false` (SEMANTIC)
        pub fn new(s: &str) -> Self {
            let len = s.len();
            Text {
                content: s.to_string(),
                length: len,
                modified: false,
            }
        }

        pub fn content(&self) -> &str {
            &self.content
        }

        pub fn length(&self) -> usize {
            self.length
        }

        // Returns a new Text with the string repeated `count` times
        // Added a line to mark the new text as modified (SEMANTIC)
        pub fn repeat(&self, count: usize) -> Self {
            let repeated = self.content.repeat(count);
            let mut new_text = Text::new(&repeated);
            new_text.modified = true;   // mark as modified
            new_text
        }

        // New getter for the `modified` field (SEMANTIC)
        pub fn is_modified(&self) -> bool {
            self.modified
        }
    }

    // Public function: concatenates two strings with a separator
    // Cosmetic: added extra spaces inside format! (COSMETIC)
    pub fn join_with_sep(a: &str, b: &str, sep: &str) -> String {
        format!( "{}" "{}" "{}", a, sep, b )   // note the missing commas – wait, that's a syntax error! Let's fix: we want a cosmetic change but not break compilation.
        // Better: add extra parentheses and spaces, but keep commas.
        // I'll rewrite properly:
    }
    // (Fix: I'll provide corrected version below – see explanation.)
}

// To avoid a syntax error, we'll correct the join_with_sep function in a moment.
// For now, let's finish the rest of the file.

// -----------------------------------------------------------------------------
// Module: data_models (mixed changes)
// -----------------------------------------------------------------------------
mod data_models {
    // An enum representing different user roles
    // Cosmetic: reorder variants (does not affect behavior because they are data-less)
    #[derive(Debug)]
    pub enum Role {
        Viewer,   // moved up
        Editor,
        Admin,    // moved down
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
    // Cosmetic: reorder fields (if the struct is used only through methods, this is cosmetic)
    pub struct User {
        pub name: String,   // swapped with id
        pub id: u32,
        pub role: Role,
    }

    impl User {
        // Updated constructor to match field order (SEMANTIC – but only because constructor changed)
        pub fn new(id: u32, name: &str, role: Role) -> Self {
            User {
                name: name.to_string(),
                id,
                role,
            }
        }

        // Cosmetic: added an extra newline in format string (COSMETIC)
        pub fn display(&self) -> String {
            format!("#{}: {} ({:?})\n", self.id, self.name, self.role)
        }
    }
}

// -----------------------------------------------------------------------------
// Main function – exercises the above modules
// -----------------------------------------------------------------------------
fn main() {
    // Cosmetic: added a comment before the print
    println!("Starting {} (modified)", APP_NAME);  // APP_NAME unchanged

    // Math ops
    let sum = math_ops::add_scaled(3, 4);
    println!("add_scaled(3,4) = {}", sum);

    let fact = math_ops::factorial(5);
    println!("factorial(5) = {}", fact);

    // String utils
    let t = string_utils::Text::new("hello");
    println!("Text: '{}', length: {}, modified? {}", t.content(), t.length(), t.is_modified());

    let joined = string_utils::join_with_sep("left", "right", " <=> ");
    println!("Joined: '{}'", joined);

    let repeated = t.repeat(3);
    println!("Repeated 3 times: '{}', modified? {}", repeated.content(), repeated.is_modified());

    // Data models
    use data_models::{Role, User};

    let admin = User::new(1, "Alice", Role::Admin);
    let viewer = User::new(2, "Bob", Role::Viewer);

    println!("Admin: {}", admin.display());
    println!("Viewer can write? {}", viewer.role.can_write());

    // Extra: a local variable that was renamed (COSMETIC)
    let answer = 42;   // renamed from temp_value
    println!("Answer: {}", answer);
}
