use chaos_engine::math::Vec2;

pub struct BoundingCircle {
    pub _center: Vec2,
    pub _radius: f32,
}

impl BoundingCircle {
    pub fn new() -> Self {
        Self {
            _center: Vec2::zero(),
            _radius: 0.0,
        }
    }
}
