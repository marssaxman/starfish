//! The authored sample and parameter declarations. No UI or rendering dependencies.

pub const NODE_COUNT: usize = 8;
pub const PARAMETER_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Coordinates,
    Scalar,
    Color,
}

impl ValueType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Coordinates => "Coordinates",
            Self::Scalar => "Scalar",
            Self::Color => "Color",
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum ParameterId {
    Rotation,
    FrequencyA,
    FrequencyB,
    RadialFrequency,
    InterferenceStrength,
    RadialStrength,
    Phase,
}

pub struct Parameter {
    pub id: ParameterId,
    pub name: &'static str,
    pub initial: f32,
    pub min: f32,
    pub max: f32,
    pub step: f64,
    pub suffix: &'static str,
    pub description: &'static str,
}

/// Runtime state is separate from the immutable program declarations.
#[derive(Clone)]
pub struct ParameterBindings(pub [f32; PARAMETER_COUNT]);

impl ParameterBindings {
    pub fn defaults(program: &StarfishProgram) -> Self {
        let mut values = [0.0; PARAMETER_COUNT];
        for parameter in &program.parameters {
            values[parameter.id as usize] = parameter.initial;
        }
        Self(values)
    }

    pub fn get(&self, id: ParameterId) -> f32 {
        self.0[id as usize]
    }

    pub fn get_mut(&mut self, id: ParameterId) -> &mut f32 {
        &mut self.0[id as usize]
    }
}

#[derive(Clone, Copy)]
pub enum Operation {
    Coordinates,
    Rotate,
    Sine {
        direction: [f32; 2],
        frequency: ParameterId,
    },
    Multiply,
    Radial {
        center: [f32; 2],
    },
    Modulate,
    Palette,
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Self::Coordinates => "normalized coordinates",
            Self::Rotate => "rotate shared frame",
            Self::Sine { .. } => "sine wave",
            Self::Multiply => "multiply patterns",
            Self::Radial { .. } => "radial sine",
            Self::Modulate => "weighted sum / tanh",
            Self::Palette => "interpolate palette",
        }
    }
}

pub struct Node {
    pub id: &'static str,
    pub name: &'static str,
    pub operation: Operation,
    pub value_type: ValueType,
    /// Indices of earlier nodes in this small, topologically ordered sample.
    /// Public output selection always uses `id`, never these internal indices.
    pub inputs: &'static [usize],
    pub expression: &'static str,
    pub description: &'static str,
}

pub struct StarfishProgram {
    pub name: &'static str,
    pub parameters: [Parameter; PARAMETER_COUNT],
    pub nodes: [Node; NODE_COUNT],
    pub output: &'static str,
}

