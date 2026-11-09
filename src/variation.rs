//! Pure local exploration of an ordinary saved program, including handwritten sources.

use std::{collections::HashMap, ops::Range};

use crate::{
    arrangement::{self, Plan},
    evaluate::Evaluator,
    program::{ParameterBindings, StarfishProgram, Type},
    source::{self, Decl, Expr},
};

pub const VERSION: &str = "starfish-variations-v2";

pub fn proposal_seed(batch: u64, index: usize, attempt: u64, settings: Settings) -> u64 {
    let seed = batch
        .wrapping_add((index as u64).wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add(attempt.wrapping_mul(0xd1b54a32d192ed03));
    if settings.composition {
        (seed & !7) | batch.wrapping_add(index as u64) % arrangement::Kind::ALL.len() as u64
    } else {
        seed
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub source: String,
    pub output: String,
}

impl Snapshot {
    pub fn capture(
        program: &StarfishProgram,
        bindings: &ParameterBindings,
        output: &str,
    ) -> Result<Self, String> {
        Evaluator::new(program, bindings, output)?;
        let mut edits = Vec::new();
        let mut index = 0;
        for decl in &program.hir {
            if let Decl::Parameter(parameter) = decl {
                let value = bindings.0[index];
                if value.to_bits() != parameter.initial.to_bits() {
                    edits.push((parameter.initial_span.clone(), literal(value, parameter.ty)));
                }
                index += 1;
            }
        }
        Ok(Self {
            source: edit(&program.source, edits),
            output: output.to_owned(),
        })
    }

    pub fn program(&self) -> Result<StarfishProgram, String> {
        let program = StarfishProgram::from_source(&self.source)?;
        Evaluator::new(
            &program,
            &ParameterBindings::defaults(&program),
            &self.output,
        )?;
        Ok(program)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub gesture: bool,
    pub texture: bool,
    pub palette: bool,
    pub composition: bool,
    pub strength: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gesture: false,
            texture: true,
            palette: false,
            composition: false,
            strength: 0.12,
        }
    }
}

impl Settings {
    pub fn mask(self) -> u8 {
        u8::from(self.gesture)
            | u8::from(self.texture) << 1
            | u8::from(self.palette) << 2
            | u8::from(self.composition) << 3
    }

    pub fn from_mask(mask: u8, strength: f32) -> Result<Self, String> {
        if mask == 0 || mask > 15 || !strength.is_finite() || !(0.01..=0.5).contains(&strength) {
            return Err(
                "Choose at least one feature to vary and an amount between 0.01 and 0.5".into(),
            );
        }
        Ok(Self {
            gesture: mask & 1 != 0,
            texture: mask & 2 != 0,
            palette: mask & 4 != 0,
            composition: mask & 8 != 0,
            strength,
        })
    }
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut value = self.0;
        value = (value ^ value >> 30).wrapping_mul(0xbf58476d1ce4e5b9);
        value = (value ^ value >> 27).wrapping_mul(0x94d049bb133111eb);
        value ^ value >> 31
    }
    fn signed(&mut self) -> f32 {
        ((self.next() >> 40) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    }
}

fn group(name: &str) -> u8 {
    if name == "rotation"
        || name == "crossing"
        || name == "spread"
        || name == "bend"
        || name.starts_with("curve_")
        || name.starts_with("start_")
        || name.starts_with("end_")
        || name.starts_with("handle_")
    {
        1
    } else if name.starts_with("weight_") || name.ends_with("_strength") {
        8
    } else {
        2
    }
}

fn shared_key(name: &str) -> &str {
    name.strip_prefix("curve_")
        .and_then(|rest| rest.split_once('_'))
        .filter(|(index, _)| index.bytes().all(|b| b.is_ascii_digit()))
        .map(|(_, point)| point)
        .unwrap_or(name)
}

fn names(expr: &Expr, output: &mut Vec<String>) {
    match expr {
        Expr::Name(name) if !output.contains(name) => output.push(name.clone()),
        Expr::UnaryMinus(a) => names(a, output),
        Expr::Binary(_, a, b) => {
            names(a, output);
            names(b, output);
        }
        Expr::Call(_, args) => {
            for arg in args {
                names(arg, output);
            }
        }
        _ => {}
    }
}

/// All edits are constrained to declarations or function tokens. Coordinates and
/// components stay ordinary source; locked groups are copied byte for byte.
pub fn propose(parent: &Snapshot, settings: Settings, seed: u64) -> Result<Snapshot, String> {
    Settings::from_mask(settings.mask(), settings.strength)?;
    let unpacked = arrangement::unpack(parent)?;
    let base = &unpacked.base;
    let program = base.program()?;
    let structural = arrangement::supports(&program, &base.output);
    let mut random = Random(seed ^ 0x766172696174696f);
    let mut edits = Vec::new();
    let mut shared = HashMap::new();
    let palette_shift = std::array::from_fn::<_, 3, _>(|_| random.signed() * settings.strength);
    for decl in &program.hir {
        match decl {
            Decl::Parameter(parameter)
                if settings.mask() & group(&parameter.name) != 0
                    && !(structural && group(&parameter.name) == 8) =>
            {
                let key = if parameter.name.starts_with("scale_") {
                    "scale"
                } else {
                    shared_key(&parameter.name)
                };
                let delta = *shared
                    .entry(key.to_owned())
                    .or_insert_with(|| random.signed());
                let span = parameter.max - parameter.min;
                let value =
                    if parameter.name.starts_with("scale_") || parameter.ty == Type::Frequency {
                        parameter.initial * (1.0 + delta * settings.strength * 2.0)
                    } else {
                        parameter.initial + delta * span * settings.strength
                    };
                let value = value.clamp(parameter.min, parameter.max);
                edits.push((parameter.initial_span.clone(), literal(value, parameter.ty)));
            }
            Decl::Palette(palette) if settings.palette => {
                for (color, span) in palette.stops.iter().zip(&palette.stop_spans) {
                    let [r, g, b] =
                        std::array::from_fn(|i| (color[i] + palette_shift[i]).clamp(0.0, 1.0));
                    edits.push((span.clone(), format!("rgb({r}, {g}, {b})")));
                }
            }
            Decl::Definition(definition)
                if settings.composition && !structural && definition.name == "composition" =>
            {
                let mut inputs = program
                    .items
                    .iter()
                    .filter(|item| {
                        item.ty == Type::FieldScalar && item.symbol.starts_with("layer_")
                    })
                    .map(|item| item.symbol.clone())
                    .collect::<Vec<_>>();
                if inputs.len() < 2 {
                    inputs.clear();
                    names(&definition.expr, &mut inputs);
                    inputs.retain(|name| {
                        program
                            .items
                            .iter()
                            .any(|item| item.symbol == *name && item.ty == Type::FieldScalar)
                    });
                }
                if inputs.len() >= 2 {
                    let operation = random.next() % 3;
                    let first = &inputs[0];
                    let second = &inputs[1];
                    let rest = inputs[2..].join(" + ");
                    let rest = if rest.is_empty() { "0" } else { &rest };
                    let expression = match operation {
                        0 => inputs.join(" + "),
                        1 => format!("{first} * {second} + 0.65 * ({rest})"),
                        _ => format!("({first} + {second}) * (0.6 + {rest})"),
                    };
                    edits.push((
                        definition.expression_span.clone(),
                        format!("tanh({expression})"),
                    ));
                }
            }
            _ => {}
        }
    }
    if settings.texture {
        for (span, function) in source::call_spans(&base.source, &["wave", "triangle", "saw"])? {
            if random.signed().abs() < settings.strength * 1.5
                && !edits
                    .iter()
                    .any(|(range, _)| range.start <= span.start && range.end >= span.end)
            {
                let options: Vec<_> = ["wave", "triangle", "saw"]
                    .into_iter()
                    .filter(|name| *name != function)
                    .collect();
                edits.push((span, options[(random.next() % 2) as usize].to_owned()));
            }
        }
    }
    let source = edit(&base.source, edits);
    let source = if source == base.source && !(structural && settings.composition) {
        source
    } else {
        format!(
            "// variation policy: {VERSION}; seed: {seed}; groups: {}; amount: {}\n// Current defaults are authoritative; earlier generator comments describe ancestry.\n{source}",
            settings.mask(),
            settings.strength
        )
    };
    let mut snapshot = Snapshot {
        source,
        output: base.output.clone(),
    };
    if structural && settings.composition {
        snapshot = arrangement::compose(&snapshot, &Plan::from_seed(seed))?;
    } else if let Some(plan) = unpacked.plan {
        snapshot = arrangement::compose(&snapshot, &plan)?;
    }
    snapshot.output = parent.output.clone();
    snapshot.program()?;
    Ok(snapshot)
}

fn literal(value: f32, ty: Type) -> String {
    if ty == Type::Angle {
        format!("{value}rad")
    } else {
        value.to_string()
    }
}

fn edit(source: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(range, _)| range.start);
    let mut result = source.to_owned();
    for (range, replacement) in edits.into_iter().rev() {
        result.replace_range(range, &replacement);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        render::{self, RenderRequest},
        source::Decl,
    };

    #[test]
    fn snapshots_round_trip_live_bindings_exactly_without_touching_comments_or_ids() {
        let source = include_str!("../programs/curve_textures.sf");
        let source = source.replace(
            "param frequency:",
            "// ☃ frequency = 123; is a comment\n    param frequency @id(\"cycles-control\"):",
        );
        let program = StarfishProgram::from_source(&source).unwrap();
        let mut bindings = ParameterBindings::defaults(&program);
        for (index, parameter) in program.parameters.iter().enumerate() {
            bindings.0[index] = parameter.min + (parameter.max - parameter.min) * 0.373;
        }
        let snapshot = Snapshot::capture(&program, &bindings, "braiding").unwrap();
        assert!(
            snapshot
                .source
                .contains("// ☃ frequency = 123; is a comment")
        );
        assert!(snapshot.source.contains("@id(\"cycles-control\")"));
        let reloaded = snapshot.program().unwrap();
        assert_eq!(
            bindings.0.iter().map(|n| n.to_bits()).collect::<Vec<_>>(),
            ParameterBindings::defaults(&reloaded)
                .0
                .iter()
                .map(|n| n.to_bits())
                .collect::<Vec<_>>()
        );
        let image = |program: &StarfishProgram, bindings: &ParameterBindings| {
            render::render(
                program,
                bindings,
                RenderRequest {
                    output: "braiding",
                    width: 40,
                    height: 40,
                },
            )
            .unwrap()
            .rgba
        };
        assert_eq!(
            image(&program, &bindings),
            image(&reloaded, &ParameterBindings::defaults(&reloaded))
        );
        assert_eq!(program.source, source);
        assert_eq!(snapshot.output, "braiding");
        assert!(Snapshot::capture(&program, &ParameterBindings(vec![]), "image").is_err());
        let mut invalid = bindings.clone();
        invalid.0[0] = f32::NAN;
        assert!(Snapshot::capture(&program, &invalid, "image").is_err());
    }

    #[test]
    fn variations_reproduce_and_leave_locked_features_intact() {
        let program =
            StarfishProgram::from_source(&crate::generate::comparison_pair(42, 0).a.source())
                .unwrap();
        let parent = Snapshot::capture(
            &program,
            &ParameterBindings::defaults(&program),
            &program.output,
        )
        .unwrap();
        for mask in [1, 2, 4, 8, 15] {
            let settings = Settings::from_mask(mask, 0.12).unwrap();
            let child = propose(&parent, settings, 123).unwrap();
            assert_eq!(child, propose(&parent, settings, 123).unwrap());
            let changed = child.program().unwrap();
            for (before, after) in program.hir.iter().zip(&changed.hir) {
                match (before, after) {
                    (Decl::Parameter(before), Decl::Parameter(after))
                        if settings.mask() & group(&before.name) == 0 =>
                    {
                        assert_eq!(
                            before.initial.to_bits(),
                            after.initial.to_bits(),
                            "{}",
                            before.name
                        );
                    }
                    (Decl::Palette(before), Decl::Palette(after)) if !settings.palette => {
                        assert_eq!(before.stops, after.stops)
                    }
                    (Decl::Definition(before), Decl::Definition(after))
                        if !settings.composition && before.name == "composition" =>
                    {
                        assert_eq!(before.expression, after.expression);
                    }
                    _ => {}
                }
            }
            assert_ne!(parent.source, child.source);
            assert_eq!(parent.output, child.output);
        }
        let none = Settings {
            texture: false,
            ..Settings::default()
        };
        assert!(propose(&parent, none, 1).is_err());
        assert!(Settings::from_mask(1, f32::NAN).is_err());
    }
}
