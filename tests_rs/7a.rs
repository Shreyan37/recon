fn sum_up_to(n: usize) -> usize {
    let mut sum = 0;
    for i in 0..=n {
        sum += i;
    }
    sum
}