impl StarfishProgram {
    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.node_index(id).map(|index| &self.nodes[index])
    }

    pub fn sample() -> Self {
        use ParameterId::*;
        use ValueType::*;

        Self {
            name: "Shared tides",
            output: "color",
            parameters: [
                Parameter {
                    id: Rotation,
                    name: "Rotation",
                    initial: 24.0,
                    min: -180.0,
                    max: 180.0,
                    step: 0.5,
                    suffix: "°",
                    description: "Rotates Pattern A, Pattern B, and the radial center together.",
                },
                Parameter {
                    id: FrequencyA,
                    name: "Frequency A",
                    initial: 6.0,
                    min: 0.5,
                    max: 20.0,
                    step: 0.05,
                    suffix: "",
                    description: "Cycles per coordinate unit in Pattern A.",
                },
                Parameter {
                    id: FrequencyB,
                    name: "Frequency B",
                    initial: 9.0,
                    min: 0.5,
                    max: 20.0,
                    step: 0.05,
                    suffix: "",
                    description: "Cycles per coordinate unit in Pattern B.",
                },
                Parameter {
                    id: RadialFrequency,
                    name: "Radial frequency",
                    initial: 1.8,
                    min: 0.2,
                    max: 8.0,
                    step: 0.05,
                    suffix: "",
                    description: "Controls the spacing of the broad concentric rings.",
                },
                Parameter {
                    id: InterferenceStrength,
                    name: "Interference strength",
                    initial: 0.65,
                    min: 0.0,
                    max: 2.0,
                    step: 0.01,
                    suffix: "",
                    description: "Contribution of the multiplied wave patterns to Field.",
                },
                Parameter {
                    id: RadialStrength,
                    name: "Radial strength",
                    initial: 1.1,
                    min: 0.0,
                    max: 2.0,
                    step: 0.01,
                    suffix: "",
                    description: "Contribution of the radial rings to Field.",
                },
                Parameter {
                    id: Phase,
                    name: "Phase",
                    initial: 0.0,
                    min: -std::f32::consts::PI,
                    max: std::f32::consts::PI,
                    step: 0.01,
                    suffix: " rad",
                    description: "Shared phase offset for both waves and the radial rings.",
                },
            ],
            nodes: [
                Node {
                    id: "xy",
                    name: "Coordinates",
                    operation: Operation::Coordinates,
                    value_type: Coordinates,
                    inputs: &[],
                    expression: "(x, y) ∈ [−1, 1]²",
                    description: "Continuous coordinates sampled at pixel centers; y points upward.",
                },
                Node {
                    id: "rotated",
                    name: "Shared frame",
                    operation: Operation::Rotate,
                    value_type: Coordinates,
                    inputs: &[0],
                    expression: "x′ = cos(θ)x − sin(θ)y\ny′ = sin(θ)x + cos(θ)y",
                    description: "One rotation feeds both patterns and the radial field.",
                },
                Node {
                    id: "pattern_a",
                    name: "Pattern A",
                    operation: Operation::Sine {
                        direction: [1.0, 0.0],
                        frequency: FrequencyA,
                    },
                    value_type: Scalar,
                    inputs: &[1],
                    expression: "sin(τ × frequency A × x′ + phase)",
                    description: "Fine stripes along the shared frame's first axis.",
                },
                Node {
                    id: "pattern_b",
                    name: "Pattern B",
                    operation: Operation::Sine {
                        direction: [0.6, 0.8],
                        frequency: FrequencyB,
                    },
                    value_type: Scalar,
                    inputs: &[1],
                    expression: "sin(τ × frequency B\n    × (0.6x′ + 0.8y′) + phase)",
                    description: "A second family of stripes, oblique to Pattern A in the same frame.",
                },
                Node {
                    id: "interference",
                    name: "Interference",
                    operation: Operation::Multiply,
                    value_type: Scalar,
                    inputs: &[2, 3],
                    expression: "Pattern A × Pattern B",
                    description: "The two waves multiply into a fine woven structure.",
                },
                Node {
                    id: "radial",
                    name: "Radial",
                    operation: Operation::Radial {
                        center: [0.25, -0.15],
                    },
                    value_type: Scalar,
                    inputs: &[1],
                    expression: "r = length((x′, y′) − (0.25, −0.15))\nsin(τ × radial frequency × r + phase)",
                    description: "Broad rings around an offset center. The center follows the shared rotation.",
                },
                Node {
                    id: "field",
                    name: "Field",
                    operation: Operation::Modulate,
                    value_type: Scalar,
                    inputs: &[4, 5],
                    expression: "tanh(interference strength × Interference\n   + radial strength × Radial)",
                    description: "Balances the fine interference against the broad rings, with a smooth compression into [−1, 1].",
                },
                Node {
                    id: "color",
                    name: "Palette",
                    operation: Operation::Palette,
                    value_type: Color,
                    inputs: &[6],
                    expression: "palette((Field + 1) / 2)",
                    description: "Maps the field through ink, teal, pale sand, and warm coral. This is the declared final output.",
                },
            ],
        }
    }
}
