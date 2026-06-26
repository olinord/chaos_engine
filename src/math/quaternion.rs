use std::ops::{Mul, MulAssign};

use vulkano_macros::BufferContents;

use crate::math::vector::{vec3::Vec3, vec4::Vec4};

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

    pub fn normalized(quaternion: Self) -> Self {
        let length = quaternion.a * quaternion.a
            + quaternion.b * quaternion.b
            + quaternion.c * quaternion.c
            + quaternion.d * quaternion.d;
        if length != 0.0 {
            let length = length.sqrt();
            Self {
                a: quaternion.a / length,
                b: quaternion.b / length,
                c: quaternion.c / length,
                d: quaternion.d / length,
            }
        } else {
            Self::identity()
        }
    }

    pub fn lerp(quat1: Self, quat2: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let a = quat1.a * (1.0 - t) + quat2.a * t;
        let b = quat1.b * (1.0 - t) + quat2.b * t;
        let c = quat1.c * (1.0 - t) + quat2.c * t;
        let d = quat1.d * (1.0 - t) + quat2.d * t;

        Self { a, b, c, d }
    }

    pub fn slerp(quat1: Self, quat2: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let dot = quat1.a * quat2.a + quat1.b * quat2.b + quat1.c * quat2.c + quat1.d * quat2.d;

        if dot < 0.0 {
            return Self::slerp(
                Self {
                    a: -quat1.a,
                    b: -quat1.b,
                    c: -quat1.c,
                    d: -quat1.d,
                },
                quat2,
                t,
            );
        }

        if dot > 0.9995 {
            return Self::lerp(quat1, quat2, t);
        }

        let theta_0 = dot.acos();
        let theta = theta_0 * t;
        let sin_theta = theta.sin();
        let sin_theta_0 = theta_0.sin();

        let s0 = (theta.cos() - dot * sin_theta / sin_theta_0) / sin_theta_0;
        let s1 = sin_theta / sin_theta_0;

        Self {
            a: s0 * quat1.a + s1 * quat2.a,
            b: s0 * quat1.b + s1 * quat2.b,
            c: s0 * quat1.c + s1 * quat2.c,
            d: s0 * quat1.d + s1 * quat2.d,
        }
    }

    pub fn inverse(&self) -> Self {
        let norm_sq = self.a * self.a + self.b * self.b + self.c * self.c + self.d * self.d;
        if norm_sq != 0.0 {
            let inv_norm_sq = 1.0 / norm_sq;
            Self {
                a: -self.a * inv_norm_sq,
                b: -self.b * inv_norm_sq,
                c: -self.c * inv_norm_sq,
                d: self.d * inv_norm_sq,
            }
        } else {
            Self::identity()
        }
    }

    pub fn conjugate(&self) -> Self {
        Self {
            a: -self.a,
            b: -self.b,
            c: -self.c,
            d: self.d,
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
