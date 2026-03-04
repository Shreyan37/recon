enum Status {
    Active,
    Inactive,
    Pending,
}

fn describe(status: Status) -> String {
    match status {
        Status::Active => "active".to_string(),
        Status::Inactive => "inactive".to_string(),
        Status::Pending => "pending".to_string(),
    }
}
