//! Deterministic CPU evaluation of a validated, lowered program.

use std::f32::consts::TAU;

use crate::program::{Operation, ParameterBindings, StarfishProgram, Type};

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
        };
        // Uniform values and rotation angles depend on bindings, but not on pixels.
        for &index in &evaluator.order.clone() {
            let node = &program.nodes[index];
            if matches!(node.operation, Operation::Rotate) {
                evaluator.rotations[index] =
                    Some(evaluator.values[node.inputs[1]].scalar().sin_cos());
            }
            if !matches!(
                node.ty,
                Type::FieldScalar | Type::FieldPoint | Type::FieldColor
            ) {
                evaluator.evaluate_node(index, 0.0, 0.0);
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
