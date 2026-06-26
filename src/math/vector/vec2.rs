use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::math::vector::vec3::Vec3;
use crate::math::vector::vec4::Vec4;
use vulkano_macros::BufferContents;

#[derive(Debug, Clone, Copy, PartialEq, BufferContents)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// Returns the zero vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2::zero(), Vec2 { x: 0.0, y: 0.0 });
    /// ```
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Returns a vector with every component set to `1.0`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2::one(), Vec2 { x: 1.0, y: 1.0 });
    /// ```
    pub const fn one() -> Self {
        Self { x: 1.0, y: 1.0 }
    }

    /// Returns the unit vector along the positive x axis.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2::x_axis(), Vec2 { x: 1.0, y: 0.0 });
    /// ```
    pub const fn x_axis() -> Self {
        Self { x: 1.0, y: 0.0 }
    }

    /// Returns the unit vector along the positive y axis.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2::y_axis(), Vec2 { x: 0.0, y: 1.0 });
    /// ```
    pub const fn y_axis() -> Self {
        Self { x: 0.0, y: 1.0 }
    }

    /// Returns a vector with every component set to `value`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2::splat(2.5), Vec2 { x: 2.5, y: 2.5 });
    /// ```
    pub const fn splat(value: f32) -> Self {
        Self { x: value, y: value }
    }

    // non member functions
    /// Returns a normalized copy of `v`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let normalized = Vec2::normalized(&Vec2 { x: 3.0, y: 4.0 });
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
    /// use chaos_engine::math::Vec2;
    ///
    /// let distance = Vec2::distance(&Vec2::zero(), &Vec2 { x: 3.0, y: 4.0 });
    /// assert_eq!(distance, 5.0);
    /// ```
    pub fn distance(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the squared Euclidean distance between two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let distance_squared = Vec2::distance_squared(&Vec2::zero(), &Vec2 { x: 3.0, y: 4.0 });
    /// assert_eq!(distance_squared, 25.0);
    /// ```
    pub fn distance_squared(one: &Self, other: &Self) -> f32 {
        let dx = one.x - other.x;
        let dy = one.y - other.y;
        dx * dx + dy * dy
    }

    /// Returns the dot product of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let dot = Vec2::dot(&Vec2 { x: 1.0, y: 2.0 }, &Vec2 { x: 3.0, y: 4.0 });
    /// assert_eq!(dot, 11.0);
    /// ```
    pub fn dot(one: &Self, other: &Self) -> f32 {
        one.x * other.x + one.y * other.y
    }

    /// Returns the scalar 2D cross product of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let cross = Vec2::cross(&Vec2::x_axis(), &Vec2::y_axis());
    /// assert_eq!(cross, 1.0);
    /// ```
    pub fn cross(one: &Self, other: &Self) -> f32 {
        one.x * other.y - one.y * other.x
    }

    /// Linearly interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::lerp(&Vec2::zero(), &Vec2 { x: 10.0, y: 20.0 }, 0.25);
    /// assert_eq!(result, Vec2 { x: 2.5, y: 5.0 });
    /// ```
    pub fn lerp(one: &Self, other: &Self, t: f32) -> Self {
        Self {
            x: one.x + (other.x - one.x) * t,
            y: one.y + (other.y - one.y) * t,
        }
    }

    /// Spherically interpolates from `one` to `other` by `t`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::slerp(&Vec2::x_axis(), &Vec2::y_axis(), 0.5);
    /// assert!((result.x - 0.70710677).abs() < 0.00001);
    /// assert!((result.y - 0.70710677).abs() < 0.00001);
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
        }
    }

    /// Linearly interpolates from `one` to `other` by `t`, then normalizes the result.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::nlerp(&Vec2::x_axis(), &Vec2::y_axis(), 0.5);
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
    /// use chaos_engine::math::Vec2;
    ///
    /// let angle = Vec2::angle(&Vec2::x_axis(), &Vec2::y_axis());
    /// assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 0.00001);
    /// ```
    pub fn angle(one: &Self, other: &Self) -> f32 {
        let dot = Self::dot(one, other);
        dot.acos()
    }

    /// Reflects an incident vector around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let reflected = Vec2::reflect(&Vec2 { x: 1.0, y: -1.0 }, &Vec2::y_axis());
    /// assert_eq!(reflected, Vec2 { x: 1.0, y: 1.0 });
    /// ```
    pub fn reflect(incident: &Self, normal: &Self) -> Self {
        let dot = Self::dot(incident, normal);
        *incident - *normal * (2.0 * dot)
    }

    /// Projects vector `a` onto vector `b`.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let projection = Vec2::project(&Vec2 { x: 2.0, y: 3.0 }, &Vec2::x_axis());
    /// assert_eq!(projection, Vec2 { x: 2.0, y: 0.0 });
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
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::min(&Vec2 { x: 1.0, y: 4.0 }, &Vec2 { x: 2.0, y: 3.0 });
    /// assert_eq!(result, Vec2 { x: 1.0, y: 3.0 });
    /// ```
    pub fn min(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.min(other.x),
            y: one.y.min(other.y),
        }
    }

    /// Returns the component-wise maximum of two vectors.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::max(&Vec2 { x: 1.0, y: 4.0 }, &Vec2 { x: 2.0, y: 3.0 });
    /// assert_eq!(result, Vec2 { x: 2.0, y: 4.0 });
    /// ```
    pub fn max(one: &Self, other: &Self) -> Self {
        Self {
            x: one.x.max(other.x),
            y: one.y.max(other.y),
        }
    }

    /// Clamps each component of `value` between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let result = Vec2::clamp(&Vec2 { x: -1.0, y: 3.0 }, &Vec2::zero(), &Vec2::one());
    /// assert_eq!(result, Vec2 { x: 0.0, y: 1.0 });
    /// ```
    pub fn clamp(value: &Self, min: &Self, max: &Self) -> Self {
        Self {
            x: value.x.clamp(min.x, max.x),
            y: value.y.clamp(min.y, max.y),
        }
    }

    // member functions
    /// Normalizes this vector in place if its length is non-zero.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let mut vector = Vec2 { x: 3.0, y: 4.0 };
    /// vector.normalize();
    /// assert!((vector.length() - 1.0).abs() < 0.00001);
    /// ```
    pub fn normalize(&mut self) {
        let length = self.length_squared();
        if length != 0.0 {
            let inv_length = 1.0 / length.sqrt();
            self.x *= inv_length;
            self.y *= inv_length;
        }
    }

    /// Reflects this vector in place around a normal vector.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let mut vector = Vec2 { x: 1.0, y: -1.0 };
    /// vector.reflected(&Vec2::y_axis());
    /// assert_eq!(vector, Vec2 { x: 1.0, y: 1.0 });
    /// ```
    pub fn reflected(&mut self, normal: &Self) {
        let dot = Self::dot(self, normal);
        let translation = *normal * (2.0 * dot);
        *self -= translation;
    }

    /// Clamps this vector in place between the corresponding `min` and `max` components.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// let mut vector = Vec2 { x: -1.0, y: 3.0 };
    /// vector.clamped(&Vec2::zero(), &Vec2::one());
    /// assert_eq!(vector, Vec2 { x: 0.0, y: 1.0 });
    /// ```
    pub fn clamped(&mut self, min: &Self, max: &Self) {
        self.x = self.x.clamp(min.x, max.x);
        self.y = self.y.clamp(min.y, max.y);
    }

    /// Returns this vector's Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2 { x: 3.0, y: 4.0 }.length(), 5.0);
    /// ```
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Returns this vector's squared Euclidean length.
    ///
    /// ```rust
    /// use chaos_engine::math::Vec2;
    ///
    /// assert_eq!(Vec2 { x: 3.0, y: 4.0 }.length_squared(), 25.0);
    /// ```
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    // swizzling methods
    impl_swizzle2_all!((x, y), Vec2; x, y);
    impl_swizzle3_all!((x, y), Vec3; x, y);
    impl_swizzle4_all!((x, y), Vec4; x, y);
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
        }
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, scalar: f32) {
        self.x *= scalar;
        self.y *= scalar;
    }
}

