//! A sampled cubic curve supplies coordinates for ordinary scalar fields.

const TOLERANCE: f32 = 0.0005;
const MAX_DEPTH: usize = 10;

struct Segment {
    start: [f32; 2],
    delta: [f32; 2],
    length_squared: f32,
    length: f32,
    along: f32,
    start_t: f32,
    end_t: f32,
}

pub struct BezierFrame {
    origin: [f32; 2],
    segments: Vec<Segment>,
    length: f32,
    coefficients: [[f64; 2]; 4],
}

impl BezierFrame {
    /// Prepared once per binding set, rather than subdividing for each pixel.
    pub fn new(points: [[f32; 2]; 4]) -> Result<Self, String> {
        if !points.iter().flatten().all(|value| value.is_finite()) {
            return Err("bezier control points must be finite".into());
        }
        let mut vertices = vec![(0.0, points[0])];
        flatten(points, [0.0, 1.0], 0, &mut vertices);
        let mut segments = Vec::new();
        let mut length = 0.0;
        for pair in vertices.windows(2) {
            let delta = subtract(pair[1].1, pair[0].1);
            let length_squared = dot(delta, delta);
            if length_squared == 0.0 {
                continue;
            }
            let segment_length = length_squared.sqrt();
            segments.push(Segment {
                start: pair[0].1,
                delta,
                length_squared,
                length: segment_length,
                along: length,
                start_t: pair[0].0,
                end_t: pair[1].0,
            });
            length += segment_length;
        }
        if !length.is_finite() {
            return Err("bezier curve length must be finite".into());
        }
        // Refining a minimum in f64 avoids losing small distance improvements
        // and changing its location when the same curve is traversed backward.
        let points = points.map(|point| point.map(f64::from));
        let coefficients = [
            std::array::from_fn(|i| {
                -points[0][i] + 3.0 * points[1][i] - 3.0 * points[2][i] + points[3][i]
            }),
            std::array::from_fn(|i| 3.0 * points[0][i] - 6.0 * points[1][i] + 3.0 * points[2][i]),
            std::array::from_fn(|i| 3.0 * (points[1][i] - points[0][i])),
            points[0],
        ];
        if !coefficients.iter().flatten().all(|value| value.is_finite()) {
            return Err("bezier coefficients must be finite".into());
        }
        Ok(Self {
            origin: points[0].map(|value| value as f32),
            segments,
            length,
            coefficients,
        })
    }

    /// x: arc length relative to the curve's halfway distance.
    /// y: distance to the finite curve, positive to its left as traversed.
    pub fn coordinates(&self, point: [f32; 2]) -> [f32; 2] {
        if self.segments.is_empty() {
            let offset = subtract(point, self.origin);
            return [0.0, offset[0].hypot(offset[1])];
        }
        let mut best_squared = f32::INFINITY;
        let mut best_t = 0.0;
        for segment in &self.segments {
            let offset = subtract(point, segment.start);
            let t = (dot(offset, segment.delta) / segment.length_squared).clamp(0.0, 1.0);
            let residual = [
                offset[0] - t * segment.delta[0],
                offset[1] - t * segment.delta[1],
            ];
            let squared = dot(residual, residual);
            // Equal-distance ties resolve to the first segment in curve order.
            if squared < best_squared {
                best_squared = squared;
                best_t = segment.start_t + t * (segment.end_t - segment.start_t);
            }
        }
        // Refine the nearby minimum on the cubic itself. This avoids changes in
        // waveform phase caused by projecting onto flat chords at a distance.
        let point64 = point.map(f64::from);
        let mut t = f64::from(best_t);
        for _ in 0..5 {
            let (position, tangent, acceleration) = self.at(t);
            let residual = [position[0] - point64[0], position[1] - point64[1]];
            let denominator = dot64(tangent, tangent) + dot64(residual, acceleration);
            if denominator.abs() < 1e-10 {
                break;
            }
            let next = (t - dot64(residual, tangent) / denominator).clamp(0.0, 1.0);
            let next_position = self.at(next).0;
            let next_residual = [next_position[0] - point64[0], next_position[1] - point64[1]];
            if dot64(next_residual, next_residual) > dot64(residual, residual) {
                break;
            }
            let change = (next - t).abs();
            t = next;
            if change < 1e-6 {
                break;
            }
        }
        let (position, tangent, _) = self.at(t);
        let position = position.map(|value| value as f32);
        let mut tangent = tangent.map(|value| value as f32);
        let index = self
            .segments
            .partition_point(|segment| segment.end_t < t as f32)
            .min(self.segments.len() - 1);
        let segment = &self.segments[index];
        let fraction = (dot(subtract(position, segment.start), segment.delta)
            / segment.length_squared)
            .clamp(0.0, 1.0);
        if dot(tangent, tangent) < 1e-12 {
            tangent = segment.delta;
        }
        let residual = subtract(point, position);
        let side = tangent[0] * residual[1] - tangent[1] * residual[0];
        [
            segment.along + fraction * segment.length - self.length * 0.5,
            residual[0].hypot(residual[1]).copysign(side),
        ]
    }

