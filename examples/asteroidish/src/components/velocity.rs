use chaos_engine::math::Vec2;

pub struct VelocityComponent {
    pub velocity: Vec2,
}

impl VelocityComponent {
    pub fn new() -> Self {
        Self {
            velocity: Vec2::zero(),
        }
    }
}
