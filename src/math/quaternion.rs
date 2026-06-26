use std::ops::{self, Mul, MulAssign};

use vulkano_macros::BufferContents;

use crate::math::{
    matrix::Mat4,
    vector::{vec3::Vec3, vec4::Vec4},
};

#[derive(Debug, Clone, Copy, BufferContents)]
#[repr(C)]
pub struct Quaternion {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
}

impl Quaternion {
    pub fn identity() -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
        }
    }

    pub fn from_axis_angle(axis: &Vec3, angle: f32) -> Self {
        let half_angle = angle / 2.0;
        let sin_half_angle = half_angle.sin();
        let cos_half_angle = half_angle.cos();

        Self {
            a: axis.x * sin_half_angle,
            b: axis.y * sin_half_angle,
            c: axis.z * sin_half_angle,
            d: cos_half_angle,
        }
    }

    pub fn from_euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        let (sr, cr) = (roll * 0.5).sin_cos();
        let (sp, cp) = (pitch * 0.5).sin_cos();
        let (sy, cy) = (yaw * 0.5).sin_cos();

        Self {
            a: sr * cp * cy - cr * sp * sy,
            b: cr * sp * cy + sr * cp * sy,
            c: cr * cp * sy - sr * sp * cy,
            d: cr * cp * cy + sr * sp * sy,
        }
    }

    pub fn normalize(&mut self) {
        let length = (self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d).sqrt();
        if length != 0.0 {
            self.a /= length;
            self.b /= length;
            self.c /= length;
            self.d /= length;
        }
    }
}

impl From<Vec4> for Quaternion {
    fn from(vec: Vec4) -> Self {
        Self {
            a: vec.x,
            b: vec.y,
            c: vec.z,
            d: vec.w,
        }
    }
}

impl From<[f32; 4]> for Quaternion {
    fn from(arr: [f32; 4]) -> Self {
        Self {
            a: arr[0],
            b: arr[1],
            c: arr[2],
            d: arr[3],
        }
    }
}

impl Mul<Quaternion> for Quaternion {
    type Output = Self;

    fn mul(self, other: Quaternion) -> Self::Output {
        Self {
            a: self.d * other.a + self.a * other.d + self.b * other.c - self.c * other.b,
            b: self.d * other.b - self.a * other.c + self.b * other.d + self.c * other.a,
            c: self.d * other.c + self.a * other.b - self.b * other.a + self.c * other.d,
            d: self.d * other.d - self.a * other.a - self.b * other.b - self.c * other.c,
        }
    }
}

impl MulAssign<Quaternion> for Quaternion {
    fn mul_assign(&mut self, other: Quaternion) {
        *self = *self * other;
    }
}
