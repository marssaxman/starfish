//! Deterministic CPU evaluation of a validated, lowered program.

use std::f32::consts::TAU;

use crate::{
    bezier::BezierFrame,
    program::{Operation, ParameterBindings, StarfishProgram, Type},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Point([f32; 2]),
    Color([f32; 3]),
}

impl Value {
    fn scalar(self) -> f32 {
        match self {
            Self::Scalar(v) => v,
            _ => unreachable!("validated scalar input"),
        }
    }
    fn point(self) -> [f32; 2] {
        match self {
            Self::Point(v) => v,
            _ => unreachable!("validated point input"),
        }
    }
}

pub struct Evaluator<'a> {
    program: &'a StarfishProgram,
    bindings: &'a ParameterBindings,
    output: usize,
    order: Vec<usize>,
    values: Vec<Value>,
    rotations: Vec<Option<(f32, f32)>>,
    curves: Vec<Option<BezierFrame>>,
}

impl<'a> Evaluator<'a> {
    pub fn new(
        program: &'a StarfishProgram,
        bindings: &'a ParameterBindings,
        output: &str,
    ) -> Result<Self, String> {
        let item = program
            .item(output)
            .ok_or_else(|| format!("Unknown output: {output}"))?;
        if !matches!(item.ty, Type::FieldScalar | Type::FieldColor) {
            return Err(format!("{} cannot be displayed as an image", item.name));
        }
        if bindings.0.len() != program.parameters.len() {
            return Err("Parameter binding count does not match the program".into());
        }
        for (value, parameter) in bindings.0.iter().zip(&program.parameters) {
            if !value.is_finite() || *value < parameter.min || *value > parameter.max {
                return Err(format!(
                    "Binding {} is outside its declared range",
                    parameter.id
                ));
            }
        }
        let mut seen = vec![false; program.nodes.len()];
        let mut order = Vec::new();
        fn visit(
            index: usize,
            program: &StarfishProgram,
            seen: &mut [bool],
            order: &mut Vec<usize>,
        ) {
            if seen[index] {
                return;
            }
            seen[index] = true;
            for &input in &program.nodes[index].inputs {
                visit(input, program, seen, order);
            }
            order.push(index);
        }
        visit(item.node, program, &mut seen, &mut order);
        let mut evaluator = Self {
            program,
            bindings,
            output: item.node,
            order,
            values: vec![Value::Scalar(0.0); program.nodes.len()],
            rotations: vec![None; program.nodes.len()],
            curves: (0..program.nodes.len()).map(|_| None).collect(),
        };
        // Uniform values and rotation angles depend on bindings, but not on pixels.
        for &index in &evaluator.order.clone() {
            let node = &program.nodes[index];
            if matches!(node.operation, Operation::Rotate) {
                evaluator.rotations[index] =
                    Some(evaluator.values[node.inputs[1]].scalar().sin_cos());
            }
            if matches!(node.operation, Operation::Bezier) {
                let points =
                    std::array::from_fn(|slot| evaluator.values[node.inputs[slot + 1]].point());
                evaluator.curves[index] = Some(BezierFrame::new(points)?);
            }
            if !matches!(
                node.ty,
                Type::FieldScalar | Type::FieldPoint | Type::FieldColor
            ) {
                evaluator.evaluate_node(index, 0.0, 0.0);
            }
        }
        for &index in &evaluator.order {
            let node = &program.nodes[index];
            if matches!(node.operation, Operation::Pulse) {
                let width = evaluator.values[node.inputs[3]].scalar();
                if !width.is_finite() || !(0.0..=1.0).contains(&width) {
                    return Err("pulse width must be finite and within [0, 1]".into());
                }
            }
        }
        Ok(evaluator)
    }

    pub fn sample(&mut self, x: f32, y: f32) -> Value {
        for offset in 0..self.order.len() {
            let index = self.order[offset];
            if matches!(
                self.program.nodes[index].ty,
                Type::FieldScalar | Type::FieldPoint | Type::FieldColor
            ) {
                self.evaluate_node(index, x, y);
            }
        }
        self.values[self.output]
    }

