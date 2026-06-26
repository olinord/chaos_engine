use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::math::vector::vec2::Vec2;
use crate::math::vector::vec3::Vec3;

use vulkano_macros::BufferContents;

#[derive(Debug, Clone, Copy, PartialEq, BufferContents)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    /// Returns the zero vector.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::zero(), Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 0.0 });
    /// ```
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// Returns a vector with every component set to `1.0`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::one(), Vec4 { x: 1.0, y: 1.0, z: 1.0, w: 1.0 });
    /// ```
    pub const fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
            w: 1.0,
        }
    }

    /// Returns the unit vector along the positive x axis.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::x_axis(), Vec4 { x: 1.0, y: 0.0, z: 0.0, w: 0.0 });
    /// ```
    pub const fn x_axis() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// Returns the unit vector along the positive y axis.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::y_axis(), Vec4 { x: 0.0, y: 1.0, z: 0.0, w: 0.0 });
    /// ```
    pub const fn y_axis() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// Returns the unit vector along the positive z axis.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::z_axis(), Vec4 { x: 0.0, y: 0.0, z: 1.0, w: 0.0 });
    /// ```
    pub const fn z_axis() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
            w: 0.0,
        }
    }

    /// Returns the unit vector along the positive w axis.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::w_axis(), Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 });
    /// ```
    pub const fn w_axis() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    /// Returns a vector with every component set to `value`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4::splat(2.5), Vec4 { x: 2.5, y: 2.5, z: 2.5, w: 2.5 });
    /// ```
    pub const fn splat(value: f32) -> Self {
        Self {
            x: value,
            y: value,
            z: value,
            w: value,
        }
    }

    // non member functions
    /// Returns a normalized copy of `v`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let normalized = Vec4::normalized(&Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 });
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
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let distance = Vec4::distance(&Vec4::zero(), &Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 });
    /// assert_eq!(distance, 5.0);
    /// ```
    pub fn distance(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        let dz = one.z - other.z;
        let dw = one.w - other.w;
        (dx * dx + dy * dy + dz * dz + dw * dw).sqrt()
    }

    /// Returns the squared Euclidean distance between two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let distance_squared = Vec4::distance_squared(&Vec4::zero(), &Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 });
    /// assert_eq!(distance_squared, 25.0);
    /// ```
    pub fn distance_squared(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        let dz = one.z - other.z;
        let dw = one.w - other.w;
        dx * dx + dy * dy + dz * dz + dw * dw
    }

    /// Returns the dot product of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let dot = Vec4::dot(&Vec4 { x: 1.0, y: 2.0, z: 3.0, w: 4.0 }, &Vec4 { x: 5.0, y: 6.0, z: 7.0, w: 8.0 });
    /// assert_eq!(dot, 70.0);
    /// ```
    pub fn dot(one: &Self, other: &Self) -> f32 {
        one.x * other.x + one.y * other.y + one.z * other.z + one.w * other.w
    }

    /// Returns the 4D cross product perpendicular to three input vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let cross = Vec4::cross(&Vec4::x_axis(), &Vec4::y_axis(), &Vec4::z_axis());
    /// assert_eq!(cross, Vec4 { x: 0.0, y: -0.0, z: 0.0, w: -1.0 });
    /// ```
    pub fn cross(one: &Self, other: &Self, another: &Self) -> Self {
        let x = one.y * (other.z * another.w - other.w * another.z)
            - one.z * (other.y * another.w - other.w * another.y)
            + one.w * (other.y * another.z - other.z * another.y);

        let y = -(one.x * (other.z * another.w - other.w * another.z)
            - one.z * (other.x * another.w - other.w * another.x)
            + one.w * (other.x * another.z - other.z * another.x));

        let z = one.x * (other.y * another.w - other.w * another.y)
            - one.y * (other.x * another.w - other.w * another.x)
            + one.w * (other.x * another.y - other.y * another.x);

        let w = -(one.x * (other.y * another.z - other.z * another.y)
            - one.y * (other.x * another.z - other.z * another.x)
            + one.z * (other.x * another.y - other.y * another.x));

        Self { x, y, z, w }
    }

    /// Linearly interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::lerp(&Vec4::zero(), &Vec4 { x: 10.0, y: 20.0, z: 30.0, w: 40.0 }, 0.25);
    /// assert_eq!(result, Vec4 { x: 2.5, y: 5.0, z: 7.5, w: 10.0 });
    /// ```
    pub fn lerp(one: &Self, other: &Self, t: f32) -> Self {
        Self {
            x: one.x + (other.x - one.x) * t,
            y: one.y + (other.y - one.y) * t,
            z: one.z + (other.z - one.z) * t,
            w: one.w + (other.w - one.w) * t,
        }
    }

    /// Spherically interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::slerp(&Vec4::x_axis(), &Vec4::y_axis(), 0.5);
    /// assert!((result.x - 0.70710677).abs() < 0.00001);
    /// assert!((result.y - 0.70710677).abs() < 0.00001);
    /// assert_eq!(result.z, 0.0);
    /// assert_eq!(result.w, 0.0);
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
            w: scale_one * one.w + scale_other * other.w,
        }
    }

    /// Linearly interpolates from `one` to `other` by `t`, then normalizes the result.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::nlerp(&Vec4::x_axis(), &Vec4::y_axis(), 0.5);
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
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let angle = Vec4::angle(&Vec4::x_axis(), &Vec4::y_axis());
    /// assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
    /// ```
    pub fn angle(one: &Self, other: &Self) -> f32 {
        let dot = Self::dot(one, other);
        dot.acos()
    }

    /// Reflects an incident vector around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let reflected = Vec4::reflect(&Vec4 { x: 1.0, y: -1.0, z: 2.0, w: 3.0 }, &Vec4::y_axis());
    /// assert_eq!(reflected, Vec4 { x: 1.0, y: 1.0, z: 2.0, w: 3.0 });
    /// ```
    pub fn reflect(incident: &Self, normal: &Self) -> Self {
        let dot = Self::dot(incident, normal);
        *incident - *normal * (2.0 * dot)
    }

    /// Projects vector `a` onto vector `b`.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let projection = Vec4::project(&Vec4 { x: 2.0, y: 3.0, z: 4.0, w: 5.0 }, &Vec4::w_axis());
    /// assert_eq!(projection, Vec4 { x: 0.0, y: 0.0, z: 0.0, w: 5.0 });
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
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::min(&Vec4 { x: 1.0, y: 4.0, z: 2.0, w: 8.0 }, &Vec4 { x: 2.0, y: 3.0, z: 5.0, w: 7.0 });
    /// assert_eq!(result, Vec4 { x: 1.0, y: 3.0, z: 2.0, w: 7.0 });
    /// ```
    pub fn min(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.min(other.x),
            y: one.y.min(other.y),
            z: one.z.min(other.z),
            w: one.w.min(other.w),
        }
    }

    /// Returns the component-wise maximum of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::max(&Vec4 { x: 1.0, y: 4.0, z: 2.0, w: 8.0 }, &Vec4 { x: 2.0, y: 3.0, z: 5.0, w: 7.0 });
    /// assert_eq!(result, Vec4 { x: 2.0, y: 4.0, z: 5.0, w: 8.0 });
    /// ```
    pub fn max(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.max(other.x),
            y: one.y.max(other.y),
            z: one.z.max(other.z),
            w: one.w.max(other.w),
        }
    }

    /// Clamps each component of `value` between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let result = Vec4::clamp(&Vec4 { x: -1.0, y: 0.5, z: 3.0, w: 0.25 }, &Vec4::zero(), &Vec4::one());
    /// assert_eq!(result, Vec4 { x: 0.0, y: 0.5, z: 1.0, w: 0.25 });
    /// ```
    pub fn clamp(value: &Self, min: &Self, max: &Self) -> Self {
        Self {
            x: value.x.clamp(min.x, max.x),
            y: value.y.clamp(min.y, max.y),
            z: value.z.clamp(min.z, max.z),
            w: value.w.clamp(min.w, max.w),
        }
    }

    // member functions
    /// Normalizes this vector in place if its length is non-zero.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let mut vector = Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 };
    /// vector.normalize();
    /// assert!((vector.length() - 1.0).abs() < 0.00001);
    /// ```
    pub fn normalize(&mut self) {
        let length = self.length();
        if length != 0.0 {
            self.x /= length;
            self.y /= length;
            self.z /= length;
            self.w /= length;
        }
    }

    /// Reflects this vector in place around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let mut vector = Vec4 { x: 1.0, y: -1.0, z: 2.0, w: 3.0 };
    /// vector.reflected(&Vec4::y_axis());
    /// assert_eq!(vector, Vec4 { x: 1.0, y: 1.0, z: 2.0, w: 3.0 });
    /// ```
    pub fn reflected(&mut self, normal: &Self) {
        let dot = Self::dot(self, normal);
        let translation = *normal * (2.0 * dot);
        *self -= translation;
    }

    /// Clamps this vector in place between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// let mut vector = Vec4 { x: -1.0, y: 0.5, z: 3.0, w: 0.25 };
    /// vector.clamped(&Vec4::zero(), &Vec4::one());
    /// assert_eq!(vector, Vec4 { x: 0.0, y: 0.5, z: 1.0, w: 0.25 });
    /// ```
    pub fn clamped(&mut self, min: &Self, max: &Self) {
        self.x = self.x.clamp(min.x, max.x);
        self.y = self.y.clamp(min.y, max.y);
        self.z = self.z.clamp(min.z, max.z);
        self.w = self.w.clamp(min.w, max.w);
    }

    /// Returns this vector's Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 }.length(), 5.0);
    /// ```
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    /// Returns this vector's squared Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 }.length_squared(), 25.0);
    /// ```
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Returns this vector's squared Euclidean length.
    ///
    /// This misspelled alias is kept for compatibility with older callers.
    ///
    /// ```rust
    /// use chaos_engine::math::vector::Vec4;
    ///
    /// assert_eq!(Vec4 { x: 0.0, y: 0.0, z: 3.0, w: 4.0 }.length_sqared(), 25.0);
    /// ```
    pub fn length_sqared(&self) -> f32 {
        self.length_squared()
    }

    // swizzling methods
    impl_swizzle2_all!((x, y, z, w), Vec2; x, y, z, w);
    impl_swizzle3_all!((x, y, z, w), Vec3; x, y, z, w);
    impl_swizzle4_all!((x, y, z, w), Vec4; x, y, z, w);
}