impl Mul<Vec2> for Vec2 {
    type Output = Self;

    fn mul(self, other: Vec2) -> Self::Output {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
        }
    }
}

impl MulAssign<Vec2> for Vec2 {
    fn mul_assign(&mut self, other: Vec2) {
        self.x *= other.x;
        self.y *= other.y;
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;

    fn div(self, scalar: f32) -> Self::Output {
        Self {
            x: self.x / scalar,
            y: self.y / scalar,
        }
    }
}

impl DivAssign<f32> for Vec2 {
    fn div_assign(&mut self, scalar: f32) {
        self.x /= scalar;
        self.y /= scalar;
    }
}

impl Div<Vec2> for Vec2 {
    type Output = Self;

    fn div(self, other: Vec2) -> Self::Output {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
        }
    }
}

impl DivAssign<Vec2> for Vec2 {
    fn div_assign(&mut self, other: Vec2) {
        self.x /= other.x;
        self.y /= other.y;
    }
}

impl Neg for Vec2 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl_vector_reference_ops!(Vec2);

impl From<[f32; 2]> for Vec2 {
    fn from(arr: [f32; 2]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
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

    fn assert_vec2_eq(actual: Vec2, expected: Vec2) {
        assert_f32_eq(actual.x, expected.x);
        assert_f32_eq(actual.y, expected.y);
    }

    #[test]
    fn vec2_constructors_return_expected_values() {
        assert_eq!(Vec2::zero(), Vec2 { x: 0.0, y: 0.0 });
        assert_eq!(Vec2::one(), Vec2 { x: 1.0, y: 1.0 });
        assert_eq!(Vec2::x_axis(), Vec2 { x: 1.0, y: 0.0 });
        assert_eq!(Vec2::y_axis(), Vec2 { x: 0.0, y: 1.0 });
        assert_eq!(Vec2::splat(2.5), Vec2 { x: 2.5, y: 2.5 });
        assert_eq!(Vec2::from([1.0, 2.0]), Vec2 { x: 1.0, y: 2.0 });
    }

    #[test]
    fn vec2_swizzling_methods_return_expected_vectors() {
        let vector = Vec2 { x: 1.0, y: 2.0 };

        assert_eq!(vector.xx(), Vec2 { x: 1.0, y: 1.0 });
        assert_eq!(vector.xy(), Vec2 { x: 1.0, y: 2.0 });
        assert_eq!(vector.yx(), Vec2 { x: 2.0, y: 1.0 });
        assert_eq!(vector.yy(), Vec2 { x: 2.0, y: 2.0 });

        assert_eq!(
            vector.xxx(),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 1.0
            }
        );
        assert_eq!(
            vector.xxy(),
            Vec3 {
                x: 1.0,
                y: 1.0,
                z: 2.0
            }
        );
        assert_eq!(
            vector.xyx(),
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 1.0
            }
        );
        assert_eq!(
            vector.xyy(),
            Vec3 {
                x: 1.0,
                y: 2.0,
                z: 2.0
            }
        );
        assert_eq!(
            vector.yxx(),
            Vec3 {
                x: 2.0,
                y: 1.0,
                z: 1.0
            }
        );
        assert_eq!(
            vector.yxy(),
            Vec3 {
                x: 2.0,
                y: 1.0,
                z: 2.0
            }
        );
        assert_eq!(
            vector.yyx(),
            Vec3 {
                x: 2.0,
                y: 2.0,
                z: 1.0
            }
        );
        assert_eq!(
            vector.yyy(),
            Vec3 {
                x: 2.0,
                y: 2.0,
                z: 2.0
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
            vector.xxxy(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.xxyx(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.xxyy(),
            Vec4 {
                x: 1.0,
                y: 1.0,
                z: 2.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.xyxx(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 1.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.xyxy(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 1.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.xyyx(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.xyyy(),
            Vec4 {
                x: 1.0,
                y: 2.0,
                z: 2.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.yxxx(),
            Vec4 {
                x: 2.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.yxyx(),
            Vec4 {
                x: 2.0,
                y: 1.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.yxyy(),
            Vec4 {
                x: 2.0,
                y: 1.0,
                z: 2.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.yyxx(),
            Vec4 {
                x: 2.0,
                y: 2.0,
                z: 1.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.yyxy(),
            Vec4 {
                x: 2.0,
                y: 2.0,
                z: 1.0,
                w: 2.0,
            }
        );
        assert_eq!(
            vector.yyyx(),
            Vec4 {
                x: 2.0,
                y: 2.0,
                z: 2.0,
                w: 1.0,
            }
        );
        assert_eq!(
            vector.yyyy(),
            Vec4 {
                x: 2.0,
                y: 2.0,
                z: 2.0,
                w: 2.0,
            }
        );
    }

    #[test]
    fn vec2_measurement_and_normalization_methods_work() {
        let vector = Vec2 { x: 3.0, y: 4.0 };

        assert_eq!(vector.length(), 5.0);
        assert_eq!(vector.length_squared(), 25.0);
        assert_eq!(Vec2::distance(&Vec2::zero(), &vector), 5.0);
        assert_eq!(Vec2::distance_squared(&Vec2::zero(), &vector), 25.0);
        assert_eq!(
            Vec2::dot(&Vec2 { x: 1.0, y: 2.0 }, &Vec2 { x: 3.0, y: 4.0 }),
            11.0
        );
        assert_eq!(Vec2::cross(&Vec2::x_axis(), &Vec2::y_axis()), 1.0);
        assert_f32_eq(
            Vec2::angle(&Vec2::x_axis(), &Vec2::y_axis()),
            std::f32::consts::FRAC_PI_2,
        );

        let normalized = Vec2::normalized(&vector);
        assert_f32_eq(normalized.length(), 1.0);

        let mut mutable = vector;
        mutable.normalize();
        assert_vec2_eq(mutable, normalized);

        let mut zero = Vec2::zero();
        zero.normalize();
        assert_eq!(zero, Vec2::zero());
    }

    #[test]
    fn vec2_interpolation_projection_reflection_and_clamping_methods_work() {
        assert_vec2_eq(
            Vec2::lerp(&Vec2::zero(), &Vec2 { x: 10.0, y: 20.0 }, 0.25),
            Vec2 { x: 2.5, y: 5.0 },
        );

        let slerped = Vec2::slerp(&Vec2::x_axis(), &Vec2::y_axis(), 0.5);
        assert_vec2_eq(
            slerped,
            Vec2 {
                x: std::f32::consts::FRAC_1_SQRT_2,
                y: std::f32::consts::FRAC_1_SQRT_2,
            },
        );

        let nlerped = Vec2::nlerp(&Vec2::x_axis(), &Vec2::y_axis(), 0.5);
        assert_f32_eq(nlerped.length(), 1.0);

        assert_eq!(
            Vec2::reflect(&Vec2 { x: 1.0, y: -1.0 }, &Vec2::y_axis()),
            Vec2 { x: 1.0, y: 1.0 }
        );
        assert_eq!(
            Vec2::project(&Vec2 { x: 2.0, y: 3.0 }, &Vec2::x_axis()),
            Vec2 { x: 2.0, y: 0.0 }
        );
        assert_eq!(Vec2::project(&Vec2::one(), &Vec2::zero()), Vec2::zero());
        assert_eq!(
            Vec2::min(&Vec2 { x: 1.0, y: 4.0 }, &Vec2 { x: 2.0, y: 3.0 }),
            Vec2 { x: 1.0, y: 3.0 }
        );
        assert_eq!(
            Vec2::max(&Vec2 { x: 1.0, y: 4.0 }, &Vec2 { x: 2.0, y: 3.0 }),
            Vec2 { x: 2.0, y: 4.0 }
        );
        assert_eq!(
            Vec2::clamp(&Vec2 { x: -1.0, y: 3.0 }, &Vec2::zero(), &Vec2::one()),
            Vec2 { x: 0.0, y: 1.0 }
        );

        let mut reflected = Vec2 { x: 1.0, y: -1.0 };
        reflected.reflected(&Vec2::y_axis());
        assert_eq!(reflected, Vec2 { x: 1.0, y: 1.0 });

        let mut clamped = Vec2 { x: -1.0, y: 3.0 };
        clamped.clamped(&Vec2::zero(), &Vec2::one());
        assert_eq!(clamped, Vec2 { x: 0.0, y: 1.0 });
    }

    #[test]
    fn vec2_operator_impls_work_component_wise() {
        let mut vector = Vec2 { x: 2.0, y: 4.0 };

        assert_eq!(vector + Vec2 { x: 1.0, y: 2.0 }, Vec2 { x: 3.0, y: 6.0 });
        vector += Vec2 { x: 1.0, y: 2.0 };
        assert_eq!(vector, Vec2 { x: 3.0, y: 6.0 });
        assert_eq!(vector - Vec2 { x: 1.0, y: 2.0 }, Vec2 { x: 2.0, y: 4.0 });
        vector -= Vec2 { x: 1.0, y: 2.0 };
        assert_eq!(vector, Vec2 { x: 2.0, y: 4.0 });
        assert_eq!(vector * 2.0, Vec2 { x: 4.0, y: 8.0 });
        vector *= 2.0;
        assert_eq!(vector, Vec2 { x: 4.0, y: 8.0 });
        assert_eq!(vector * Vec2 { x: 2.0, y: 4.0 }, Vec2 { x: 8.0, y: 32.0 });
        vector *= Vec2 { x: 2.0, y: 4.0 };
        assert_eq!(vector, Vec2 { x: 8.0, y: 32.0 });
        assert_eq!(vector / 2.0, Vec2 { x: 4.0, y: 16.0 });
        vector /= 2.0;
        assert_eq!(vector, Vec2 { x: 4.0, y: 16.0 });
        assert_eq!(vector / Vec2 { x: 2.0, y: 4.0 }, Vec2 { x: 2.0, y: 4.0 });
        vector /= Vec2 { x: 2.0, y: 4.0 };
        assert_eq!(vector, Vec2 { x: 2.0, y: 4.0 });
        assert_eq!(-vector, Vec2 { x: -2.0, y: -4.0 });
    }

    #[test]
    fn vec2_reference_operator_impls_accept_borrowed_operands() {
        let lhs = Vec2 { x: 8.0, y: 16.0 };
        let rhs = Vec2 { x: 2.0, y: 4.0 };

        assert_eq!(lhs + &rhs, Vec2 { x: 10.0, y: 20.0 });
        assert_eq!(&lhs + rhs, Vec2 { x: 10.0, y: 20.0 });
        assert_eq!(&lhs + &rhs, Vec2 { x: 10.0, y: 20.0 });

        let mut assigned = lhs;
        assigned += &rhs;
        assert_eq!(assigned, Vec2 { x: 10.0, y: 20.0 });

        assert_eq!(lhs - &rhs, Vec2 { x: 6.0, y: 12.0 });
        assert_eq!(&lhs - rhs, Vec2 { x: 6.0, y: 12.0 });
        assert_eq!(&lhs - &rhs, Vec2 { x: 6.0, y: 12.0 });

        assigned = lhs;
        assigned -= &rhs;
        assert_eq!(assigned, Vec2 { x: 6.0, y: 12.0 });

        assert_eq!(&lhs * 2.0, Vec2 { x: 16.0, y: 32.0 });
        assert_eq!(lhs * &rhs, Vec2 { x: 16.0, y: 64.0 });
        assert_eq!(&lhs * rhs, Vec2 { x: 16.0, y: 64.0 });
        assert_eq!(&lhs * &rhs, Vec2 { x: 16.0, y: 64.0 });

        assigned = lhs;
        assigned *= &rhs;
        assert_eq!(assigned, Vec2 { x: 16.0, y: 64.0 });

        assert_eq!(&lhs / 2.0, Vec2 { x: 4.0, y: 8.0 });
        assert_eq!(lhs / &rhs, Vec2 { x: 4.0, y: 4.0 });
        assert_eq!(&lhs / rhs, Vec2 { x: 4.0, y: 4.0 });
        assert_eq!(&lhs / &rhs, Vec2 { x: 4.0, y: 4.0 });

        assigned = lhs;
        assigned /= &rhs;
        assert_eq!(assigned, Vec2 { x: 4.0, y: 4.0 });

        assert_eq!(-&lhs, Vec2 { x: -8.0, y: -16.0 });
    }
}
