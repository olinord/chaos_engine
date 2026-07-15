use std::collections::HashMap;

use chaos_engine::math::{Vec2, shape::triangle::Triangle2D};
use noise::{NoiseFn, Perlin};
use rand::{RngExt, SeedableRng, rngs::SmallRng};

pub struct ShapeComponent {
    pub shape: Vec<Triangle2D>,
    pub bounding_radius: f32,
}

impl ShapeComponent {
    fn compute_bounding_radius(shape: &[Triangle2D]) -> f32 {
        shape
            .iter()
            .flat_map(|tri| {
                [
                    tri.a.length_squared(),
                    tri.b.length_squared(),
                    tri.c.length_squared(),
                ]
            })
            .fold(0.0f32, f32::max)
            .sqrt()
    }

    fn triangulate_polygon(points: &[Vec2]) -> Vec<Triangle2D> {
        let mut triangles = Vec::new();

        if points.len() < 3 {
            return triangles;
        }

        let mut classifications: HashMap<usize, (usize, usize, usize)> = (0..points.len())
            .map(|i| {
                let prev_index = if i == 0 { points.len() - 1 } else { i - 1 };
                let next_index = (i + 1) % points.len();
                (i, (prev_index, i, next_index))
            })
            .collect();

        // Normalize to CCW winding so the convex test has a consistent sign.
        // Signed area is positive for CCW, negative for CW.
        let signed_area: f32 = (0..points.len())
            .map(|i| {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                a.x * b.y - b.x * a.y
            })
            .sum::<f32>()
            * 0.5;
        if signed_area < 0.0 {
            for v in classifications.values_mut() {
                std::mem::swap(&mut v.0, &mut v.2);
            }
        }

        let is_ear = |triangle: (usize, usize, usize),
                      points: &[Vec2],
                      classifications: &HashMap<usize, (usize, usize, usize)>|
         -> bool {
            let (prev, curr, next) = triangle;
            let triangle = Triangle2D::new(points[prev], points[curr], points[next]);

            if triangle.is_concave() {
                return false;
            }

            // Only test vertices that are still in the polygon — already-clipped
            // vertices are no longer part of the boundary and must be ignored.
            for &i in classifications.keys() {
                if i == prev || i == curr || i == next {
                    continue;
                }
                let p = points[i];
                if triangle.is_point_inside(p) {
                    return false;
                }
            }
            true
        };

        while classifications.len() > 3 {
            // Scan for an ear each iteration. Snapshot keys so we can mutate
            // `classifications` inside the loop.
            let candidates: Vec<usize> = classifications.keys().copied().collect();
            let mut clipped = false;

            for curr_index in candidates {
                let (prev_index, _, next_index) = classifications[&curr_index];

                if is_ear(
                    (prev_index, curr_index, next_index),
                    points,
                    &classifications,
                ) {
                    triangles.push(Triangle2D::new(
                        points[prev_index],
                        points[curr_index],
                        points[next_index],
                    ));

                    classifications.remove(&curr_index);

                    // Splice curr out: link prev <-> next.
                    if let Some(&(prev_prev_index, _, _)) = classifications.get(&prev_index) {
                        classifications
                            .insert(prev_index, (prev_prev_index, prev_index, next_index));
                    }
                    if let Some(&(_, _, next_next_index)) = classifications.get(&next_index) {
                        classifications
                            .insert(next_index, (prev_index, next_index, next_next_index));
                    }

                    clipped = true;
                    break;
                }
            }

            if !clipped {
                // Degenerate / self-intersecting input: no ear found.
                return triangles;
            }
        }

        // Emit the final remaining triangle.
        if classifications.len() == 3 {
            let (&i0, &(_, _, i1)) = classifications.iter().next().unwrap();
            let (_, _, i2) = classifications[&i1];
            triangles.push(Triangle2D::new(points[i0], points[i1], points[i2]));
        }

        triangles
    }

