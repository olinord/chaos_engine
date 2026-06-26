use std::ops::{Mul, MulAssign};

use vulkano::buffer::BufferContents;

use crate::math::{Vec3, quaternion::Quaternion};

#[derive(Debug, Clone, Copy, PartialEq, BufferContents)]
#[repr(C)]
pub struct Mat3 {
    pub data: [[f32; 3]; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, BufferContents)]
#[repr(C)]
pub struct Mat4 {
    pub data: [[f32; 4]; 4],
}

impl Mat3 {
    const ROWS: usize = 3;
    const COLS: usize = 3;

    pub fn zero() -> Self {
        Self {
            data: [[0.0; Self::COLS]; Self::ROWS],
        }
    }

    pub fn identity() -> Self {
        let mut data = [[0.0; Self::COLS]; Self::ROWS];
        for i in 0..Self::ROWS {
            data[i][i] = 1.0;
        }
        Self { data }
    }

    pub fn scale(x: f32, y: f32) -> Self {
        let mut data = [[0.0; Self::COLS]; Self::ROWS];
        data[0][0] = x;
        data[1][1] = y;
        data[2][2] = 1.0;
        Self { data }
    }

    pub fn translation(x: f32, y: f32) -> Self {
        let mut mat = Self::identity();
        mat.data[2][0] = x;
        mat.data[2][1] = y;
        mat
    }

    pub fn rotation(angle: f32) -> Self {
        let mut mat = Self::identity();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        mat.data[0][0] = cos_angle;
        mat.data[0][1] = -sin_angle;
        mat.data[1][0] = sin_angle;
        mat.data[1][1] = cos_angle;

        mat
    }

    pub fn transpose(&self) -> Self {
        let mut transposed_data = [[0.0; Self::ROWS]; Self::COLS];
        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                transposed_data[j][i] = self.data[i][j];
            }
        }
        Self {
            data: transposed_data,
        }
    }

    pub fn inverse(&self) -> Option<Self> {
        let mut inv = [[0.0; Self::COLS]; Self::ROWS];
        let mut det: f64 = 0.0;

        for i in 0..Self::ROWS {
            det += self.data[0][i] as f64
                * (self.data[1][(i + 1) % Self::COLS] as f64
                    * self.data[2][(i + 2) % Self::COLS] as f64
                    - self.data[1][(i + 2) % Self::COLS] as f64
                        * self.data[2][(i + 1) % Self::COLS] as f64);
        }

        if det == 0.0 {
            return None;
        }

        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                inv[j][i] = ((self.data[(i + 1) % Self::ROWS][(j + 1) % Self::COLS] as f64
                    * self.data[(i + 2) % Self::ROWS][(j + 2) % Self::COLS] as f64
                    - self.data[(i + 1) % Self::ROWS][(j + 2) % Self::COLS] as f64
                        * self.data[(i + 2) % Self::ROWS][(j + 1) % Self::COLS] as f64)
                    / det) as f32;
            }
        }

        Some(Self { data: inv })
    }
}

impl From<[f32; 9]> for Mat3 {
    fn from(arr: [f32; 9]) -> Self {
        let mut data = [[0.0; Self::COLS]; Self::ROWS];
        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                data[i][j] = arr[i * Self::COLS + j];
            }
        }
        Self { data }
    }
}

impl From<[[f32; 3]; 3]> for Mat3 {
    fn from(arr: [[f32; 3]; 3]) -> Self {
        Self { data: arr }
    }
}

impl Mat4 {
    const ROWS: usize = 4;
    const COLS: usize = 4;

    pub fn zero() -> Self {
        Self {
            data: [[0.0; Self::COLS]; Self::ROWS],
        }
    }

