use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use vulkano_macros::BufferContents;

use crate::math::vector::vec2::Vec2;
use crate::math::vector::vec4::Vec4;

#[derive(Debug, Clone, Copy, PartialEq, BufferContents)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    /// Creates a new `Vec3` with the given `x`, `y`, and `z` components.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let v = Vec3::new(1.0, 2.0, 3.0);
    /// assert_eq!(v, Vec3 { x: 1.0, y: 2.0, z: 3.0 });
    /// ```
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Returns the zero vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::zero(), Vec3 { x: 0.0, y: 0.0, z: 0.0 });
    /// ```
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Returns a vector with every component set to `1.0`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::one(), Vec3 { x: 1.0, y: 1.0, z: 1.0 });
    /// ```
    pub const fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        }
    }

    /// Returns the unit vector along the positive x axis.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::x_axis(), Vec3 { x: 1.0, y: 0.0, z: 0.0 });
    /// ```
    pub const fn x_axis() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Returns the unit vector along the positive y axis.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::y_axis(), Vec3 { x: 0.0, y: 1.0, z: 0.0 });
    /// ```
    pub const fn y_axis() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }

    /// Returns the unit vector along the positive z axis.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::z_axis(), Vec3 { x: 0.0, y: 0.0, z: 1.0 });
    /// ```
    pub const fn z_axis() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }

    /// Returns a vector with every component set to `value`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3::splat(2.5), Vec3 { x: 2.5, y: 2.5, z: 2.5 });
    /// ```
    pub const fn splat(value: f32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
        }
    }

    // non member functions
    /// Returns a normalized copy of `v`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let normalized = Vec3::normalized(&Vec3 { x: 0.0, y: 3.0, z: 4.0 });
    /// assert!((normalized.length() - 1.0).abs() < 0.00001);
    /// ```
    pub fn normalized(v: &Self) -> Self {
        let mut ret = v.clone();
        ret.normalize();
        ret
    }

    /// Returns the Euclidean distance between two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let distance = Vec3::distance(&Vec3::zero(), &Vec3 { x: 0.0, y: 3.0, z: 4.0 });
    /// assert_eq!(distance, 5.0);
    /// ```
    pub fn distance(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        let dz = one.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Returns the squared Euclidean distance between two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let distance_squared = Vec3::distance_squared(&Vec3::zero(), &Vec3 { x: 0.0, y: 3.0, z: 4.0 });
    /// assert_eq!(distance_squared, 25.0);
    /// ```
    pub fn distance_squared(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        let dz = one.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    /// Returns the dot product of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let dot = Vec3::dot(&Vec3 { x: 1.0, y: 2.0, z: 3.0 }, &Vec3 { x: 4.0, y: 5.0, z: 6.0 });
    /// assert_eq!(dot, 32.0);
    /// ```
    pub fn dot(one: &Self, other: &Self) -> f32 {
        one.x * other.x + one.y * other.y + one.z * other.z
    }

    /// Returns the 3D cross product of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let cross = Vec3::cross(&Vec3::x_axis(), &Vec3::y_axis());
    /// assert_eq!(cross, Vec3::z_axis());
    /// ```
    pub fn cross(one: &Self, other: &Self) -> Self {
        Self {
            x: one.y * other.z - one.z * other.y,
            y: one.z * other.x - one.x * other.z,
            z: one.x * other.y - one.y * other.x,
        }
    }

    /// Linearly interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::lerp(&Vec3::zero(), &Vec3 { x: 10.0, y: 20.0, z: 30.0 }, 0.25);
    /// assert_eq!(result, Vec3 { x: 2.5, y: 5.0, z: 7.5 });
    /// ```
    pub fn lerp(one: &Self, other: &Self, t: f32) -> Self {
        Self {
            x: one.x + (other.x - one.x) * t,
            y: one.y + (other.y - one.y) * t,
            z: one.z + (other.z - one.z) * t,
        }
    }

    /// Spherically interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::slerp(&Vec3::x_axis(), &Vec3::y_axis(), 0.5);
    /// assert!((result.x - 0.70710677).abs() < 0.00001);
    /// assert!((result.y - 0.70710677).abs() < 0.00001);
    /// assert_eq!(result.z, 0.0);
    /// ```
    pub fn slerp(one: &Self, other: &Self, t: f32) -> Self {
        let dot = Self::dot(one, other);
        let theta = dot.acos();
        let sin_theta = theta.sin();

        if sin_theta == 0.0 {
            return *one;
        }

        let scale_one = ((1.0 - t) * theta).sin() / sin_theta;
        let scale_other = (t * theta).sin() / sin_theta;

        Self {
            x: scale_one * one.x + scale_other * other.x,
            y: scale_one * one.y + scale_other * other.y,
            z: scale_one * one.z + scale_other * other.z,
        }
    }

    /// Linearly interpolates from `one` to `other` by `t`, then normalizes the result.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::nlerp(&Vec3::x_axis(), &Vec3::y_axis(), 0.5);
    /// assert!((result.length() - 1.0).abs() < 0.00001);
    /// ```
    pub fn nlerp(one: &Self, other: &Self, t: f32) -> Self {
        let mut result = Self::lerp(one, other, t);
        result.normalize();
        result
    }

    /// Returns the angle in radians between two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let angle = Vec3::angle(&Vec3::x_axis(), &Vec3::y_axis());
    /// assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
    /// ```
    pub fn angle(one: &Self, other: &Self) -> f32 {
        let dot = Self::dot(one, other);
        dot.acos()
    }

    /// Reflects an incident vector around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let reflected = Vec3::reflect(&Vec3 { x: 1.0, y: -1.0, z: 2.0 }, &Vec3::y_axis());
    /// assert_eq!(reflected, Vec3 { x: 1.0, y: 1.0, z: 2.0 });
    /// ```
    pub fn reflect(incident: &Self, normal: &Self) -> Self {
        let dot = Self::dot(incident, normal);
        *incident - *normal * (2.0 * dot)
    }

    /// Projects vector `a` onto vector `b`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let projection = Vec3::project(&Vec3 { x: 2.0, y: 3.0, z: 4.0 }, &Vec3::z_axis());
    /// assert_eq!(projection, Vec3 { x: 0.0, y: 0.0, z: 4.0 });
    /// ```
    pub fn project(a: &Self, b: &Self) -> Self {
        let dot_product = Self::dot(a, b);
        let b_length_squared = b.length_squared();
        if b_length_squared != 0.0 {
            *b * (dot_product / b_length_squared)
        } else {
            Self::zero()
        }
    }

    /// Returns the component-wise minimum of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::min(&Vec3 { x: 1.0, y: 4.0, z: 2.0 }, &Vec3 { x: 2.0, y: 3.0, z: 5.0 });
    /// assert_eq!(result, Vec3 { x: 1.0, y: 3.0, z: 2.0 });
    /// ```
    pub fn min(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.min(other.x),
            y: one.y.min(other.y),
            z: one.z.min(other.z),
        }
    }

    /// Returns the component-wise maximum of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::max(&Vec3 { x: 1.0, y: 4.0, z: 2.0 }, &Vec3 { x: 2.0, y: 3.0, z: 5.0 });
    /// assert_eq!(result, Vec3 { x: 2.0, y: 4.0, z: 5.0 });
    /// ```
    pub fn max(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.max(other.x),
            y: one.y.max(other.y),
            z: one.z.max(other.z),
        }
    }

    /// Clamps each component of `value` between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let result = Vec3::clamp(&Vec3 { x: -1.0, y: 0.5, z: 3.0 }, &Vec3::zero(), &Vec3::one());
    /// assert_eq!(result, Vec3 { x: 0.0, y: 0.5, z: 1.0 });
    /// ```
    pub fn clamp(value: &Self, min: &Self, max: &Self) -> Self {
        Self {
            x: value.x.clamp(min.x, max.x),
            y: value.y.clamp(min.y, max.y),
            z: value.z.clamp(min.z, max.z),
        }
    }

    // member functions
    /// Normalizes this vector in place if its length is non-zero.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let mut vector = Vec3 { x: 0.0, y: 3.0, z: 4.0 };
    /// vector.normalize();
    /// assert!((vector.length() - 1.0).abs() < 0.00001);
    /// ```
    pub fn normalize(&mut self) {
        let length = self.length_squared();
        if length != 0.0 {
            let inv_length = 1.0 / length.sqrt();
            self.x *= inv_length;
            self.y *= inv_length;
            self.z *= inv_length;
        }
    }

    /// Reflects this vector in place around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let mut vector = Vec3 { x: 1.0, y: -1.0, z: 2.0 };
    /// vector.reflected(&Vec3::y_axis());
    /// assert_eq!(vector, Vec3 { x: 1.0, y: 1.0, z: 2.0 });
    /// ```
    pub fn reflected(&mut self, normal: &Self) {
        let dot = Self::dot(self, normal);
        let translation = *normal * (2.0 * dot);
        *self -= translation;
    }

    /// Clamps this vector in place between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// let mut vector = Vec3 { x: -1.0, y: 0.5, z: 3.0 };
    /// vector.clamped(&Vec3::zero(), &Vec3::one());
    /// assert_eq!(vector, Vec3 { x: 0.0, y: 0.5, z: 1.0 });
    /// ```
    pub fn clamped(&mut self, min: &Self, max: &Self) {
        self.x = self.x.clamp(min.x, max.x);
        self.y = self.y.clamp(min.y, max.y);
        self.z = self.z.clamp(min.z, max.z);
    }

    /// Returns a `Vec4` with the same `x`, `y`, and `z` components as this vector, and the given `w` component.
    pub fn as_vec4(&self, w: f32) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, w)
    }

    /// Returns this vector's Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3 { x: 0.0, y: 3.0, z: 4.0 }.length(), 5.0);
    /// ```
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Returns this vector's squared Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec3;
    ///
    /// assert_eq!(Vec3 { x: 0.0, y: 3.0, z: 4.0 }.length_squared(), 25.0);
    /// ```
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    // swizzling methods
    impl_swizzle2_all!((x, y, z), Vec2; x, y, z);
    impl_swizzle3_all!((x, y, z), Vec3; x, y, z);
    impl_swizzle4_all!((x, y, z), Vec4; x, y, z);
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
        self.z *= scalar;
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = Self;

    fn mul(self, other: Vec3) -> Self::Output {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

impl MulAssign<Vec3> for Vec3 {
    fn mul_assign(&mut self, other: Vec3) {
        self.x *= other.x;
        self.y *= other.y;
        self.z *= other.z;
    }
}

impl Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x / scalar,
            y: self.y / scalar,
            z: self.z / scalar,
        }
    }
}

