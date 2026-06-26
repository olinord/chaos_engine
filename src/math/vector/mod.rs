macro_rules! impl_vector_reference_ops {
    ($type:ty) => {
        impl ::std::ops::Add<&$type> for $type {
            type Output = Self;

            fn add(self, rhs: &$type) -> Self::Output {
                self + *rhs
            }
        }

        impl ::std::ops::Add<$type> for &$type {
            type Output = $type;

            fn add(self, rhs: $type) -> Self::Output {
                *self + rhs
            }
        }

        impl ::std::ops::Add<&$type> for &$type {
            type Output = $type;

            fn add(self, rhs: &$type) -> Self::Output {
                *self + *rhs
            }
        }

        impl ::std::ops::AddAssign<&$type> for $type {
            fn add_assign(&mut self, rhs: &$type) {
                *self += *rhs;
            }
        }

        impl ::std::ops::Sub<&$type> for $type {
            type Output = Self;

            fn sub(self, rhs: &$type) -> Self::Output {
                self - *rhs
            }
        }

        impl ::std::ops::Sub<$type> for &$type {
            type Output = $type;

            fn sub(self, rhs: $type) -> Self::Output {
                *self - rhs
            }
        }

        impl ::std::ops::Sub<&$type> for &$type {
            type Output = $type;

            fn sub(self, rhs: &$type) -> Self::Output {
                *self - *rhs
            }
        }

        impl ::std::ops::SubAssign<&$type> for $type {
            fn sub_assign(&mut self, rhs: &$type) {
                *self -= *rhs;
            }
        }

        impl ::std::ops::Mul<f32> for &$type {
            type Output = $type;

            fn mul(self, scalar: f32) -> Self::Output {
                *self * scalar
            }
        }

        impl ::std::ops::Mul<&$type> for $type {
            type Output = Self;

            fn mul(self, rhs: &$type) -> Self::Output {
                self * *rhs
            }
        }

        impl ::std::ops::Mul<$type> for &$type {
            type Output = $type;

            fn mul(self, rhs: $type) -> Self::Output {
                *self * rhs
            }
        }

        impl ::std::ops::Mul<&$type> for &$type {
            type Output = $type;

            fn mul(self, rhs: &$type) -> Self::Output {
                *self * *rhs
            }
        }

        impl ::std::ops::MulAssign<&$type> for $type {
            fn mul_assign(&mut self, rhs: &$type) {
                *self *= *rhs;
            }
        }

        impl ::std::ops::Div<f32> for &$type {
            type Output = $type;

            fn div(self, scalar: f32) -> Self::Output {
                *self / scalar
            }
        }

        impl ::std::ops::Div<&$type> for $type {
            type Output = Self;

            fn div(self, rhs: &$type) -> Self::Output {
                self / *rhs
            }
        }

        impl ::std::ops::Div<$type> for &$type {
            type Output = $type;

            fn div(self, rhs: $type) -> Self::Output {
                *self / rhs
            }
        }

        impl ::std::ops::Div<&$type> for &$type {
            type Output = $type;

            fn div(self, rhs: &$type) -> Self::Output {
                *self / *rhs
            }
        }

        impl ::std::ops::DivAssign<&$type> for $type {
            fn div_assign(&mut self, rhs: &$type) {
                *self /= *rhs;
            }
        }

        impl ::std::ops::Neg for &$type {
            type Output = $type;

            fn neg(self) -> Self::Output {
                -*self
            }
        }
    };
}

pub mod vec2;
pub mod vec3;
pub mod vec4;
