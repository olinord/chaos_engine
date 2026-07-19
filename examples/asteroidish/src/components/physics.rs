use chaos_engine::math::Vec2;

pub struct PhysicsComponent {
    pub velocity: Vec2,     // The current velocity of the entity m/s
    pub acceleration: Vec2, // The current acceleration of the entity m/s^2
    pub mass: f32,          // The mass of the entity in kg
}

impl PhysicsComponent {
    pub fn new() -> Self {
        Self {
            velocity: Vec2::zero(),
            acceleration: Vec2::zero(),
            mass: 1.0,
        }
    }

    pub fn with_velocity(mut self, velocity: Vec2) -> Self {
        self.velocity = velocity;
        self
    }

    pub fn with_acceleration(mut self, acceleration: Vec2) -> Self {
        self.acceleration = acceleration;
        self
    }

    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass;
        self
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update velocity based on acceleration
        self.velocity += self.acceleration * delta_time;

        // Reset acceleration after applying it to velocity
        self.acceleration = Vec2::zero();
    }

    pub fn get_kinetic_force(&self) -> Vec2 {
        self.velocity * self.mass
    }

    pub fn apply_force(&mut self, force: Vec2) {
        // F = m * a => a = F / m
        let acceleration = force / self.mass;
        self.acceleration += acceleration;
    }
}
