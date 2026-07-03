use chaos_engine::math::Vec2;

pub struct BoundingCircle {
    pub center: Vec2,
    pub radius: f32,
}

impl BoundingCircle {
    pub fn new() -> Self {
        Self {
            center: Vec2::zero(),
            radius: 0.0,
        }
    }
}
