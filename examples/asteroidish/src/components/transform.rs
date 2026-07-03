use chaos_engine::math::{Vec2, matrix::Mat4};

pub struct TransformComponent {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl TransformComponent {
    pub fn new() -> Self {
        Self {
            position: Vec2::zero(),
            rotation: 0.0,
            scale: Vec2::one(),
        }
    }

    pub fn as_matrix(&self) -> Mat4 {
        let translation_matrix = Mat4::translation(self.position.x, self.position.y, 0.0);
        let rotation_matrix = Mat4::rotation_z(self.rotation);
        let scale_matrix = Mat4::scale(self.scale.x, self.scale.y, 1.0);

        translation_matrix * rotation_matrix * scale_matrix
    }
}
