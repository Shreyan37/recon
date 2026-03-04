async fn fetch() -> String {
    let intermediate = some_other_async().await;
    format!("{}-data", intermediate)
}

async fn some_other_async() -> String {
    "prefix".to_string()
}