impl Add for Vec4 {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }
}

impl AddAssign for Vec4 {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
        self.w += other.w;
    }
}

impl Sub for Vec4 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            w: self.w - rhs.w,
        }
    }
}

impl SubAssign for Vec4 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
        self.w -= rhs.w;
    }
}

impl Mul<f32> for Vec4 {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
            w: self.w * scalar,
        }
    }
}

impl MulAssign<f32> for Vec4 {
    fn mul_assign(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
        self.z *= scalar;
        self.w *= scalar;
    }
}

impl Mul<Vec4> for Vec4 {
    type Output = Self;

    fn mul(self, other: Vec4) -> Self::Output {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
            w: self.w * other.w,
        }
    }
}

impl MulAssign<Vec4> for Vec4 {
    fn mul_assign(&mut self, other: Vec4) {
        self.x *= other.x;
        self.y *= other.y;
        self.z *= other.z;
        self.w *= other.w;
    }
}

impl Div<f32> for Vec4 {
    type Output = Self;

    fn div(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x / scalar,
            y: self.y / scalar,
            z: self.z / scalar,
            w: self.w / scalar,
        }
    }
}

impl DivAssign<f32> for Vec4 {
    fn div_assign(&mut self, scalar: f32) {
        self.x /= scalar;
        self.y /= scalar;
        self.z /= scalar;
        self.w /= scalar;
    }
}