impl DivAssign<f32> for Vec3 {
    fn div_assign(&mut self, scalar: f32) {
        self.x /= scalar;
        self.y /= scalar;
        self.z /= scalar;
    }
}

impl Div<Vec3> for Vec3 {
    type Output = Self;

    fn div(self, other: Vec3) -> Self::Output {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
        }
    }
}

impl DivAssign<Vec3> for Vec3 {
    fn div_assign(&mut self, other: Vec3) {
        self.x /= other.x;
        self.y /= other.y;
        self.z /= other.z;
    }
}

impl Neg for Vec3 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl_vector_reference_ops!(Vec3);

impl From<[f32; 3]> for Vec3 {
    fn from(arr: [f32; 3]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.00001;

    fn assert_f32_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    fn assert_vec3_eq(actual: Vec3, expected: Vec3) {
        assert_f32_eq(actual.x, expected.x);
        assert_f32_eq(actual.y, expected.y);
        assert_f32_eq(actual.z, expected.z);
    }

    #[test]
    fn vec3_constructors_return_expected_values() {
        assert_eq!(
            Vec3::zero(),
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(
            Vec3::one(),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0
            }
        );
        assert_eq!(
            Vec3::x_axis(),
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(
            Vec3::y_axis(),
            Vec3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            }
        );
        assert_eq!(
            Vec3::z_axis(),
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0
            }
        );
        assert_eq!(
            Vec3::splat(2.5),
            Vec3 {
                x: 2.5,
                y: 2.5,
                z: 2.5
            }
        );
        assert_eq!(
            Vec3::from([1.0, 2.0, 3.0]),
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0
            }
        );
    }

    #[test]
    fn vec3_swizzling_methods_return_expected_vectors() {
        let vector = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };

        assert_eq!(vector.xx(), Vec2 { x: 1.0, y: 1.0 });
        assert_eq!(vector.xy(), Vec2 { x: 1.0, y: 2.0 });
        assert_eq!(vector.xz(), Vec2 { x: 1.0, y: 3.0 });
        assert_eq!(vector.yx(), Vec2 { x: 2.0, y: 1.0 });
        assert_eq!(vector.yy(), Vec2 { x: 2.0, y: 2.0 });
        assert_eq!(vector.yz(), Vec2 { x: 2.0, y: 3.0 });
        assert_eq!(vector.zx(), Vec2 { x: 3.0, y: 1.0 });
        assert_eq!(vector.zy(), Vec2 { x: 3.0, y: 2.0 });
        assert_eq!(vector.zz(), Vec2 { x: 3.0, y: 3.0 });

        assert_eq!(
            vector.xxx(),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            }
        );
        assert_eq!(
            vector.xyz(),
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }
        );
        assert_eq!(
            vector.xzy(),
            Vec3 {
                x: 1.0,
                y: 3.0,
                z: 2.0,
            }
        );
        assert_eq!(
            vector.yxz(),
            Vec3 {
                x: 2.0,
                y: 1.0,
                z: 3.0,
            }
        );
        assert_eq!(
            vector.yyy(),
            Vec3 {
                x: 2.0,
                y: 2.0,
                z: 2.0,
            }
        );
        assert_eq!(
            vector.yzx(),
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 1.0,
            }
        );
        assert_eq!(
            vector.zxy(),
            Vec3 {
                x: 3.0,
                y: 1.0,
                z: 2.0,
            }
        );
        assert_eq!(
            vector.zyx(),
            Vec3 {
                x: 3.0,
                y: 2.0,
                z: 1.0,
            }
        );
        assert_eq!(
            vector.zzz(),
            Vec3 {
                x: 3.0,
                y: 3.0,
                z: 3.0,
            }
        );

        assert_eq!(
            vector.xxyz(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                w: 3.0,
            }
        );
        assert_eq!(
            vector.xyzx(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.xzyz(),
            Vec4 {
                x: 1.0,
                y: 3.0,
                z: 2.0,
                w: 3.0,
            }
        );
        assert_eq!(
            vector.yxzy(),
            Vec4 {
                x: 2.0,
                y: 1.0,
                z: 3.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.yzyx(),
            Vec4 {
                x: 2.0,
                y: 3.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.zxyz(),
            Vec4 {
                x: 3.0,
                y: 1.0,
                z: 2.0,
                w: 3.0,
            }
        );
        assert_eq!(
            vector.zyxz(),
            Vec4 {
                x: 3.0,
                y: 2.0,
                z: 1.0,
                w: 3.0,
            }
        );
        assert_eq!(
            vector.zzzz(),
            Vec4 {
                x: 3.0,
                y: 3.0,
                z: 3.0,
                w: 3.0,
            }
        );
    }

    #[test]
    fn vec3_measurement_and_normalization_methods_work() {
        let vector = Vec3 {
            x: 0.0,
            y: 3.0,
            z: 4.0,
        };

        assert_eq!(vector.length(), 5.0);
        assert_eq!(vector.length_squared(), 25.0);
        assert_eq!(Vec3::distance(&Vec3::zero(), &vector), 5.0);
        assert_eq!(Vec3::distance_squared(&Vec3::zero(), &vector), 25.0);
        assert_eq!(
            Vec3::dot(
                &Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0
                },
                &Vec3 {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0
                }
            ),
            32.0
        );
        assert_eq!(
            Vec3::cross(&Vec3::x_axis(), &Vec3::y_axis()),
            Vec3::z_axis()
        );
        assert_f32_eq(
            Vec3::angle(&Vec3::x_axis(), &Vec3::y_axis()),
            std::f32::consts::FRAC_PI_2,
        );

        let normalized = Vec3::normalized(&vector);
        assert_f32_eq(normalized.length(), 1.0);

        let mut mutable = vector;
        mutable.normalize();
        assert_vec3_eq(mutable, normalized);

        let mut zero = Vec3::zero();
        zero.normalize();
        assert_eq!(zero, Vec3::zero());
    }

    #[test]
    fn vec3_interpolation_projection_reflection_and_clamping_methods_work() {
        assert_vec3_eq(
            Vec3::lerp(
                &Vec3::zero(),
                &Vec3 {
                    x: 10.0,
                    y: 20.0,
                    z: 30.0,
                },
                0.25,
            ),
            Vec3 {
                x: 2.5,
                y: 5.0,
                z: 7.5,
            },
        );

        let slerped = Vec3::slerp(&Vec3::x_axis(), &Vec3::y_axis(), 0.5);
        assert_vec3_eq(
            slerped,
            Vec3 {
                x: std::f32::consts::FRAC_1_SQRT_2,
                y: std::f32::consts::FRAC_1_SQRT_2,
                z: 0.0,
            },
        );

        let nlerped = Vec3::nlerp(&Vec3::x_axis(), &Vec3::y_axis(), 0.5);
        assert_f32_eq(nlerped.length(), 1.0);

        assert_eq!(
            Vec3::reflect(
                &Vec3 {
                    x: 1.0,
                    y: -1.0,
                    z: 2.0
                },
                &Vec3::y_axis()
            ),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 2.0
            }
        );
        assert_eq!(
            Vec3::project(
                &Vec3 {
                    x: 2.0,
                    y: 3.0,
                    z: 4.0
                },
                &Vec3::z_axis()
            ),
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 4.0
            }
        );
        assert_eq!(Vec3::project(&Vec3::one(), &Vec3::zero()), Vec3::zero());
        assert_eq!(
            Vec3::min(
                &Vec3 {
                    x: 1.0,
                    y: 4.0,
                    z: 2.0
                },
                &Vec3 {
                    x: 2.0,
                    y: 3.0,
                    z: 5.0
                }
            ),
            Vec3 {
                x: 1.0,
                y: 3.0,
                z: 2.0
            }
        );
        assert_eq!(
            Vec3::max(
                &Vec3 {
                    x: 1.0,
                    y: 4.0,
                    z: 2.0
                },
                &Vec3 {
                    x: 2.0,
                    y: 3.0,
                    z: 5.0
                }
            ),
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 5.0
            }
        );
        assert_eq!(
            Vec3::clamp(
                &Vec3 {
                    x: -1.0,
                    y: 0.5,
                    z: 3.0
                },
                &Vec3::zero(),
                &Vec3::one()
            ),
            Vec3 {
                x: 0.0,
                y: 0.5,
                z: 1.0
            }
        );

        let mut reflected = Vec3 {
            x: 1.0,
            y: -1.0,
            z: 2.0,
        };
        reflected.reflected(&Vec3::y_axis());
        assert_eq!(
            reflected,
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 2.0
            }
        );

        let mut clamped = Vec3 {
            x: -1.0,
            y: 0.5,
            z: 3.0,
        };
        clamped.clamped(&Vec3::zero(), &Vec3::one());
        assert_eq!(
            clamped,
            Vec3 {
                x: 0.0,
                y: 0.5,
                z: 1.0
            }
        );
    }

    #[test]
    fn vec3_operator_impls_work_component_wise() {
        let mut vector = Vec3 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
        };

        assert_eq!(
            vector
                + Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 4.0
                },
            Vec3 {
                x: 3.0,
                y: 6.0,
                z: 12.0
            }
        );
        vector += Vec3 {
            x: 1.0,
            y: 2.0,
            z: 4.0,
        };
        assert_eq!(
            vector,
            Vec3 {
                x: 3.0,
                y: 6.0,
                z: 12.0
            }
        );
        assert_eq!(
            vector
                - Vec3 {
                    x: 1.0,
                    y: 2.0,
                    z: 4.0
                },
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 8.0
            }
        );
        vector -= Vec3 {
            x: 1.0,
            y: 2.0,
            z: 4.0,
        };
        assert_eq!(
            vector,
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 8.0
            }
        );
        assert_eq!(
            vector * 2.0,
            Vec3 {
                x: 4.0,
                y: 8.0,
                z: 16.0
            }
        );
        vector *= 2.0;
        assert_eq!(
            vector,
            Vec3 {
                x: 4.0,
                y: 8.0,
                z: 16.0
            }
        );
        assert_eq!(
            vector
                * Vec3 {
                    x: 2.0,
                    y: 4.0,
                    z: 8.0
                },
            Vec3 {
                x: 8.0,
                y: 32.0,
                z: 128.0
            }
        );
        vector *= Vec3 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
        };
        assert_eq!(
            vector,
            Vec3 {
                x: 8.0,
                y: 32.0,
                z: 128.0
            }
        );
        assert_eq!(
            vector / 2.0,
            Vec3 {
                x: 4.0,
                y: 16.0,
                z: 64.0
            }
        );
        vector /= 2.0;
        assert_eq!(
            vector,
            Vec3 {
                x: 4.0,
                y: 16.0,
                z: 64.0
            }
        );
        assert_eq!(
            vector
                / Vec3 {
                    x: 2.0,
                    y: 4.0,
                    z: 8.0
                },
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 8.0
            }
        );
        vector /= Vec3 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
        };
        assert_eq!(
            vector,
            Vec3 {
                x: 2.0,
                y: 4.0,
                z: 8.0
            }
        );
        assert_eq!(
            -vector,
            Vec3 {
                x: -2.0,
                y: -4.0,
                z: -8.0
            }
        );
    }

    #[test]
    fn vec3_reference_operator_impls_accept_borrowed_operands() {
        let lhs = Vec3 {
            x: 8.0,
            y: 16.0,
            z: 32.0,
        };
        let rhs = Vec3 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
        };

        assert_eq!(
            lhs + &rhs,
            Vec3 {
                x: 10.0,
                y: 20.0,
                z: 40.0
            }
        );
        assert_eq!(
            &lhs + rhs,
            Vec3 {
                x: 10.0,
                y: 20.0,
                z: 40.0
            }
        );
        assert_eq!(
            &lhs + &rhs,
            Vec3 {
                x: 10.0,
                y: 20.0,
                z: 40.0
            }
        );

        let mut assigned = lhs;
        assigned += &rhs;
        assert_eq!(
            assigned,
            Vec3 {
                x: 10.0,
                y: 20.0,
                z: 40.0
            }
        );

        assert_eq!(
            lhs - &rhs,
            Vec3 {
                x: 6.0,
                y: 12.0,
                z: 24.0
            }
        );
        assert_eq!(
            &lhs - rhs,
            Vec3 {
                x: 6.0,
                y: 12.0,
                z: 24.0
            }
        );
        assert_eq!(
            &lhs - &rhs,
            Vec3 {
                x: 6.0,
                y: 12.0,
                z: 24.0
            }
        );

        assigned = lhs;
        assigned -= &rhs;
        assert_eq!(
            assigned,
            Vec3 {
                x: 6.0,
                y: 12.0,
                z: 24.0
            }
        );

        assert_eq!(
            &lhs * 2.0,
            Vec3 {
                x: 16.0,
                y: 32.0,
                z: 64.0
            }
        );
        assert_eq!(
            lhs * &rhs,
            Vec3 {
                x: 16.0,
                y: 64.0,
                z: 256.0
            }
        );
        assert_eq!(
            &lhs * rhs,
            Vec3 {
                x: 16.0,
                y: 64.0,
                z: 256.0
            }
        );
        assert_eq!(
            &lhs * &rhs,
            Vec3 {
                x: 16.0,
                y: 64.0,
                z: 256.0
            }
        );

        assigned = lhs;
        assigned *= &rhs;
        assert_eq!(
            assigned,
            Vec3 {
                x: 16.0,
                y: 64.0,
                z: 256.0
            }
        );

        assert_eq!(
            &lhs / 2.0,
            Vec3 {
                x: 4.0,
                y: 8.0,
                z: 16.0
            }
        );
        assert_eq!(
            lhs / &rhs,
            Vec3 {
                x: 4.0,
                y: 4.0,
                z: 4.0
            }
        );
        assert_eq!(
            &lhs / rhs,
            Vec3 {
                x: 4.0,
                y: 4.0,
                z: 4.0
            }
        );
        assert_eq!(
            &lhs / &rhs,
            Vec3 {
                x: 4.0,
                y: 4.0,
                z: 4.0
            }
        );

        assigned = lhs;
        assigned /= &rhs;
        assert_eq!(
            assigned,
            Vec3 {
                x: 4.0,
                y: 4.0,
                z: 4.0
            }
        );

        assert_eq!(
            -&lhs,
            Vec3 {
                x: -8.0,
                y: -16.0,
                z: -32.0
            }
        );
    }
}
