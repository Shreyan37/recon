mod outer {
    pub struct Container {
        items: Vec<Item>,
        name: String, // added field
    }

    struct Item {
        id: u32,
        description: String, // added field
    }
}