    fn evaluate_node(&mut self, index: usize, x: f32, y: f32) {
        let node = &self.program.nodes[index];
        let input = |slot: usize| self.values[node.inputs[slot]];
        self.values[index] = match &node.operation {
            Operation::Dummy => unreachable!("component validation nodes have no output path"),
            Operation::Scalar(v) => Value::Scalar(*v),
            Operation::Vec2 => Value::Point([input(0).scalar(), input(1).scalar()]),
            Operation::World => Value::Point([x, y]),
            Operation::Parameter(i) => Value::Scalar(self.bindings.0[*i]),
            Operation::Rotate => {
                let [x, y] = input(0).point();
                let (sin, cos) =
                    self.rotations[index].expect("rotation prepared from a uniform angle");
                Value::Point([cos * x - sin * y, sin * x + cos * y])
            }
            Operation::Dot => {
                let [x, y] = input(0).point();
                let [a, b] = input(1).point();
                Value::Scalar(x * a + y * b)
            }
            Operation::Bezier => Value::Point(
                self.curves[index]
                    .as_ref()
                    .expect("curve prepared from uniform control points")
                    .coordinates(input(0).point()),
            ),
            Operation::Abs => Value::Scalar(input(0).scalar().abs()),
            Operation::Sub if node.ty == Type::FieldPoint => {
                let [x, y] = input(0).point();
                let [a, b] = input(1).point();
                Value::Point([x - a, y - b])
            }
            Operation::Sub => Value::Scalar(input(0).scalar() - input(1).scalar()),
            Operation::Length => {
                let [x, y] = input(0).point();
                Value::Scalar(x.hypot(y))
            }
            Operation::Wave => Value::Scalar(
                (TAU * input(1).scalar() * input(0).scalar() + input(2).scalar()).sin(),
            ),
            Operation::Triangle | Operation::Saw | Operation::Pulse => {
                let cycles = input(1).scalar() * input(0).scalar() + input(2).scalar() / TAU;
                Value::Scalar(match node.operation {
                    Operation::Triangle => {
                        1.0 - 4.0 * ((cycles + 0.25).rem_euclid(1.0) - 0.5).abs()
                    }
                    Operation::Saw => 2.0 * (cycles + 0.5).rem_euclid(1.0) - 1.0,
                    Operation::Pulse => {
                        let width = input(3).scalar();
                        // A tiny negative cycle can round to 1 after wrapping.
                        if width == 1.0 || cycles.rem_euclid(1.0) < width {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    _ => unreachable!(),
                })
            }
            Operation::Add => Value::Scalar(input(0).scalar() + input(1).scalar()),
            Operation::Multiply => Value::Scalar(input(0).scalar() * input(1).scalar()),
            Operation::Negate => Value::Scalar(-input(0).scalar()),
            Operation::Tanh => Value::Scalar(input(0).scalar().tanh()),
            Operation::Palette(palette) => Value::Color(palette_sample(
                input(0).scalar(),
                &self.program.palettes[*palette].stops,
            )),
        };
    }
}

fn palette_sample(field: f32, stops: &[[f32; 3]]) -> [f32; 3] {
    let position = ((field + 1.0) * 0.5).clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let index = (position.floor() as usize).min(stops.len() - 2);
    let fraction = position - index as f32;
    let fraction = fraction * fraction * (3.0 - 2.0 * fraction);
    std::array::from_fn(|channel| {
        stops[index][channel] * (1.0 - fraction) + stops[index + 1][channel] * fraction
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_fields_follow_shared_rotation_and_validate_computed_points() {
        let program =
            StarfishProgram::from_source(include_str!("../programs/bezier_study.sf")).unwrap();
        let rotation = program
            .parameters
            .iter()
            .position(|p| p.id == "rotation")
            .unwrap();
        let mut base = ParameterBindings::defaults(&program);
        base.0[rotation] = 0.0;
        let mut rotated = base.clone();
        rotated.0[rotation] = TAU * 0.25;
        for output in ["along", "across", "distance", "modulated", "composition"] {
            let expected = Evaluator::new(&program, &base, output)
                .unwrap()
                .sample(0.23, -0.31)
                .scalar();
            let actual = Evaluator::new(&program, &rotated, output)
                .unwrap()
                .sample(-0.31, -0.23)
                .scalar();
            assert!(
                (expected - actual).abs() < 3e-5,
                "{output}: {actual} != {expected}"
            );
        }
        let invalid = program.source.replace(
            "point2(start_x, start_y)",
            "point2(300000000000000000000000000000000000000 * 2, start_y)",
        );
        let invalid = StarfishProgram::from_source(&invalid).unwrap();
        let error = match Evaluator::new(&invalid, &base, "composition") {
            Err(error) => error,
            Ok(_) => panic!("accepted a computed infinite control point"),
        };
        assert!(error.contains("control points must be finite"));
    }

    const WAVEFORMS: &str = r#"starfish 1; program "Waveforms" {
        param frequency: Frequency = 1 in -4..4 step 0.1;
        param phase: Angle = 0rad in -180deg..180deg step 1deg;
        param width: Scalar = 0.25 in -1..2 step 0.01;
        field coordinate = dot(world, vec2(1, 0));
        field sine = wave(coordinate, frequency, phase);
        field triangular = triangle(coordinate, frequency, phase);
        field sawtooth = saw(coordinate, frequency, phase);
        field pulses = pulse(coordinate, frequency, phase, width);
        palette ink = smooth [rgb(0, 0, 0), rgb(1, 1, 1)];
        output image = palette(ink, pulses);
    }"#;

    #[test]
    fn waveform_profiles_have_known_values_including_negative_coordinates() {
        let program = StarfishProgram::from_source(WAVEFORMS).unwrap();
        let bindings = ParameterBindings::defaults(&program);
        for (output, samples) in [
            (
                "triangular",
                vec![
                    (-0.25, -1.0),
                    (0.0, 0.0),
                    (0.125, 0.5),
                    (0.25, 1.0),
                    (0.5, 0.0),
                    (0.75, -1.0),
                ],
            ),
            (
                "sawtooth",
                vec![
                    (-0.75, 0.5),
                    (-0.25, -0.5),
                    (0.0, 0.0),
                    (0.25, 0.5),
                    (0.5, -1.0),
                    (0.75, -0.5),
                ],
            ),
            (
                "pulses",
                vec![
                    (-0.875, 1.0),
                    (-0.125, -1.0),
                    (0.0, 1.0),
                    (0.125, 1.0),
                    (0.25, -1.0),
                    (0.75, -1.0),
                ],
            ),
        ] {
            let mut evaluator = Evaluator::new(&program, &bindings, output).unwrap();
            for (x, expected) in samples {
                let actual = evaluator.sample(x, 0.0).scalar();
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "{output} at {x}: {actual}"
                );
            }
        }
    }

    #[test]
    fn profiles_share_frequency_phase_and_periodic_semantics() {
        let program = StarfishProgram::from_source(WAVEFORMS).unwrap();
        let base = ParameterBindings::defaults(&program);
        for output in ["sine", "triangular", "sawtooth", "pulses"] {
            for frequency in [-2.0, 0.0, 2.0] {
                let mut shifted = base.clone();
                shifted.0[0] = frequency;
                shifted.0[1] = TAU * 0.25;
                let mut actual = Evaluator::new(&program, &shifted, output).unwrap();
                let mut expected = Evaluator::new(&program, &base, output).unwrap();
                for x in [-1.125, -0.0625, 0.1875, 0.8125] {
                    let value = actual.sample(x, 0.0).scalar();
                    let reference = expected.sample(frequency * x + 0.25, 0.0).scalar();
                    assert!(
                        (value - reference).abs() < 1e-5,
                        "{output}, f={frequency}, x={x}"
                    );
                    assert!((-1.0..=1.0).contains(&value));
                    if frequency != 0.0 {
                        assert!(
                            (value - actual.sample(x + 1.0 / frequency, 0.0).scalar()).abs() < 1e-5
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pulse_width_endpoints_and_invalid_domains_are_handled() {
        let program = StarfishProgram::from_source(WAVEFORMS).unwrap();
        let mut bindings = ParameterBindings::defaults(&program);
        for (width, expected) in [(0.0, -1.0), (1.0, 1.0)] {
            bindings.0[2] = width;
            let mut evaluator = Evaluator::new(&program, &bindings, "pulses").unwrap();
            for x in [-1.0, -0.125, -f32::MIN_POSITIVE, 0.0, 0.5, 1.0] {
                assert_eq!(evaluator.sample(x, 0.0).scalar(), expected);
            }
        }
        for width in [-0.1, 1.1] {
            bindings.0[2] = width;
            let error = match Evaluator::new(&program, &bindings, "image") {
                Err(error) => error,
                Ok(_) => panic!("accepted pulse width {width}"),
            };
            assert!(error.contains("pulse width"));
            // An unrelated intermediate remains inspectable.
            assert!(Evaluator::new(&program, &bindings, "triangular").is_ok());
        }
        let computed =
            StarfishProgram::from_source(&WAVEFORMS.replace("phase, width)", "phase, 2 * width)"))
                .unwrap();
        bindings.0[2] = 0.75;
        assert!(Evaluator::new(&computed, &bindings, "pulses").is_err());
    }

    #[test]
    fn sine_and_binding_changes_have_known_values() {
        let program = StarfishProgram::sample();
        let mut bindings = ParameterBindings::defaults(&program);
        for (name, value) in [("rotation", 0.0), ("frequency_a", 1.0), ("phase", 0.0)] {
            let index = program
                .parameters
                .iter()
                .position(|p| p.id == name)
                .unwrap();
            bindings.0[index] = value;
        }
        let mut evaluator = Evaluator::new(&program, &bindings, "pattern_a").unwrap();
        assert!((evaluator.sample(0.25, 0.0).scalar() - 1.0).abs() < 1e-6);
        let frequency = program
            .parameters
            .iter()
            .position(|p| p.id == "frequency_a")
            .unwrap();
        bindings.0[frequency] = 2.0;
        let mut evaluator = Evaluator::new(&program, &bindings, "pattern_a").unwrap();
        assert!(evaluator.sample(0.25, 0.0).scalar().abs() < 1e-6);
    }

    #[test]
    fn shared_rotation_moves_wave_and_ring_together() {
        let program = StarfishProgram::sample();
        let mut base = ParameterBindings::defaults(&program);
        let rotation = program
            .parameters
            .iter()
            .position(|p| p.id == "rotation")
            .unwrap();
        base.0[rotation] = 0.0;
        let mut rotated = base.clone();
        rotated.0[rotation] = 90.0_f32.to_radians();
        for output in ["pattern_a", "pattern_b", "radial", "field"] {
            let expected = Evaluator::new(&program, &base, output)
                .unwrap()
                .sample(0.37, 0.21)
                .scalar();
            let actual = Evaluator::new(&program, &rotated, output)
                .unwrap()
                .sample(0.21, -0.37)
                .scalar();
            assert!(
                (actual - expected).abs() < 1e-5,
                "{output}: {actual} != {expected}"
            );
        }
    }
}
