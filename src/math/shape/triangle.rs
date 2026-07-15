use crate::math::Vec2;
use crate::math::matrix::Mat3;
use std::ops::Mul;

pub struct Triangle2D {
    pub a: Vec2,
    pub b: Vec2,
    pub c: Vec2,
}

impl Triangle2D {
    pub fn new(a: Vec2, b: Vec2, c: Vec2) -> Self {
        Self { a, b, c }
    }

    pub fn intersect(a: &Triangle2D, b: &Triangle2D) -> bool {
        // Check if the triangles are intersecting using the Separating Axis Theorem (SAT)
        let axes = [
            (a.b - a.a).perpendicular(),
            (a.c - a.b).perpendicular(),
            (a.a - a.c).perpendicular(),
            (b.b - b.a).perpendicular(),
            (b.c - b.b).perpendicular(),
            (b.a - b.c).perpendicular(),
        ];

        for axis in axes.iter() {
            let (min_a, max_a) = a.project_onto_axis(*axis);
            let (min_b, max_b) = b.project_onto_axis(*axis);
            if max_a < min_b || max_b < min_a {
                return false;
            }
        }

        true
    }

    pub fn is_ccw(&self) -> bool {
        let ab = self.b - self.a;
        let ac = self.c - self.a;

        Vec2::cross(&ab, &ac) > 0.0
    }

    pub fn is_concave(&self) -> bool {
        let ab = self.b - self.a;
        let ac = self.c - self.a;

        Vec2::cross(&ab, &ac) < 0.0
    }

    pub fn is_point_inside(&self, point: Vec2) -> bool {
        // The three directed edges of the triangle traversed in order are
        // A->B, B->C, C->A. For a point strictly inside, the sign of the
        // cross product between each edge and the vector from that edge's
        // starting vertex to the point must be the same for all three
        // (positive for CCW, negative for CW).
        let ab = self.b - self.a;
        let bc = self.c - self.b;
        let ca = self.a - self.c;

        let ap = point - self.a;
        let bp = point - self.b;
        let cp = point - self.c;

        let s0 = Vec2::cross(&ab, &ap);
        let s1 = Vec2::cross(&bc, &bp);
        let s2 = Vec2::cross(&ca, &cp);

        (s0 > 0.0 && s1 > 0.0 && s2 > 0.0) || (s0 < 0.0 && s1 < 0.0 && s2 < 0.0)
    }

    pub fn project_onto_axis(&self, axis: Vec2) -> (f32, f32) {
        let a_proj = Vec2::dot(&self.a, &axis);
        let b_proj = Vec2::dot(&self.b, &axis);
        let c_proj = Vec2::dot(&self.c, &axis);

        let min_proj = a_proj.min(b_proj).min(c_proj);
        let max_proj = a_proj.max(b_proj).max(c_proj);

        (min_proj, max_proj)
    }
}

impl Mul<Mat3> for Triangle2D {
    type Output = Triangle2D;

    fn mul(self, rhs: Mat3) -> Self::Output {
        Triangle2D::new(rhs * self.a, rhs * self.b, rhs * self.c)
    }
}

impl Mul<Mat3> for &Triangle2D {
    type Output = Triangle2D;

    fn mul(self, rhs: Mat3) -> Self::Output {
        Triangle2D::new(rhs * self.a, rhs * self.b, rhs * self.c)
    }
}
