fn process(n: i32) -> &'static str {
    if n > 0 {
        "positive"
    } else if n == 0 {
        "zero"
    } else {
        "negative"
    }
}
