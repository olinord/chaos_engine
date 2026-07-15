use chaos_engine::math::{Vec2, Vec3, matrix::Mat4};
pub struct CameraComponent {
    pub eye: Vec3,
    pub target: Vec3,
    pub rotation: f32,
    pub aspect_ratio: f32,
    pub fov: f32,
    pub near_clip: f32,
    pub far_clip: f32,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
}

const DEFAULT_FOV: f32 = std::f32::consts::PI / 2.0;
const DEFAULT_NEAR_CLIP: f32 = 0.1;
const DEFAULT_FAR_CLIP: f32 = 100.0;
const DEFAULT_CAMERA_DISTANCE: f32 = -25.0;
const RENDERING_PLANE: f32 = 0.0;

impl CameraComponent {
    pub fn new(eye: Vec2, target: Vec2, rotation: f32, aspect_ratio: f32) -> Self {
        Self {
            eye: Vec3::new(eye.x, eye.y, DEFAULT_CAMERA_DISTANCE),
            target: Vec3::new(target.x, target.y, RENDERING_PLANE),
            rotation,
            aspect_ratio: aspect_ratio,
            fov: DEFAULT_FOV,
            near_clip: DEFAULT_NEAR_CLIP,
            far_clip: DEFAULT_FAR_CLIP,
            view_matrix: Mat4::look_at(
                &Vec3::new(eye.x, eye.y, DEFAULT_CAMERA_DISTANCE),
                &Vec3::new(target.x, target.y, RENDERING_PLANE),
                &Vec3::y_axis(),
            ),
            projection_matrix: Mat4::perspective_projection(
                DEFAULT_FOV,
                aspect_ratio,
                DEFAULT_NEAR_CLIP,
                DEFAULT_FAR_CLIP,
            ),
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;

        self.projection_matrix = Mat4::perspective_projection(
            self.fov,
            self.aspect_ratio,
            self.near_clip,
            self.far_clip,
        );
    }

    pub fn update(&mut self, eye: Vec2, target: Vec2, rotation: f32) {
        self.eye = Vec3::new(eye.x, eye.y, DEFAULT_CAMERA_DISTANCE);
        self.target = Vec3::new(target.x, target.y, RENDERING_PLANE);
        self.rotation = rotation;

        self.view_matrix = Mat4::look_at(&self.eye, &self.target, &Vec3::y_axis());
    }
}