impl Div<Vec4> for Vec4 {
    type Output = Self;

    fn div(self, other: Vec4) -> Self::Output {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
            w: self.w / other.w,
        }
    }
}

impl DivAssign<Vec4> for Vec4 {
    fn div_assign(&mut self, other: Vec4) {
        self.x /= other.x;
        self.y /= other.y;
        self.z /= other.z;
        self.w /= other.w;
    }
}

impl Neg for Vec4 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: -self.w,
        }
    }
}

impl_vector_reference_ops!(Vec4);

impl From<[f32; 4]> for Vec4 {
    fn from(arr: [f32; 4]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
            w: arr[3],
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

    fn assert_vec4_eq(actual: Vec4, expected: Vec4) {
        assert_f32_eq(actual.x, expected.x);
        assert_f32_eq(actual.y, expected.y);
        assert_f32_eq(actual.z, expected.z);
        assert_f32_eq(actual.w, expected.w);
    }

    #[test]
    fn vec4_constructors_return_expected_values() {
        assert_eq!(
            Vec4::zero(),
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0
            }
        );
        assert_eq!(
            Vec4::one(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0
            }
        );
        assert_eq!(
            Vec4::x_axis(),
            Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 0.0
            }
        );
        assert_eq!(
            Vec4::y_axis(),
            Vec4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 0.0
            }
        );
        assert_eq!(
            Vec4::z_axis(),
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 0.0
            }
        );
        assert_eq!(
            Vec4::w_axis(),
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0
            }
        );
        assert_eq!(
            Vec4::splat(2.5),
            Vec4 {
                x: 2.5,
                y: 2.5,
                z: 2.5,
                w: 2.5
            }
        );
        assert_eq!(
            Vec4::from([1.0, 2.0, 3.0, 4.0]),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 4.0
            }
        );
    }

    #[test]
    fn vec4_swizzling_methods_return_expected_vectors() {
        let vector = Vec4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };

        assert_eq!(vector.xx(), Vec2 { x: 1.0, y: 1.0 });
        assert_eq!(vector.xy(), Vec2 { x: 1.0, y: 2.0 });
        assert_eq!(vector.xz(), Vec2 { x: 1.0, y: 3.0 });
        assert_eq!(vector.xw(), Vec2 { x: 1.0, y: 4.0 });
        assert_eq!(vector.yx(), Vec2 { x: 2.0, y: 1.0 });
        assert_eq!(vector.yy(), Vec2 { x: 2.0, y: 2.0 });
        assert_eq!(vector.yz(), Vec2 { x: 2.0, y: 3.0 });
        assert_eq!(vector.yw(), Vec2 { x: 2.0, y: 4.0 });
        assert_eq!(vector.zx(), Vec2 { x: 3.0, y: 1.0 });
        assert_eq!(vector.zy(), Vec2 { x: 3.0, y: 2.0 });
        assert_eq!(vector.zz(), Vec2 { x: 3.0, y: 3.0 });
        assert_eq!(vector.zw(), Vec2 { x: 3.0, y: 4.0 });
        assert_eq!(vector.wx(), Vec2 { x: 4.0, y: 1.0 });
        assert_eq!(vector.wy(), Vec2 { x: 4.0, y: 2.0 });
        assert_eq!(vector.wz(), Vec2 { x: 4.0, y: 3.0 });
        assert_eq!(vector.ww(), Vec2 { x: 4.0, y: 4.0 });

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
            vector.xwy(),
            Vec3 {
                x: 1.0,
                y: 4.0,
                z: 2.0,
            }
        );
        assert_eq!(
            vector.yzw(),
            Vec3 {
                x: 2.0,
                y: 3.0,
                z: 4.0,
            }
        );
        assert_eq!(
            vector.zwx(),
            Vec3 {
                x: 3.0,
                y: 4.0,
                z: 1.0,
            }
        );
        assert_eq!(
            vector.wzy(),
            Vec3 {
                x: 4.0,
                y: 3.0,
                z: 2.0,
            }
        );
        assert_eq!(
            vector.www(),
            Vec3 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
            }
        );

        assert_eq!(
            vector.xxxx(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.xyzw(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                w: 4.0,
            }
        );
        assert_eq!(
            vector.xwzy(),
            Vec4 {
                x: 1.0,
                y: 4.0,
                z: 3.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.yxwz(),
            Vec4 {
                x: 2.0,
                y: 1.0,
                z: 4.0,
                w: 3.0,
            }
        );
        assert_eq!(
            vector.zwyx(),
            Vec4 {
                x: 3.0,
                y: 4.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.wzyx(),
            Vec4 {
                x: 4.0,
                y: 3.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.wwww(),
            Vec4 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
                w: 4.0,
            }
        );
    }

    #[test]
    fn vec4_measurement_and_normalization_methods_work() {
        let vector = Vec4 {
            x: 0.0,
            y: 0.0,
            z: 3.0,
            w: 4.0,
        };

        assert_eq!(vector.length(), 5.0);
        assert_eq!(vector.length_squared(), 25.0);
        assert_eq!(vector.length_sqared(), 25.0);
        assert_eq!(Vec4::distance(&Vec4::zero(), &vector), 5.0);
        assert_eq!(Vec4::distance_squared(&Vec4::zero(), &vector), 25.0);
        assert_eq!(
            Vec4::dot(
                &Vec4 {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    w: 4.0
                },
                &Vec4 {
                    x: 5.0,
                    y: 6.0,
                    z: 7.0,
                    w: 8.0
                },
            ),
            70.0
        );
        assert_eq!(
            Vec4::cross(&Vec4::x_axis(), &Vec4::y_axis(), &Vec4::z_axis()),
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: -1.0
            }
        );
        assert_f32_eq(
            Vec4::angle(&Vec4::x_axis(), &Vec4::y_axis()),
            std::f32::consts::FRAC_PI_2,
        );

        let normalized = Vec4::normalized(&vector);
        assert_f32_eq(normalized.length(), 1.0);

        let mut mutable = vector;
        mutable.normalize();
        assert_vec4_eq(mutable, normalized);

        let mut zero = Vec4::zero();
        zero.normalize();
        assert_eq!(zero, Vec4::zero());
    }

    #[test]
    fn vec4_interpolation_projection_reflection_and_clamping_methods_work() {
        assert_vec4_eq(
            Vec4::lerp(
                &Vec4::zero(),
                &Vec4 {
                    x: 10.0,
                    y: 20.0,
                    z: 30.0,
                    w: 40.0,
                },
                0.25,
            ),
            Vec4 {
                x: 2.5,
                y: 5.0,
                z: 7.5,
                w: 10.0,
            },
        );

        let slerped = Vec4::slerp(&Vec4::x_axis(), &Vec4::y_axis(), 0.5);
        assert_vec4_eq(
            slerped,
            Vec4 {
                x: std::f32::consts::FRAC_1_SQRT_2,
                y: std::f32::consts::FRAC_1_SQRT_2,
                z: 0.0,
                w: 0.0,
            },
        );

        let nlerped = Vec4::nlerp(&Vec4::x_axis(), &Vec4::y_axis(), 0.5);
        assert_f32_eq(nlerped.length(), 1.0);

        assert_eq!(
            Vec4::reflect(
                &Vec4 {
                    x: 1.0,
                    y: -1.0,
                    z: 2.0,
                    w: 3.0
                },
                &Vec4::y_axis()
            ),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                w: 3.0
            }
        );
        assert_eq!(
            Vec4::project(
                &Vec4 {
                    x: 2.0,
                    y: 3.0,
                    z: 4.0,
                    w: 5.0
                },
                &Vec4::w_axis()
            ),
            Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 5.0
            }
        );
        assert_eq!(Vec4::project(&Vec4::one(), &Vec4::zero()), Vec4::zero());
        assert_eq!(
            Vec4::min(
                &Vec4 {
                    x: 1.0,
                    y: 4.0,
                    z: 2.0,
                    w: 8.0
                },
                &Vec4 {
                    x: 2.0,
                    y: 3.0,
                    z: 5.0,
                    w: 7.0
                },
            ),
            Vec4 {
                x: 1.0,
                y: 3.0,
                z: 2.0,
                w: 7.0
            }
        );
        assert_eq!(
            Vec4::max(
                &Vec4 {
                    x: 1.0,
                    y: 4.0,
                    z: 2.0,
                    w: 8.0
                },
                &Vec4 {
                    x: 2.0,
                    y: 3.0,
                    z: 5.0,
                    w: 7.0
                },
            ),
            Vec4 {
                x: 2.0,
                y: 4.0,
                z: 5.0,
                w: 8.0
            }
        );
        assert_eq!(
            Vec4::clamp(
                &Vec4 {
                    x: -1.0,
                    y: 0.5,
                    z: 3.0,
                    w: 0.25
                },
                &Vec4::zero(),
                &Vec4::one(),
            ),
            Vec4 {
                x: 0.0,
                y: 0.5,
                z: 1.0,
                w: 0.25
            }
        );

        let mut reflected = Vec4 {
            x: 1.0,
            y: -1.0,
            z: 2.0,
            w: 3.0,
        };
        reflected.reflected(&Vec4::y_axis());
        assert_eq!(
            reflected,
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                w: 3.0
            }
        );

        let mut clamped = Vec4 {
            x: -1.0,
            y: 0.5,
            z: 3.0,
            w: 0.25,
        };
        clamped.clamped(&Vec4::zero(), &Vec4::one());
        assert_eq!(
            clamped,
            Vec4 {
                x: 0.0,
                y: 0.5,
                z: 1.0,
                w: 0.25
            }
        );
    }

    #[test]
    fn vec4_operator_impls_work_component_wise() {
        let mut vector = Vec4 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
            w: 16.0,
        };

        assert_eq!(
            vector
                + Vec4 {
                    x: 1.0,
                    y: 2.0,
                    z: 4.0,
                    w: 8.0
                },
            Vec4 {
                x: 3.0,
                y: 6.0,
                z: 12.0,
                w: 24.0
            }
        );
        vector += Vec4 {
            x: 1.0,
            y: 2.0,
            z: 4.0,
            w: 8.0,
        };
        assert_eq!(
            vector,
            Vec4 {
                x: 3.0,
                y: 6.0,
                z: 12.0,
                w: 24.0
            }
        );
        assert_eq!(
            vector
                - Vec4 {
                    x: 1.0,
                    y: 2.0,
                    z: 4.0,
                    w: 8.0
                },
            Vec4 {
                x: 2.0,
                y: 4.0,
                z: 8.0,
                w: 16.0
            }
        );
        vector -= Vec4 {
            x: 1.0,
            y: 2.0,
            z: 4.0,
            w: 8.0,
        };
        assert_eq!(
            vector,
            Vec4 {
                x: 2.0,
                y: 4.0,
                z: 8.0,
                w: 16.0
            }
        );
        assert_eq!(
            vector * 2.0,
            Vec4 {
                x: 4.0,
                y: 8.0,
                z: 16.0,
                w: 32.0
            }
        );
        vector *= 2.0;
        assert_eq!(
            vector,
            Vec4 {
                x: 4.0,
                y: 8.0,
                z: 16.0,
                w: 32.0
            }
        );
        assert_eq!(
            vector
                * Vec4 {
                    x: 2.0,
                    y: 4.0,
                    z: 8.0,
                    w: 16.0
                },
            Vec4 {
                x: 8.0,
                y: 32.0,
                z: 128.0,
                w: 512.0
            }
        );
        vector *= Vec4 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
            w: 16.0,
        };
        assert_eq!(
            vector,
            Vec4 {
                x: 8.0,
                y: 32.0,
                z: 128.0,
                w: 512.0
            }
        );
        assert_eq!(
            vector / 2.0,
            Vec4 {
                x: 4.0,
                y: 16.0,
                z: 64.0,
                w: 256.0
            }
        );
        vector /= 2.0;
        assert_eq!(
            vector,
            Vec4 {
                x: 4.0,
                y: 16.0,
                z: 64.0,
                w: 256.0
            }
        );
        assert_eq!(
            vector
                / Vec4 {
                    x: 2.0,
                    y: 4.0,
                    z: 8.0,
                    w: 16.0
                },
            Vec4 {
                x: 2.0,
                y: 4.0,
                z: 8.0,
                w: 16.0
            }
        );
        vector /= Vec4 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
            w: 16.0,
        };
        assert_eq!(
            vector,
            Vec4 {
                x: 2.0,
                y: 4.0,
                z: 8.0,
                w: 16.0
            }
        );
        assert_eq!(
            -vector,
            Vec4 {
                x: -2.0,
                y: -4.0,
                z: -8.0,
                w: -16.0
            }
        );
    }

    #[test]
    fn vec4_reference_operator_impls_accept_borrowed_operands() {
        let lhs = Vec4 {
            x: 8.0,
            y: 16.0,
            z: 32.0,
            w: 64.0,
        };
        let rhs = Vec4 {
            x: 2.0,
            y: 4.0,
            z: 8.0,
            w: 16.0,
        };

        assert_eq!(
            lhs + &rhs,
            Vec4 {
                x: 10.0,
                y: 20.0,
                z: 40.0,
                w: 80.0
            }
        );
        assert_eq!(
            &lhs + rhs,
            Vec4 {
                x: 10.0,
                y: 20.0,
                z: 40.0,
                w: 80.0
            }
        );
        assert_eq!(
            &lhs + &rhs,
            Vec4 {
                x: 10.0,
                y: 20.0,
                z: 40.0,
                w: 80.0
            }
        );

        let mut assigned = lhs;
        assigned += &rhs;
        assert_eq!(
            assigned,
            Vec4 {
                x: 10.0,
                y: 20.0,
                z: 40.0,
                w: 80.0
            }
        );

        assert_eq!(
            lhs - &rhs,
            Vec4 {
                x: 6.0,
                y: 12.0,
                z: 24.0,
                w: 48.0
            }
        );
        assert_eq!(
            &lhs - rhs,
            Vec4 {
                x: 6.0,
                y: 12.0,
                z: 24.0,
                w: 48.0
            }
        );
        assert_eq!(
            &lhs - &rhs,
            Vec4 {
                x: 6.0,
                y: 12.0,
                z: 24.0,
                w: 48.0
            }
        );

        assigned = lhs;
        assigned -= &rhs;
        assert_eq!(
            assigned,
            Vec4 {
                x: 6.0,
                y: 12.0,
                z: 24.0,
                w: 48.0
            }
        );

        assert_eq!(
            &lhs * 2.0,
            Vec4 {
                x: 16.0,
                y: 32.0,
                z: 64.0,
                w: 128.0
            }
        );
        assert_eq!(
            lhs * &rhs,
            Vec4 {
                x: 16.0,
                y: 64.0,
                z: 256.0,
                w: 1024.0
            }
        );
        assert_eq!(
            &lhs * rhs,
            Vec4 {
                x: 16.0,
                y: 64.0,
                z: 256.0,
                w: 1024.0
            }
        );
        assert_eq!(
            &lhs * &rhs,
            Vec4 {
                x: 16.0,
                y: 64.0,
                z: 256.0,
                w: 1024.0
            }
        );

        assigned = lhs;
        assigned *= &rhs;
        assert_eq!(
            assigned,
            Vec4 {
                x: 16.0,
                y: 64.0,
                z: 256.0,
                w: 1024.0
            }
        );

        assert_eq!(
            &lhs / 2.0,
            Vec4 {
                x: 4.0,
                y: 8.0,
                z: 16.0,
                w: 32.0
            }
        );
        assert_eq!(
            lhs / &rhs,
            Vec4 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
                w: 4.0
            }
        );
        assert_eq!(
            &lhs / rhs,
            Vec4 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
                w: 4.0
            }
        );
        assert_eq!(
            &lhs / &rhs,
            Vec4 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
                w: 4.0
            }
        );

        assigned = lhs;
        assigned /= &rhs;
        assert_eq!(
            assigned,
            Vec4 {
                x: 4.0,
                y: 4.0,
                z: 4.0,
                w: 4.0
            }
        );

        assert_eq!(
            -&lhs,
            Vec4 {
                x: -8.0,
                y: -16.0,
                z: -32.0,
                w: -64.0
            }
        );
    }
}
