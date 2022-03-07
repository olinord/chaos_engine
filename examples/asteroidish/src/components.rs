use std::cmp::min;
use std::sync::{Arc, Mutex};
use rand::Rng;
use chaos_engine::rendering::buffer::Buffer;
use crate::commands::QuadVertex;
use crate::math::Vec2;
use chaos_engine::ChaosBackend;

pub struct CollisionComponent {
    bounding_radius: f32,
}

pub struct PhysicsComponent {
    position: Vec2, // meters
    velocity: Vec2, // m/s
    momentum: Vec2, // kgm/s
    mass: f32, // kg
    rotation: f32, // rad
    angular_momentum: f32 // rad/s
}

pub struct VoxelData {
    width: usize,
    height: usize,
    voxel_data: Vec<bool>,
    voxel_size: f32,
    vertex_buffer: Option<Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>>>
}
///
/// Returns a percentage chance of a x,y coord is within a ellipse with width height radii
///
fn get_percentage(x: f32, y: f32, width: f32, height: f32) -> f64 {
    let half_width = width / 2.0;
    let half_height = height / 2.0;
    let ellipse_check = (x - half_width).powf(2.0)/half_width.powf(2.0) + (y - half_height).powf(2.0)/half_height.powf(2.0);

    if ellipse_check >= 1.0 {
        0.0 as f64
    }
    else {
        (1.0 - ellipse_check) as f64
    }
}

impl VoxelData {
    ///
    /// Generates a random ellipse voxel with width and height
    ///
    pub fn generate_random_asteroid(width: usize, height: usize) -> VoxelData {
        assert_ne!(width, 0);
        assert_ne!(height, 0);
        let mut vd = VoxelData::new(width, height);
        let mut rng = rand::thread_rng();

        let height_f32 = height as f32;
        let width_f32 = width as f32;

        vd.set_filled_pixels((0..(width * height)).map(|i| {
            let i_f32 = i as f32;
            let x = i_f32 % width_f32;
            let y = i_f32 / width_f32;

            rng.gen_bool( get_percentage(x, y, width_f32, height_f32))
        }).collect());
        vd.construct_vertex_buffer();
        return vd;
    }

    pub fn new(width: usize, height: usize) -> Self{
        let mut voxel_data = Vec::with_capacity(width * height);
        voxel_data.fill(false);

        return Self{
            width,
            height,
            voxel_data,
            voxel_size: 0.01,
            vertex_buffer: None
        };
    }

    pub fn set_filled_pixels(&mut self, new_voxel_data: Vec<bool>){
        if new_voxel_data.len() != self.width * self.height {
            panic!("Trying to fill VoxelData with data that is not {} but {}", self.width * self.height, new_voxel_data.len());
        }

        self.voxel_data = new_voxel_data;
        self.drop_buffer();
    }

    pub fn set_voxel(&mut self, x: usize, y: usize, filled: bool) {
        assert!(x < self.width);
        assert!(y < self.height);
        self.voxel_data[x + y * self.width] = filled;
        self.drop_buffer();
    }

    fn drop_buffer(&mut self) {
        // if let Some(vb) = self.vertex_buffer {
        // std::mem::drop((self.vertex_buffer.unwrap()));
        self.vertex_buffer = None;
        // }
    }

    fn create_quad(&self, x: f32, y: f32) -> Vec<QuadVertex> {
        let half_size = self.voxel_size / 2.0;
        let mod_x = half_size * x;
        let mod_y = half_size * y;
        vec! {
            QuadVertex { pos: [mod_x - half_size, mod_y + half_size] },
            QuadVertex { pos: [mod_x + half_size, mod_y + half_size] },
            QuadVertex { pos: [mod_x + half_size, mod_y - half_size] },
            QuadVertex { pos: [mod_x - half_size, mod_y + half_size] },
            QuadVertex { pos: [mod_x + half_size, mod_y - half_size] },
            QuadVertex { pos: [mod_x - half_size, mod_y - half_size] },
        }
    }

    pub fn construct_vertex_buffer(&mut self) {
        let vertices = self.voxel_data.iter().enumerate().filter_map(| (index, voxel_data) | {
            if *voxel_data {
                Some(self.create_quad((index % self.width) as f32, (index / self.width) as f32))
            }
            else{
                None
            }
        }).flatten().collect();
        self.vertex_buffer = Some(Arc::new(Mutex::new(Buffer::<ChaosBackend, QuadVertex>::new(vertices))));
    }

    pub fn get_vertex_buffer(&self) -> Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>> {
        if let Some(vb) = &self.vertex_buffer {
            return vb.clone();
        }
        panic!("Requested a vertex buffer from VoxelData when it has not been set")
    }
}

pub struct AsteroidRenderComponent {
    voxel_data: VoxelData
}

impl AsteroidRenderComponent{
    pub fn new(voxel_data: VoxelData) -> Self{
        return Self{voxel_data}
    }

    pub fn get_voxel_data(&mut self) -> &VoxelData {
        return &self.voxel_data;
    }

    pub fn get_vertex_buffer(&self) -> Arc<Mutex<Buffer<ChaosBackend, QuadVertex>>> {
        return self.voxel_data.get_vertex_buffer();
    }
}

impl CollisionComponent{
    pub fn new(bounding_radius: f32) -> Self{
        return CollisionComponent{
            bounding_radius
        }
    }

    pub fn get_bounding_radius(&self) -> f32 {
        self.bounding_radius
    }
}

impl PhysicsComponent {
    pub fn new(position: Vec2, velocity: Vec2, mass: f32) -> Self{
        return PhysicsComponent {
            position,
            velocity,
            momentum: Vec2::zero(),
            mass,
            rotation: 0.0,
            angular_momentum: 0.0
        }
    }

    pub fn get_position(&self) -> &Vec2{
        &self.position
    }

    pub fn update(&mut self, delta_time: f32) {
        // convert momentum to velocity
        if self.momentum.x != 0.0 || self.momentum.y != 0.0 {
            self.velocity += self.momentum * self.mass;
        }

        self.position.add(&Vec2::scale(&self.velocity, delta_time));
        if self.position.x > 1.0 || self.position.x < -1.0 {
            self.position.x *= -1.0;
        }
        if self.position.y > 1.0 || self.position.y < -1.0 {
            self.position.y *= -1.0;
        }
    }

    pub fn collide(&mut self, other: &Self) {
        // https://en.wikipedia.org/wiki/Elastic_collision
        let u1 = self.velocity;
        let u2 = other.velocity;
        let m1 = self.mass;
        let m2 = other.mass;

        let v1 = ((m1 - m2)/(m1 + m2)) * u1 + ((2.0 * m2) / (m1 + m2)) * u2;

        self.momentum += v1 / self.mass;
    }
}