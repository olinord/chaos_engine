use std::ops::{Add, AddAssign, Div, Mul};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Matrix3x3{
    pub r1c1: f32,

}

impl Vec2 {

    pub fn zero() -> Self {
        return Vec2{x: 0.0, y: 0.0}
    }

    pub fn dist(first: &Self, second: &Self) -> f32 {
        ((first.x - second.x).powi(2) - (first.y - second.y).powi(2)).sqrt()
    }

    pub fn scale(first: &Self, scale: f32) -> Self {
        return Self{ x: first.x * scale, y: first.y * scale };
    }

    pub fn add(&mut self, other: &Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

// float multiplication
impl Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, rhs: f32) -> Self::Output {
        Vec2{x: self.x * rhs, y: self.y * rhs}
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;

    fn mul(self, rhs: Vec2) -> Self::Output {
        Vec2{x: rhs.x * self, y: rhs.y * self}
    }
}

impl AddAssign for Vec2{
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Add<Vec2> for Vec2 {
    type Output = Vec2;

    fn add(self, rhs: Self) -> Self::Output {
        Vec2{ x: self.x + rhs.x, y: self.y + rhs.y}
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;

    fn div(self, rhs: f32) -> Self::Output {
        Vec2{x: self.x/rhs, y: self.y/rhs}
    }
}