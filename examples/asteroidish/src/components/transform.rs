use chaos_engine::math::{
    Vec2,
    matrix::{Mat3, Mat4},
};

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

    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    pub fn as_mat3(&self) -> Mat3 {
        let translation_matrix = Mat3::translation(self.position.x, self.position.y);
        let rotation_matrix = Mat3::rotation(self.rotation);
        let scale_matrix = Mat3::scale(self.scale.x, self.scale.y);

        scale_matrix * rotation_matrix * translation_matrix
    }

    pub fn as_mat4(&self) -> Mat4 {
        let translation_matrix = Mat4::translation(self.position.x, self.position.y, 0.0);
        let rotation_matrix = Mat4::rotation_z(self.rotation);
        let scale_matrix = Mat4::scale(self.scale.x, self.scale.y, 1.0);

        scale_matrix * rotation_matrix * translation_matrix
    }
}