    fn at(&self, t: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
        let [a, b, c, d] = self.coefficients;
        (
            std::array::from_fn(|i| ((a[i] * t + b[i]) * t + c[i]) * t + d[i]),
            std::array::from_fn(|i| (3.0 * a[i] * t + 2.0 * b[i]) * t + c[i]),
            std::array::from_fn(|i| 6.0 * a[i] * t + 2.0 * b[i]),
        )
    }
}

fn subtract(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

fn dot64(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [0.5 * a[0] + 0.5 * b[0], 0.5 * a[1] + 0.5 * b[1]]
}

fn distance_to_chord_squared(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let delta = subtract(b, a);
    let offset = subtract(point, a);
    let length_squared = dot(delta, delta);
    let t = if length_squared == 0.0 {
        0.0
    } else {
        (dot(offset, delta) / length_squared).clamp(0.0, 1.0)
    };
    let residual = [offset[0] - t * delta[0], offset[1] - t * delta[1]];
    dot(residual, residual)
}

fn flatten(
    points: [[f32; 2]; 4],
    range: [f32; 2],
    depth: usize,
    vertices: &mut Vec<(f32, [f32; 2])>,
) {
    let [a, b, c, d] = points;
    if depth == MAX_DEPTH
        || (distance_to_chord_squared(b, a, d) <= TOLERANCE * TOLERANCE
            && distance_to_chord_squared(c, a, d) <= TOLERANCE * TOLERANCE)
    {
        vertices.push((range[1], d));
        return;
    }
    // de Casteljau subdivision at t = 1/2.
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let cd = midpoint(c, d);
    let abc = midpoint(ab, bc);
    let bcd = midpoint(bc, cd);
    let center = midpoint(abc, bcd);
    let middle = (range[0] + range[1]) * 0.5;
    flatten(
        [a, ab, abc, center],
        [range[0], middle],
        depth + 1,
        vertices,
    );
    flatten(
        [center, bcd, cd, d],
        [middle, range[1]],
        depth + 1,
        vertices,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_curve_has_world_unit_coordinates_and_round_endpoint_distances() {
        let frame =
            BezierFrame::new([[-0.75, 0.0], [-0.25, 0.0], [0.25, 0.0], [0.75, 0.0]]).unwrap();
        for (point, expected) in [
            ([0.2, 0.1], [0.2, 0.1]),
            ([-0.4, -0.2], [-0.4, -0.2]),
            ([1.0, 0.0], [0.75, 0.25]),
            ([1.0, -0.2], [0.75, -0.25_f32.hypot(0.2)]),
            ([-1.0, 0.2], [-0.75, 0.25_f32.hypot(0.2)]),
        ] {
            let actual = frame.coordinates(point);
            for axis in 0..2 {
                assert!(
                    (actual[axis] - expected[axis]).abs() < 1e-6,
                    "{point:?}: {actual:?}"
                );
            }
        }
    }

    #[test]
    fn curved_coordinates_agree_with_an_independent_dense_cubic_reference() {
        let controls = [[-0.8, -0.72], [1.15, -0.65], [-1.15, 0.65], [0.8, 0.72]];
        let frame = BezierFrame::new(controls).unwrap();
        // Direct Bernstein evaluation independently checks the adaptive subdivision.
        let vertices: Vec<[f32; 2]> = (0..=8000)
            .map(|i| {
                let t = i as f32 / 8000.0;
                let u = 1.0 - t;
                std::array::from_fn(|axis| {
                    u * u * u * controls[0][axis]
                        + 3.0 * u * u * t * controls[1][axis]
                        + 3.0 * u * t * t * controls[2][axis]
                        + t * t * t * controls[3][axis]
                })
            })
            .collect();
        let mut distances = vec![0.0];
        for pair in vertices.windows(2) {
            let delta = subtract(pair[1], pair[0]);
            distances.push(distances.last().unwrap() + delta[0].hypot(delta[1]));
        }
        let total = *distances.last().unwrap();
        assert!((frame.length - total).abs() < 0.002);
        for point in [
            [0.0, 0.0],
            [0.3, 0.3],
            [-0.8, -0.4],
            [0.6, -0.5],
            [-0.2, 0.7],
        ] {
            let (index, distance) = vertices
                .iter()
                .enumerate()
                .map(|(i, vertex)| {
                    let delta = subtract(point, *vertex);
                    (i, delta[0].hypot(delta[1]))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            let actual = frame.coordinates(point);
            assert!(
                (actual[1].abs() - distance).abs() < 0.001,
                "{point:?}: distance {}, expected {distance}",
                actual[1]
            );
            assert!(
                (actual[0] - (distances[index] - total * 0.5)).abs() < 0.002,
                "{point:?}: along {}, expected {}",
                actual[0],
                distances[index] - total * 0.5
            );
        }
        let mut reversed = controls;
        reversed.reverse();
        let reverse = BezierFrame::new(reversed).unwrap();
        for point in [[0.3, 0.3], [-0.8, -0.4], [0.6, -0.5]] {
            let forward = frame.coordinates(point);
            let backward = reverse.coordinates(point);
            assert!(
                (forward[0] + backward[0]).abs() < 1e-5,
                "{point:?}: forward {forward:?}, backward {backward:?}"
            );
            assert!(
                (forward[1] + backward[1]).abs() < 1e-5,
                "{point:?}: forward {forward:?}, backward {backward:?}"
            );
        }
    }

    #[test]
    fn collapsed_repeated_and_closed_control_points_remain_defined() {
        let collapsed = BezierFrame::new([[0.2, 0.3]; 4]).unwrap();
        let sample = collapsed.coordinates([0.5, 0.7]);
        assert_eq!(sample[0], 0.0);
        assert!((sample[1] - 0.5).abs() < 1e-6);
        let repeated = BezierFrame::new([[0.0, 0.0], [0.0, 0.0], [1.0, 0.0], [1.0, 0.0]]).unwrap();
        assert_eq!(repeated.coordinates([0.5, 0.2]), [0.0, 0.2]);
        let closed = BezierFrame::new([[0.0, 0.0], [1.0, 1.0], [-1.0, 1.0], [0.0, 0.0]]).unwrap();
        assert!(closed.length > 1.0);
        assert!(closed.coordinates([0.0, 0.75])[1].abs() < 1e-6);
        assert_eq!(closed.coordinates([0.0, 0.0])[0], -closed.length * 0.5);
        assert!(BezierFrame::new([[f32::NAN, 0.0]; 4]).is_err());
        assert!(BezierFrame::new([[f32::INFINITY, 0.0]; 4]).is_err());
    }
}