    pub fn ship() -> Self {
        let shape = vec![Triangle2D::new(
            Vec2::new(0.0, -0.35),
            Vec2::new(0.25, 0.35),
            Vec2::new(-0.25, 0.35),
        )];
        let bounding_radius = Self::compute_bounding_radius(&shape);
        Self {
            shape,
            bounding_radius,
        }
    }

    /// Generate an asteroid silhouette.
    /// algorithm:
    ///  1. Generate a set of "clip spheres" that carve concave craters out of the asteroid rim.
    ///  2. For each generated point on the asteroid rim, check if it intersects with any clip spheres
    ///     - if it does, clamp the radius to the near entry point of the sphere.
    ///  3. Apply fBm noise to the radius to create a rough, jagged silhouette.
    ///  4. Return the generated shape as a vector of Vec2 points
    pub fn asteroid(radius: f32, roughness: f32, seed: u32) -> Self {
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        let noise = Perlin::new(seed);

        let half_radius = radius * 0.5;
        let num_clip_spheres = rng.random_range(1..=15);

        // calculate the clip spheres that will carve concave craters out of the asteroid rim
        let mut clip_spheres: Vec<(Vec2, f32)> = Vec::with_capacity(num_clip_spheres);
        for _ in 0..num_clip_spheres {
            let clip_radius = rng.random_range(half_radius * 0.1..half_radius * 0.9);
            let angle = rng.random_range(0.0..std::f32::consts::TAU);
            let center = Vec2::new(radius * angle.cos(), radius * angle.sin());
            clip_spheres.push((center, clip_radius));
        }

        let num_points = 128;
        let mut shape = Vec::with_capacity(num_points);
        let min_r = radius * 0.15;

        for i in 0..num_points {
            // Step angles evenly around the circle so the emitted vertices
            // form an ordered simple polygon (random angles would produce a
            // self-intersecting scribble when connected in insertion order).
            let angle = (i as f32 / num_points as f32) * std::f32::consts::TAU;
            let (c, s) = (angle.cos(), angle.sin());

            // calculate the clipped radius by checking if the point intersects with any clip spheres
            let clipped_radius = {
                let mut r = radius;
                for (center, clip_radius) in &clip_spheres {
                    let b = c * center.x + s * center.y;
                    let cc = center.x * center.x + center.y * center.y - clip_radius * clip_radius;
                    let disc = b * b - cc;
                    if disc > 0.0 {
                        let t_enter = b - disc.sqrt();
                        if t_enter > 0.0 {
                            r = r.min(t_enter);
                        }
                    }
                }
                r
            };

            // apply fBm noise to the radius to create a rough, jagged silhouette
            let n1 = noise.get([c as f64 * 1.2, s as f64 * 1.2]) as f32; // big bays
            let n2 = noise.get([c as f64 * 2.5, s as f64 * 2.5]) as f32 * 0.25; // mid bumps
            let n3 = noise.get([c as f64 * 5.0, s as f64 * 5.0]) as f32 * 0.125; // small crags
            let fbm = n1 + n2 + n3;

            // Floor against `min_r` (not `radius`) so craters and fBm dips
            // actually reduce the rim; f32::max returns the larger value, so
            // clamping against `radius` would erase every concavity.
            let r = (clipped_radius * (1.0 + fbm * roughness)).max(min_r);
            shape.push(Vec2::new(r * c, r * s));
        }

        let triangulated = Self::triangulate_polygon(&shape);
        let bounding_radius = Self::compute_bounding_radius(&triangulated);
        return Self {
            shape: triangulated,
            bounding_radius,
        };
    }

    pub fn bullet() -> Self {
        let shape = vec![Triangle2D::new(
            Vec2::new(0.0, -0.1),
            Vec2::new(0.05, 0.1),
            Vec2::new(-0.05, 0.1),
        )];
        let bounding_radius = Self::compute_bounding_radius(&shape);
        Self {
            shape,
            bounding_radius,
        }
    }
}
