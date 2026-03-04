struct Point {
    x: f64,
    y: f64,
    z: f64, // added field
}

impl Point {
    fn new(x: f64, y: f64, z: f64) -> Self { // added parameter
        Point { x, y, z }
    }

    fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z; // added dimension
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    // new method
    fn origin() -> Self {
        Point { x: 0.0, y: 0.0, z: 0.0 }
    }
}
