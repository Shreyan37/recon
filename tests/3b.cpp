enum Status {
    Active,
    Inactive,
    Paused, // renamed from Pending
    Error(String), // added variant with data
}

fn describe(status: Status) -> String {
    match status {
        Status::Active => "active".to_string(),
        Status::Inactive => "inactive".to_string(),
        Status::Paused => "paused".to_string(),
        Status::Error(msg) => format!("error: {}", msg),
    }
}
