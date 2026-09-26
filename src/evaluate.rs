//! A tiny CPU evaluator for the trusted, topologically ordered sample graph.

use crate::program::{NODE_COUNT, Operation, ParameterBindings, ParameterId, StarfishProgram};
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Coordinates([f32; 2]),
    Scalar(f32),
    Color([f32; 3]),
}

impl Value {
    fn scalar(self) -> f32 {
        match self {
            Self::Scalar(value) => value,
            _ => unreachable!("the authored graph supplies a scalar here"),
        }
    }

    fn coordinates(self) -> [f32; 2] {
        match self {
            Self::Coordinates(value) => value,
            _ => unreachable!("the authored graph supplies coordinates here"),
        }
    }
}

pub struct Evaluator<'a> {
    program: &'a StarfishProgram,
    bindings: &'a ParameterBindings,
    output_index: usize,
    rotation: (f32, f32),
}

impl<'a> Evaluator<'a> {
    pub fn new(
        program: &'a StarfishProgram,
        bindings: &'a ParameterBindings,
        output: &str,
    ) -> Result<Self, String> {
        let output_index = program
            .node_index(output)
            .ok_or_else(|| format!("Unknown output: {output}"))?;
        Ok(Self {
            program,
            bindings,
            output_index,
            rotation: bindings.get(ParameterId::Rotation).to_radians().sin_cos(),
        })
    }

    pub fn sample(&self, x: f32, y: f32) -> Value {
        use ParameterId::*;
        let parameter = |id| self.bindings.get(id);
        // Fixed stack storage: each shared node is evaluated at most once per pixel.
        let mut values = [Value::Scalar(0.0); NODE_COUNT];
        for (index, node) in self.program.nodes[..=self.output_index].iter().enumerate() {
            let input = |slot| values[node.inputs[slot]];
            values[index] = match node.operation {
                Operation::Coordinates => Value::Coordinates([x, y]),
                Operation::Rotate => {
                    let [x, y] = input(0).coordinates();
                    let (sin, cos) = self.rotation;
                    Value::Coordinates([cos * x - sin * y, sin * x + cos * y])
                }
                Operation::Sine {
                    direction,
                    frequency,
                } => {
                    let [x, y] = input(0).coordinates();
                    let position = direction[0] * x + direction[1] * y;
                    Value::Scalar((TAU * parameter(frequency) * position + parameter(Phase)).sin())
                }
                Operation::Multiply => Value::Scalar(input(0).scalar() * input(1).scalar()),
                Operation::Radial { center } => {
                    let [x, y] = input(0).coordinates();
                    let dx = x - center[0];
                    let dy = y - center[1];
                    let radius = (dx * dx + dy * dy).sqrt();
                    Value::Scalar(
                        (TAU * parameter(RadialFrequency) * radius + parameter(Phase)).sin(),
                    )
                }
                Operation::Modulate => Value::Scalar(
                    (parameter(InterferenceStrength) * input(0).scalar()
                        + parameter(RadialStrength) * input(1).scalar())
                    .tanh(),
                ),
                Operation::Palette => Value::Color(palette(input(0).scalar())),
            };
        }
        values[self.output_index]
    }
}

fn palette(field: f32) -> [f32; 3] {
    const STOPS: [[f32; 3]; 5] = [
        [0.035, 0.065, 0.13],
        [0.055, 0.32, 0.39],
        [0.36, 0.69, 0.65],
        [0.95, 0.86, 0.62],
        [0.83, 0.27, 0.20],
    ];
    let position = ((field + 1.0) * 0.5).clamp(0.0, 1.0) * 4.0;
    let index = (position.floor() as usize).min(3);
    let fraction = position - index as f32;
    // Smooth interpolation keeps parameter sweeps continuous at palette stops.
    let fraction = fraction * fraction * (3.0 - 2.0 * fraction);
    std::array::from_fn(|channel| {
        STOPS[index][channel] * (1.0 - fraction) + STOPS[index + 1][channel] * fraction
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sine_result_and_changed_binding() {
        let program = StarfishProgram::sample();
        let mut bindings = ParameterBindings::defaults(&program);
        *bindings.get_mut(ParameterId::Rotation) = 0.0;
        *bindings.get_mut(ParameterId::FrequencyA) = 1.0;
        let sample = |bindings: &ParameterBindings| {
            Evaluator::new(&program, bindings, "pattern_a")
                .unwrap()
                .sample(0.25, 0.0)
                .scalar()
        };
        assert!((sample(&bindings) - 1.0).abs() < 1e-6);
        *bindings.get_mut(ParameterId::FrequencyA) = 2.0;
        assert!(sample(&bindings).abs() < 1e-6);
    }

    #[test]
    fn deterministic_evaluation() {
        let program = StarfishProgram::sample();
        let bindings = ParameterBindings::defaults(&program);
        let evaluator = Evaluator::new(&program, &bindings, program.output).unwrap();
        assert_eq!(evaluator.sample(0.17, -0.38), evaluator.sample(0.17, -0.38));
    }

    #[test]
    fn rotation_is_shared_by_both_patterns_and_radial() {
        let program = StarfishProgram::sample();
        let mut base = ParameterBindings::defaults(&program);
        *base.get_mut(ParameterId::Rotation) = 0.0;
        let mut rotated = base.clone();
        *rotated.get_mut(ParameterId::Rotation) = 90.0;
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

    #[test]
    fn zero_strengths_produce_a_neutral_field() {
        let program = StarfishProgram::sample();
        let mut bindings = ParameterBindings::defaults(&program);
        *bindings.get_mut(ParameterId::InterferenceStrength) = 0.0;
        *bindings.get_mut(ParameterId::RadialStrength) = 0.0;
        assert_eq!(
            Evaluator::new(&program, &bindings, "field")
                .unwrap()
                .sample(0.23, -0.45),
            Value::Scalar(0.0)
        );
    }
}
