fn main() {
    let x = 42;
    unsafe {
        let _raw = &x as *const i32;
    }
}