    pub fn identity() -> Self {
        let mut data = [[0.0; Self::COLS]; Self::ROWS];
        for i in 0..Self::ROWS {
            data[i][i] = 1.0;
        }
        Self { data }
    }

    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        let mut mat = Self::identity();
        mat.data[0][0] = x;
        mat.data[1][1] = y;
        mat.data[2][2] = z;
        mat
    }

    pub fn translation(x: f32, y: f32, z: f32) -> Self {
        let mut mat = Self::identity();
        mat.data[3][0] = x;
        mat.data[3][1] = y;
        mat.data[3][2] = z;
        mat
    }

    pub fn rotation(quat: &Quaternion) -> Self {
        return (*quat).into();
    }

    pub fn rotation_from_axis_angle(axis: &Vec3, angle: f32) -> Self {
        return Quaternion::from_axis_angle(axis, angle).into();
    }

    pub fn rotation_from_euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        return Quaternion::from_euler_angles(roll, pitch, yaw).into();
    }

    pub fn rotation_x(angle: f32) -> Self {
        let mut mat = Self::identity();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        mat.data[1][1] = cos_angle;
        mat.data[1][2] = -sin_angle;
        mat.data[2][1] = sin_angle;
        mat.data[2][2] = cos_angle;

        mat
    }

    pub fn rotation_y(angle: f32) -> Self {
        let mut mat = Self::identity();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        mat.data[0][0] = cos_angle;
        mat.data[0][2] = sin_angle;
        mat.data[2][0] = -sin_angle;
        mat.data[2][2] = cos_angle;

        mat
    }

    pub fn rotation_z(angle: f32) -> Self {
        let mut mat = Self::identity();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        mat.data[0][0] = cos_angle;
        mat.data[0][1] = -sin_angle;
        mat.data[1][0] = sin_angle;
        mat.data[1][1] = cos_angle;

        mat
    }

    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let mut mat = Self::zero();
        let f = 1.0 / (fov_y / 2.0).tan();

        mat.data[0][0] = f / aspect;
        mat.data[1][1] = f;
        mat.data[2][2] = (far + near) / (near - far);
        mat.data[2][3] = -1.0;
        mat.data[3][2] = (2.0 * far * near) / (near - far);

        mat
    }

    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let mut mat = Self::identity();

        mat.data[0][0] = 2.0 / (right - left);
        mat.data[1][1] = 2.0 / (top - bottom);
        mat.data[2][2] = -2.0 / (far - near);

        mat.data[3][0] = -(right + left) / (right - left);
        mat.data[3][1] = -(top + bottom) / (top - bottom);

        mat
    }

    pub fn look_at(eye: &Vec3, center: &Vec3, up: &Vec3) -> Self {
        let f = Vec3::normalized(&(center - eye));
        let s = Vec3::normalized(&Vec3::cross(&f, up));
        let u = Vec3::cross(&s, &f);

        let mut mat = Self::identity();

        mat.data[0][0] = s.x;
        mat.data[1][0] = s.y;
        mat.data[2][0] = s.z;

        mat.data[0][1] = u.x;
        mat.data[1][1] = u.y;
        mat.data[2][1] = u.z;

        mat.data[0][2] = -f.x;
        mat.data[1][2] = -f.y;
        mat.data[2][2] = -f.z;

        mat.data[3][0] = -Vec3::dot(&s, eye);
        mat.data[3][1] = -Vec3::dot(&u, eye);
        mat.data[3][2] = Vec3::dot(&f, eye);

        mat
    }

    pub fn transform(translation: &Vec3, rotation: &Quaternion, scale: &Vec3) -> Self {
        let mut mat = Self::identity();

        // Apply scaling
        mat.data[0][0] = scale.x;
        mat.data[1][1] = scale.y;
        mat.data[2][2] = scale.z;

        // Apply rotation
        let rotation_matrix: Mat4 = (*rotation).into();
        mat = mat * rotation_matrix;

        // Apply translation
        mat.data[3][0] = translation.x;
        mat.data[3][1] = translation.y;
        mat.data[3][2] = translation.z;

        mat
    }

    pub fn transpose(&self) -> Self {
        let mut transposed_data = [[0.0; Self::ROWS]; Self::COLS];
        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                transposed_data[j][i] = self.data[i][j];
            }
        }
        Self {
            data: transposed_data,
        }
    }

    pub fn inverse(&self) -> Option<Self> {
        let mut inv = [[0.0; Self::COLS]; Self::ROWS];
        let mut det: f64 = 0.0;

        for i in 0..Self::ROWS {
            det += self.data[0][i] as f64
                * (self.data[1][(i + 1) % Self::COLS] as f64
                    * self.data[2][(i + 2) % Self::COLS] as f64
                    - self.data[1][(i + 2) % Self::COLS] as f64
                        * self.data[2][(i + 1) % Self::COLS] as f64);
        }

        if det == 0.0 {
            return None;
        }

        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                inv[j][i] = ((self.data[(i + 1) % Self::ROWS][(j + 1) % Self::COLS] as f64
                    * self.data[(i + 2) % Self::ROWS][(j + 2) % Self::COLS] as f64
                    - self.data[(i + 1) % Self::ROWS][(j + 2) % Self::COLS] as f64
                        * self.data[(i + 2) % Self::ROWS][(j + 1) % Self::COLS] as f64)
                    / det) as f32;
            }
        }

        Some(Self { data: inv })
    }
}

impl From<[f32; 16]> for Mat4 {
    fn from(arr: [f32; 16]) -> Self {
        let mut data = [[0.0; Self::COLS]; Self::ROWS];
        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                data[i][j] = arr[i * Self::COLS + j];
            }
        }
        Self { data }
    }
}

impl From<[[f32; 4]; 4]> for Mat4 {
    fn from(arr: [[f32; 4]; 4]) -> Self {
        Self { data: arr }
    }
}

impl From<Quaternion> for Mat4 {
    fn from(quat: Quaternion) -> Self {
        let mut mat = Mat4::identity();

        let x2 = quat.a + quat.a;
        let y2 = quat.b + quat.b;
        let z2 = quat.c + quat.c;

        let xx2 = quat.a * x2;
        let yy2 = quat.b * y2;
        let zz2 = quat.c * z2;

        mat.data[0][0] = 1.0 - yy2 - zz2;
        mat.data[0][1] = (quat.b * z2) - (quat.d * y2);
        mat.data[0][2] = (quat.c * x2) + (quat.d * x2);

        mat.data[1][0] = (quat.b * z2) + (quat.d * y2);
        mat.data[1][1] = 1.0 - xx2 - zz2;
        mat.data[1][2] = (quat.c * y2) - (quat.d * x2);

        mat.data[2][0] = (quat.c * x2) - (quat.d * y2);
        mat.data[2][1] = (quat.c * y2) + (quat.d * x2);
        mat.data[2][2] = 1.0 - xx2 - yy2;

        mat
    }
}

impl Mul<Mat4> for Mat4 {
    type Output = Self;

    fn mul(self, other: Mat4) -> Self::Output {
        let mut result = Mat4::zero();

        for i in 0..Self::ROWS {
            for j in 0..Self::COLS {
                for k in 0..Self::COLS {
                    result.data[i][j] += self.data[i][k] * other.data[k][j];
                }
            }
        }

        result
    }
}

impl MulAssign<Mat4> for Mat4 {
    fn mul_assign(&mut self, other: Mat4) {
        *self = *self * other;
    }
}
