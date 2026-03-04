fn classify(n: i32) -> &'static str {
    if n < 0 {
        "negative"
    } else if n == 0 {
        "zero"
    } else if n == 42 {
        "answer"
    } else {
        "positive"
    }
}
