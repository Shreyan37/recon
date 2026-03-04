fn main() {
    let v = vec![1, 2, 3];
    let closure = || println!("{:?}", v);
    closure();
    println!("still have v: {:?}", v);
}
