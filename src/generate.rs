//! Pure, versioned synthesis: seed -> design plan -> self-contained source.
//! Rendering, files, and Studio are clients of this module.

use std::fmt::Write;

pub const GENERATOR_VERSION: &str = "starfish-curves-v3";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Coordinated,
    Independent,
}

impl Condition {
    pub fn name(self) -> &'static str {
        match self {
            Self::Coordinated => "coordinated",
            Self::Independent => "independent",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Texture {
    Etched,
    Braided,
    Rippled,
}

impl Texture {
    fn name(self) -> &'static str {
        match self {
            Self::Etched => "etched",
            Self::Braided => "braided",
            Self::Rippled => "rippled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waveform {
    Sine,
    Triangle,
    Saw,
    Pulse,
    Harmonic,
    Modulated,
}

impl Waveform {
    const ALL: [Self; 6] = [
        Self::Sine,
        Self::Triangle,
        Self::Saw,
        Self::Pulse,
        Self::Harmonic,
        Self::Modulated,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::Saw => "saw",
            Self::Pulse => "pulse",
            Self::Harmonic => "harmonic",
            Self::Modulated => "modulated",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Orientation {
    Relative(f64),
    Independent(f64),
}

#[derive(Clone, Copy, Debug)]
enum Scale {
    Ratio(u32, u32),
    Independent(f64),
}

#[derive(Clone, Copy, Debug)]
enum Placement {
    Origin,
    Anchor,
    Mirror,
    Independent([f64; 2]),
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Shared,
    Offset(f64),
    Independent(f64),
}

#[derive(Clone, Debug)]
struct Motif {
    points: [[f64; 2]; 4],
    family_variation: Option<f64>,
    waveform: Waveform,
    detail: f64,
    orientation: Orientation,
    scale: Scale,
    placement: Placement,
    phase: Phase,
    weight: f64,
}

#[derive(Clone, Copy, Debug)]
enum Composition {
    Weave,
    Layers,
    Modulate,
}

impl Composition {
    fn name(self) -> &'static str {
        match self {
            Self::Weave => "weave",
            Self::Layers => "layers",
            Self::Modulate => "modulate",
        }
    }
}

#[derive(Clone, Debug)]
pub struct DesignPlan {
    pub seed: u64,
    pub condition: Condition,
    rotation: f64,
    frequency: f64,
    phase: f64,
    anchor: [f64; 2],
    palette: usize,
    composition: Composition,
    gesture: [[f64; 2]; 4],
    texture: Texture,
    bend: f64,
    texture_amount: f64,
    texture_scale: f64,
    motifs: Vec<Motif>,
}

pub struct ComparisonPair {
    pub seed: u64,
    pub a: DesignPlan,
    pub b: DesignPlan,
}

/// Separate random streams keep spatial plans, profiles, ablation, and A/B order independent.
/// Wrapping integer arithmetic and fixed decimal emission keep synthesis reproducible.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
        value ^ (value >> 31)
    }

    fn choose(&mut self, count: usize) -> usize {
        (self.next() % count as u64) as usize
    }

    fn between(&mut self, min: f64, max: f64) -> f64 {
        let unit = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        min + (max - min) * unit
    }
}

pub fn comparison_pair(batch_seed: u64, index: usize) -> ComparisonPair {
    let seed = Random(batch_seed.wrapping_add(index as u64)).next();
    let coordinated = DesignPlan::coordinated(seed);
    let independent = coordinated.ablate();
    let (a, b) = if Random(seed ^ 0x61625f6f72646572).choose(2) == 0 {
        (coordinated, independent)
    } else {
        (independent, coordinated)
    };
    ComparisonPair { seed, a, b }
}

impl DesignPlan {
    fn coordinated(seed: u64) -> Self {
        let mut random = Random(seed);
        let rotation = random.between(-45.0, 45.0);
        let frequency = random.between(1.4, 2.6);
        let phase = random.between(-90.0, 90.0);
        let anchor = [random.between(0.25, 0.6), random.between(-0.3, 0.3)];
        let palette = random.choose(PALETTES.len());
        let composition = [
            Composition::Weave,
            Composition::Layers,
            Composition::Modulate,
        ][random.choose(3)];
        let crossing = [30.0, 60.0, 90.0][random.choose(3)];
        let gesture = random_gesture(&mut random);
        let count = 2 + random.choose(3);
        let texture = [Texture::Etched, Texture::Braided, Texture::Rippled][random.choose(3)];
        let bend = random.between(0.6, 1.3);
        let texture_amount = random.between(0.3, 0.65);
        let texture_scale = random.between(2.5, 4.5);
        let mut motifs = Vec::new();
        for index in 0..count {
            let variation = [0.0, 0.18, -0.18, 0.32][index];
            let mut points = gesture;
            // A family retains the parent gesture while opening or tightening its bend.
            points[1][1] += variation;
            points[2][1] -= variation;
            motifs.push(Motif {
                points,
                family_variation: Some(variation),
                waveform: Waveform::Sine,
                detail: 0.5,
                orientation: Orientation::Relative([0.0, crossing, -crossing, 180.0][index]),
                scale: [
                    Scale::Ratio(1, 1),
                    Scale::Ratio(3, 2),
                    Scale::Ratio(1, 2),
                    Scale::Ratio(2, 1),
                ][index],
                placement: [
                    Placement::Origin,
                    Placement::Anchor,
                    Placement::Mirror,
                    Placement::Origin,
                ][index],
                phase: if index == 0 {
                    Phase::Shared
                } else {
                    Phase::Offset(90.0 * index as f64)
                },
                weight: random.between(0.55, 0.9),
            });
        }
        // Waveform choices have their own stream, separate from curve planning.
        let mut profiles = Random(seed ^ 0x77617665666f726d);
        let first = profiles.choose(Waveform::ALL.len());
        for (index, motif) in motifs.iter_mut().enumerate() {
            motif.waveform = Waveform::ALL[(first + index) % Waveform::ALL.len()];
            motif.detail = profiles.between(0.2, 0.8);
        }
        Self {
            seed,
            condition: Condition::Coordinated,
            rotation,
            frequency,
            phase,
            anchor,
            palette,
            composition,
            gesture,
            texture,
            bend,
            texture_amount,
            texture_scale,
            motifs,
        }
    }

    fn ablate(&self) -> Self {
        let mut plan = self.clone();
        plan.condition = Condition::Independent;
        let mut random = Random(self.seed ^ 0x61626c6174696f6e);
        for motif in &mut plan.motifs {
            motif.orientation = Orientation::Independent(random.between(-180.0, 180.0));
            let scale = match motif.scale {
                Scale::Ratio(a, b) => a as f64 / b as f64,
                Scale::Independent(value) => value,
            };
            // Retain each instance's broad scale band while dropping exact ratios.
            motif.scale = Scale::Independent(random.between(scale * 0.8, scale * 1.2));
            motif.placement =
                Placement::Independent([random.between(-0.6, 0.6), random.between(-0.6, 0.6)]);
            motif.phase = Phase::Independent(random.between(-180.0, 180.0));
            motif.points = random_gesture(&mut random);
            motif.family_variation = None;
        }
        plan
    }

    /// The emitted comments record the high-level choices; the declarations are
    /// their executable lowering. No generator is needed to load this artifact.
    pub fn source(&self) -> String {
        let mut source = format!(
            r#"// generator: {GENERATOR_VERSION}
// pair seed: {}
// condition: {}
// composition: {}
// texture: {}
// family gesture: {:?}
// anchor: ({:.4}, {:.4}); palette: {}
// All motifs share scene rotation, base frequency, and a phase control.
// Waveform kinds and their detail controls are identical within each pair.
// Curves bend fields throughout the image; there are no coverage masks.
// The independent condition removes constraints on their relative choices.
starfish 1;
program "Study {}" @id("study-{}") {{
    param rotation: Angle = {:.4}deg in -180deg..180deg step 0.5deg;
    param frequency: Frequency = {:.4} in 0.2..8 step 0.05;
    param phase: Angle = {:.4}deg in -180deg..180deg step 0.5deg;
    param bend: Scalar = {:.4} in 0..1.8 step 0.01;
    param texture_amount: Scalar = {:.4} in 0..1 step 0.01;
    param texture_scale: Scalar = {:.4} in 1..6 step 0.05;
    frame shared = rotate(world, rotation);
    axis primary = vec2(1, 0);
    axis sideways = vec2(0, 1);

    component harmonic(coordinate: Field<Scalar>, frequency: Frequency,
                       phase: Angle, detail: Scalar) -> Field<Scalar> =
        0.7 * wave(coordinate, frequency, phase)
        + detail * (0.2 * wave(2 * coordinate, frequency, phase)
                    + 0.1 * wave(3 * coordinate, frequency, phase));
    component modulated(coordinate: Field<Scalar>, frequency: Frequency,
                        phase: Angle, detail: Scalar) -> Field<Scalar> =
        wave(coordinate + 0.14 * detail * wave(0.5 * coordinate, frequency,
                                             phase + 90deg), frequency, phase);
"#,
            self.seed,
            self.condition.name(),
            self.composition.name(),
            self.texture.name(),
            self.gesture,
            self.anchor[0],
            self.anchor[1],
            self.palette,
            self.seed,
            self.seed,
            self.rotation,
            self.frequency,
            self.phase,
            self.bend,
            self.texture_amount,
            self.texture_scale,
        );
        for (index, motif) in self.motifs.iter().enumerate() {
            let angle = match motif.orientation {
                Orientation::Relative(value) | Orientation::Independent(value) => value,
            };
            let scale = match motif.scale {
                Scale::Ratio(numerator, denominator) => numerator as f64 / denominator as f64,
                Scale::Independent(value) => value,
            };
            let center = match motif.placement {
                Placement::Origin => [0.0, 0.0],
                Placement::Anchor => self.anchor,
                Placement::Mirror => [-self.anchor[0], -self.anchor[1]],
                Placement::Independent(value) => value,
            };
            let offset = match motif.phase {
                Phase::Shared => 0.0,
                Phase::Offset(value) | Phase::Independent(value) => value,
            };
            writeln!(source, "\n    // motif {index}: {} curve; orientation {:?}; scale {:?}; placement {:?}; phase {:?}",
                motif.waveform.name(), motif.orientation, motif.scale, motif.placement, motif.phase).unwrap();
            writeln!(
                source,
                "    // family variation: {:?}; control points: {:?}",
                motif.family_variation, motif.points
            )
            .unwrap();
            writeln!(
                source,
                "    param scale_{index}: Scalar = {scale:.4} in 0.25..5 step 0.01;"
            )
            .unwrap();
            writeln!(
                source,
                "    param weight_{index}: Scalar = {:.4} in 0..2 step 0.01;",
                motif.weight
            )
            .unwrap();
            writeln!(
                source,
                "    frame oriented_{index} = rotate(shared, {angle:.4}deg);"
            )
            .unwrap();
            writeln!(
                source,
                "    frame placed_{index} = oriented_{index} - point2({:.4}, {:.4});",
                center[0], center[1]
            )
            .unwrap();
            for (slot, [x, y]) in motif.points.iter().enumerate() {
                writeln!(
                    source,
                    "    param curve_{index}_p{slot}_x: Scalar = {x:.4} in -2..2 step 0.01;"
                )
                .unwrap();
                writeln!(
                    source,
                    "    param curve_{index}_p{slot}_y: Scalar = {y:.4} in -2..2 step 0.01;"
                )
                .unwrap();
            }
            writeln!(source, "    frame curve_{index} = bezier(placed_{index},\n        point2(curve_{index}_p0_x, curve_{index}_p0_y), point2(curve_{index}_p1_x, curve_{index}_p1_y),\n        point2(curve_{index}_p2_x, curve_{index}_p2_y), point2(curve_{index}_p3_x, curve_{index}_p3_y));").unwrap();
            writeln!(
                source,
                "    field along_{index}: Scalar = dot(curve_{index}, primary);"
            )
            .unwrap();
            writeln!(
                source,
                "    field across_{index}: Scalar = dot(curve_{index}, sideways);"
            )
            .unwrap();
            writeln!(
                source,
                "    field distance_{index}: Scalar = abs(across_{index});"
            )
            .unwrap();
            writeln!(source, "    field sweep_{index}: Scalar = dot(placed_{index}, primary) + bend * distance_{index};").unwrap();
            writeln!(
                source,
                "    field coordinate_{index}: Scalar = scale_{index} * sweep_{index};"
            )
            .unwrap();
            // Extend the cross coordinate beyond the curve's endpoints, and give fine
            // lines a slow along-curve drift. Detail remains active across the plane.
            writeln!(source, "    field cross_coordinate_{index}: Scalar = scale_{index} * (distance_{index} + 0.25 * dot(placed_{index}, sideways));").unwrap();
            writeln!(source, "    field grain_{index}: Scalar = wave(texture_scale * cross_coordinate_{index} + 0.18 * wave(along_{index}, frequency, phase), frequency, phase + {offset:.4}deg);").unwrap();
            writeln!(
                source,
                "    param detail_{index}: Scalar = {:.4} in 0..1 step 0.01;",
                motif.detail
            )
            .unwrap();
            let (function, extra) = match motif.waveform {
                Waveform::Sine => ("wave", String::new()),
                Waveform::Triangle => ("triangle", String::new()),
                Waveform::Saw => ("saw", String::new()),
                Waveform::Pulse => {
                    writeln!(
                        source,
                        "    param width_{index}: Scalar = {:.4} in 0..1 step 0.01;",
                        motif.detail
                    )
                    .unwrap();
                    ("pulse", format!(", width_{index}"))
                }
                Waveform::Harmonic | Waveform::Modulated => {
                    (motif.waveform.name(), format!(", detail_{index}"))
                }
            };
            writeln!(source, "    field motif_{index}: Scalar = {function}(coordinate_{index} + 0.05 * texture_amount * grain_{index}, frequency, phase + {offset:.4}deg{extra});").unwrap();
            let texture = match self.texture {
                Texture::Etched => format!(
                    "(0.65 + 0.35 * wave(0.5 * coordinate_{index}, frequency, phase)) * grain_{index}"
                ),
                Texture::Braided => format!(
                    "grain_{index} * wave(2 * coordinate_{index} + 0.16 * grain_{index}, frequency, phase + {offset:.4}deg)"
                ),
                Texture::Rippled => format!(
                    "0.75 * wave(texture_scale * cross_coordinate_{index} + 0.22 * motif_{index}, frequency, phase) + 0.25 * wave(2 * texture_scale * cross_coordinate_{index}, frequency, phase + {offset:.4}deg)"
                ),
            };
            writeln!(source, "    field texture_{index}: Scalar = {texture};").unwrap();
            writeln!(
                source,
                "    field layer_{index}: Scalar = weight_{index} * (motif_{index} + texture_amount * detail_{index} * texture_{index});"
            )
            .unwrap();
        }
        let relatives = (2..self.motifs.len())
            .map(|i| format!("layer_{i}"))
            .collect::<Vec<_>>()
            .join(" + ");
        let relatives = if relatives.is_empty() {
            "0"
        } else {
            &relatives
        };
        let expression = match self.composition {
            Composition::Weave => format!("layer_0 * layer_1 + 0.65 * ({relatives})"),
            Composition::Layers => format!("layer_0 + layer_1 + {relatives}"),
            Composition::Modulate => format!("(layer_0 + layer_1) * (0.6 + {relatives})"),
        };
        writeln!(
            source,
            "\n    field composition: Scalar = tanh({expression});"
        )
        .unwrap();
        source.push_str("    palette ink = smooth [\n");
        for (index, [red, green, blue]) in PALETTES[self.palette].iter().enumerate() {
            writeln!(
                source,
                "        rgb({red:.4}, {green:.4}, {blue:.4}){}",
                if index == 4 { "" } else { "," }
            )
            .unwrap();
        }
        source.push_str("    ];\n    output image: Field<Color> = palette(ink, composition);\n}\n");
        source
    }
}

fn random_gesture(random: &mut Random) -> [[f64; 2]; 4] {
    let ys = [
        [-0.65, -0.65, 0.65, 0.65],
        [-0.1, 0.95, 0.95, -0.1],
        [-0.55, 0.7, -0.7, 0.55],
        [-0.4, -0.15, 0.15, 0.4],
    ][random.choose(4)];
    [
        [-1.25, ys[0] + random.between(-0.12, 0.12)],
        [
            random.between(-0.9, -0.3),
            ys[1] + random.between(-0.12, 0.12),
        ],
        [
            random.between(0.3, 0.9),
            ys[2] + random.between(-0.12, 0.12),
        ],
        [1.25, ys[3] + random.between(-0.12, 0.12)],
    ]
}

const PALETTES: [[[f64; 3]; 5]; 4] = [
    [
        [0.035, 0.065, 0.13],
        [0.055, 0.32, 0.39],
        [0.36, 0.69, 0.65],
        [0.95, 0.86, 0.62],
        [0.83, 0.27, 0.20],
    ],
    [
        [0.06, 0.035, 0.12],
        [0.24, 0.13, 0.37],
        [0.62, 0.34, 0.47],
        [0.91, 0.70, 0.47],
        [0.98, 0.91, 0.73],
    ],
    [
        [0.025, 0.08, 0.09],
        [0.10, 0.24, 0.20],
        [0.38, 0.55, 0.30],
        [0.85, 0.83, 0.48],
        [0.99, 0.96, 0.84],
    ],
    [
        [0.07, 0.05, 0.06],
        [0.29, 0.16, 0.12],
        [0.65, 0.32, 0.18],
        [0.88, 0.62, 0.39],
        [0.98, 0.91, 0.77],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        program::{ParameterBindings, StarfishProgram, Type},
        render::{RenderRequest, render},
    };

    #[test]
    fn version_one_random_stream_has_fixed_values() {
        let mut random = Random(0);
        assert_eq!(random.next(), 0xe220a8397b1dcdaf);
        assert_eq!(random.next(), 0x6e789e6aa1b965f4);
    }

    #[test]
    fn pairs_compile_reproduce_and_preserve_field_budgets() {
        let mut structures = std::collections::HashSet::new();
        let mut orders = std::collections::HashSet::new();
        let mut waveforms = std::collections::HashSet::new();
        let mut textures = std::collections::HashSet::new();
        for seed in (0..64).chain([u64::MAX]) {
            let pair = comparison_pair(seed, 0);
            let repeated = comparison_pair(seed, 0);
            assert_eq!(pair.a.source(), repeated.a.source());
            assert_eq!(pair.b.source(), repeated.b.source());
            assert_ne!(pair.a.condition, pair.b.condition);
            orders.insert(pair.a.condition.name());
            structures.insert((pair.a.composition.name(), pair.a.motifs.len()));
            textures.insert(pair.a.texture.name());
            assert_eq!(pair.a.texture, pair.b.texture);
            assert_eq!(pair.a.bend, pair.b.bend);
            assert_eq!(pair.a.texture_amount, pair.b.texture_amount);
            assert_eq!(pair.a.texture_scale, pair.b.texture_scale);
            for (a, b) in pair.a.motifs.iter().zip(&pair.b.motifs) {
                assert_eq!(a.waveform, b.waveform);
                assert_eq!(a.detail, b.detail);
                assert_eq!(a.weight, b.weight);
                waveforms.insert(a.waveform.name());
            }
            let a = StarfishProgram::from_source(&pair.a.source()).unwrap();
            let b = StarfishProgram::from_source(&pair.b.source()).unwrap();
            let signature = |program: &StarfishProgram| {
                program
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.ty,
                            Type::FieldPoint | Type::FieldScalar | Type::FieldColor
                        )
                    })
                    .map(|node| (node.ty, std::mem::discriminant(&node.operation)))
                    .collect::<Vec<_>>()
            };
            assert_eq!(signature(&a), signature(&b));
            assert_eq!(
                a.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
                b.items.iter().map(|item| &item.id).collect::<Vec<_>>()
            );
            assert_eq!(a.palettes[0].stops, b.palettes[0].stops);
            let preview = |program: &StarfishProgram| {
                render(
                    program,
                    &ParameterBindings::defaults(program),
                    RenderRequest {
                        output: &program.output,
                        width: 24,
                        height: 24,
                    },
                )
                .unwrap()
                .rgba
            };
            let image = preview(&a);
            assert_eq!(
                image,
                preview(&StarfishProgram::from_source(&repeated.a.source()).unwrap())
            );
            assert_ne!(image, preview(&b));
            assert!(image.chunks_exact(4).any(|pixel| pixel != &image[..4]));
        }
        assert_eq!(orders.len(), 2);
        assert_eq!(structures.len(), 9);
        assert_eq!(waveforms.len(), Waveform::ALL.len());
        assert_eq!(textures.len(), 3);
    }

    #[test]
    fn curve_families_preserve_the_parent_gesture_and_have_live_texture_controls() {
        let plan = DesignPlan::coordinated(42);
        let independent = plan.ablate();
        for (motif, other) in plan.motifs.iter().zip(&independent.motifs) {
            let mut expected = plan.gesture;
            let variation = motif.family_variation.unwrap();
            expected[1][1] += variation;
            expected[2][1] -= variation;
            assert_eq!(motif.points, expected);
            assert!(other.family_variation.is_none());
            assert_ne!(motif.points, other.points);
        }
        let program = StarfishProgram::from_source(&plan.source()).unwrap();
        let bindings = ParameterBindings::defaults(&program);
        let guides = crate::evaluate::curve_guides(&program, &bindings, &program.output).unwrap();
        assert_eq!(guides.len(), plan.motifs.len());
        assert!(
            guides
                .iter()
                .all(|guide| (0..4).all(|slot| guide.editable(slot)))
        );
        let preview = |bindings: &ParameterBindings| {
            render(
                &program,
                bindings,
                RenderRequest {
                    output: &program.output,
                    width: 48,
                    height: 48,
                },
            )
            .unwrap()
            .rgba
        };
        let original = preview(&bindings);
        for id in [
            "bend",
            "texture_amount",
            "texture_scale",
            "detail_0",
            "curve_0_p1_y",
        ] {
            let index = program.parameters.iter().position(|p| p.id == id).unwrap();
            let mut changed = bindings.clone();
            changed.0[index] = program.parameters[index].min;
            assert_ne!(original, preview(&changed), "{id} has no visible effect");
        }
    }

    #[test]
    fn generated_motifs_follow_shared_rotation() {
        use crate::evaluate::Evaluator;
        let source = DesignPlan::coordinated(42).source();
        let program = StarfishProgram::from_source(&source).unwrap();
        let rotation = program
            .parameters
            .iter()
            .position(|p| p.id == "rotation")
            .unwrap();
        let mut base = ParameterBindings::defaults(&program);
        base.0[rotation] = 0.0;
        let mut rotated = base.clone();
        rotated.0[rotation] = 90.0_f32.to_radians();
        for item in program
            .items
            .iter()
            .filter(|item| item.ty == Type::FieldScalar)
        {
            let expected = Evaluator::new(&program, &base, &item.id)
                .unwrap()
                .sample(0.37, 0.21);
            let actual = Evaluator::new(&program, &rotated, &item.id)
                .unwrap()
                .sample(0.21, -0.37);
            let (crate::evaluate::Value::Scalar(expected), crate::evaluate::Value::Scalar(actual)) =
                (expected, actual)
            else {
                panic!("scalar field");
            };
            assert!((expected - actual).abs() < 1e-5, "{}", item.id);
        }
    }
}
