//! Explicit spatial relationships over a reusable scalar construction.
//! Plans lower to ordinary components and affine frames in self-contained source.

use crate::{
    program::{DefinitionKind, StarfishProgram, Type},
    source::{self, Decl, Expr},
    variation::Snapshot,
};
use std::collections::HashSet;

const START: &str = "// @starfish-arrangement-v1 ";
const END: &str = "// @starfish-arrangement-end ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mirror,
    Repeat,
    Alternate,
    Radial,
    Echo,
    Broken,
}

impl Kind {
    pub const ALL: [Self; 6] = [
        Self::Mirror,
        Self::Repeat,
        Self::Alternate,
        Self::Radial,
        Self::Echo,
        Self::Broken,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Mirror => "mirror",
            Self::Repeat => "repeat",
            Self::Alternate => "alternate",
            Self::Radial => "radial",
            Self::Echo => "echo",
            Self::Broken => "broken",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mirror => "Reflected family",
            Self::Repeat => "Repeated sweeps",
            Self::Alternate => "Alternating family",
            Self::Radial => "Rotational family",
            Self::Echo => "Nested echoes",
            Self::Broken => "Interrupted repetition",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mix {
    Sum,
    Weave,
    Modulate,
}

impl Mix {
    fn name(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Weave => "weave",
            Self::Modulate => "modulate",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub kind: Kind,
    pub count: usize,
    pub mix: Mix,
    pub spacing: f32,
    pub rotation: f32,
    pub scale: f32,
}

impl Plan {
    pub fn from_seed(seed: u64) -> Self {
        let mut state = seed ^ 0x617272616e676573;
        let mut next = || {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut n = state;
            n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
            n ^ (n >> 31)
        };
        // The proposal seed reserves low bits for stratified arrangement sampling.
        let kind = Kind::ALL[(seed & 7) as usize % Kind::ALL.len()];
        let count = match kind {
            Kind::Mirror => {
                if next() & 1 == 0 {
                    2
                } else {
                    4
                }
            }
            Kind::Radial | Kind::Broken => 3 + next() as usize % 2,
            _ => 2 + next() as usize % 3,
        };
        let mix = if kind == Kind::Mirror {
            Mix::Sum
        } else {
            [Mix::Sum, Mix::Weave, Mix::Modulate][next() as usize % 3]
        };
        let unit = |n: u64| (n >> 40) as f32 / (1u32 << 24) as f32;
        Self {
            kind,
            count,
            mix,
            spacing: 0.35 + 0.65 * unit(next()),
            rotation: (-35.0 + 70.0 * unit(next())).to_radians(),
            scale: 0.7 + 0.6 * unit(next()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(2..=4).contains(&self.count)
            || (self.kind == Kind::Mirror && self.count == 3)
            || (matches!(self.kind, Kind::Radial | Kind::Broken) && self.count < 3)
            || (self.kind == Kind::Mirror && self.mix != Mix::Sum)
            || !self.spacing.is_finite()
            || !(0.1..=1.5).contains(&self.spacing)
            || !self.rotation.is_finite()
            || !(-std::f32::consts::PI..=std::f32::consts::PI).contains(&self.rotation)
            || !self.scale.is_finite()
            || !(0.4..=2.0).contains(&self.scale)
        {
            return Err("Invalid arrangement plan".into());
        }
        Ok(())
    }

    fn instance(&self, i: usize) -> ([f32; 2], f32, [f32; 2], f32) {
        let centered = i as f32 - (self.count - 1) as f32 * 0.5;
        match self.kind {
            Kind::Mirror => {
                let x = if i & 1 == 0 { 1.0 } else { -1.0 };
                let y = if i < 2 { 1.0 } else { -1.0 };
                (
                    [0.5 * x, if self.count == 4 { 0.5 * y } else { 0.0 }],
                    0.0,
                    [x, y],
                    1.0,
                )
            }
            Kind::Repeat => ([centered, 0.0], 0.0, [1.0, 1.0], 1.0),
            Kind::Alternate => {
                let sign = if i & 1 == 0 { 1.0 } else { -1.0 };
                let density = if i & 1 == 0 { 0.8 } else { 1.25 };
                ([centered, 0.0], 0.0, [sign * density, density], sign)
            }
            Kind::Radial => {
                let angle = std::f32::consts::TAU * i as f32 / self.count as f32;
                (
                    [0.4 * angle.cos(), 0.4 * angle.sin()],
                    angle,
                    [1.0, 1.0],
                    1.0,
                )
            }
            Kind::Echo => {
                let density = 0.72f32.powi(i as i32);
                (
                    [centered * 0.55, centered * 0.3],
                    (i as f32 * 24.0).to_radians(),
                    [density, density],
                    1.0,
                )
            }
            Kind::Broken if i == self.count - 1 => {
                ([centered, 0.65], 35.0f32.to_radians(), [0.8, 0.8], -1.0)
            }
            Kind::Broken => ([centered, 0.0], 0.0, [1.0, 1.0], 1.0),
        }
    }
}

pub struct Unpacked {
    pub base: Snapshot,
    pub plan: Option<Plan>,
}

pub fn supports(program: &StarfishProgram, output: &str) -> bool {
    if !program
        .nodes
        .iter()
        .any(|node| matches!(node.operation, crate::program::Operation::Bezier))
    {
        return false;
    }
    let Some(item) = program.item(output) else {
        return false;
    };
    program.hir.iter().any(|decl| match decl {
        Decl::Definition(def) if def.id == item.id =>
            item.ty == Type::FieldScalar ||
                matches!(&def.expr, Expr::Call(name, args) if item.ty == Type::FieldColor && name == "palette" && args.len() == 2),
        _ => false,
    })
}

/// Restore the authored construction, retaining current shared parameter defaults.
/// Generated component bodies are rebuilt from that construction after mutation,
/// so gesture/texture edits cannot drift independently among related instances.
pub fn unpack(snapshot: &Snapshot) -> Result<Unpacked, String> {
    let mut headers = snapshot.source.match_indices(START);
    let Some((start, _)) = headers.next() else {
        return Ok(Unpacked {
            base: snapshot.clone(),
            plan: None,
        });
    };
    if headers.next().is_some() {
        return Err("Multiple arrangement plans in one source".into());
    }
    let line = snapshot.source[start + START.len()..]
        .lines()
        .next()
        .ok_or("Incomplete arrangement plan")?;
    let words: Vec<_> = line.split_whitespace().collect();
    let [prefix, target, kind_name, count, mix_name, encoded] = words.as_slice() else {
        return Err("Malformed arrangement plan".into());
    };
    let kind = Kind::ALL
        .into_iter()
        .find(|kind| kind.name() == *kind_name)
        .ok_or("Unknown arrangement kind")?;
    let mix = [Mix::Sum, Mix::Weave, Mix::Modulate]
        .into_iter()
        .find(|mix_value| mix_value.name() == *mix_name)
        .ok_or("Unknown arrangement combination")?;
    let count = count
        .parse::<usize>()
        .map_err(|_| "Invalid arrangement count")?;
    let end_marker = format!("{END}{prefix}");
    let end = snapshot.source[start..]
        .find(&end_marker)
        .ok_or("Missing arrangement end marker")?
        + start
        + end_marker.len();
    let block_start = start
        .checked_sub(5)
        .filter(|&i| &snapshot.source[i..start] == "\n    ")
        .ok_or("Malformed arrangement section boundary")?;
    if snapshot.source.as_bytes().get(end) != Some(&b'\n') {
        return Err("Malformed arrangement section end".into());
    }
    let program = snapshot.program()?;
    let target_item = program
        .item(target)
        .ok_or("Missing original arrangement output")?;
    let definition = program
        .hir
        .iter()
        .find_map(|decl| match decl {
            Decl::Definition(def) if def.id == target_item.id => Some(def),
            _ => None,
        })
        .ok_or("Missing arrangement output declaration")?;
    if definition.expression_span.end > start {
        return Err("Arrangement metadata overlaps its original output".into());
    }
    let value = |suffix: &str| {
        program
            .hir
            .iter()
            .find_map(|decl| match decl {
                Decl::Parameter(p) if p.name == format!("{prefix}{suffix}") => Some(p.initial),
                _ => None,
            })
            .ok_or_else(|| format!("Missing arrangement control: {suffix}"))
    };
    let plan = Plan {
        kind,
        count,
        mix,
        spacing: value("spacing")?,
        rotation: value("rotation")?,
        scale: value("scale")?,
    };
    plan.validate()?;
    let original = decode(encoded)?;
    let mut source = snapshot.source.clone();
    source.replace_range(block_start..end + 1, "");
    source.replace_range(definition.expression_span.clone(), &original);
    let base = Snapshot {
        source,
        output: target.to_string(),
    };
    base.program()?;
    Ok(Unpacked {
        base,
        plan: Some(plan),
    })
}

pub fn describe(source: &str) -> Option<String> {
    let line = source
        .lines()
        .find_map(|line| line.trim().strip_prefix(START))?;
    let words: Vec<_> = line.split_whitespace().collect();
    let kind = Kind::ALL
        .into_iter()
        .find(|kind| words.get(2) == Some(&kind.name()))?;
    Some(format!(
        "{} · {} motif instances",
        kind.label(),
        words.get(3)?
    ))
}

/// Lift field declarations into reusable components parameterized by coordinates.
/// Each instance calls the same construction; its affine frame expresses the rule.
pub fn compose(base: &Snapshot, plan: &Plan) -> Result<Snapshot, String> {
    plan.validate()?;
    if base.source.contains(START) {
        return Err("Unpack an existing arrangement before composing it".into());
    }
    let program = base.program()?;
    let item = program
        .item(&base.output)
        .ok_or("Unknown arrangement output")?;
    let target = program
        .hir
        .iter()
        .find_map(|decl| match decl {
            Decl::Definition(def) if def.id == item.id => Some(def),
            _ => None,
        })
        .ok_or("An arrangement needs a named scalar or palette output")?;
    let (root, output) = match (&target.expr, item.ty) {
        (expr, Type::FieldScalar) => (expr, None),
        (Expr::Call(name, args), Type::FieldColor) if name == "palette" && args.len() == 2 => {
            (&args[1], Some(&args[0]))
        }
        _ => {
            return Err(
                "Arrange a scalar field or an output that applies a palette directly".into(),
            );
        }
    };
    let mut prefix = "layout_".to_owned();
    let mut index = 1;
    while base.source.contains(&prefix) {
        prefix = format!("layout{index}_");
        index += 1;
    }
    let point = format!("{prefix}point");
    let definitions: HashSet<_> = program
        .hir
        .iter()
        .filter_map(|decl| match decl {
            Decl::Definition(def) if def.kind != DefinitionKind::Output => Some(def.name.as_str()),
            _ => None,
        })
        .collect();
    let components: HashSet<_> = program
        .hir
        .iter()
        .filter_map(|decl| match decl {
            Decl::Component(c) => Some(c.name.as_str()),
            _ => None,
        })
        .collect();
    let empty = HashSet::new();
    let rewrite = |expr: &Expr, locals: &HashSet<&str>| {
        expression(expr, &prefix, &point, &definitions, &components, locals)
    };
    let mut block = format!(
        "\n    {START}{prefix} {} {} {} {} {}\n",
        target.id,
        plan.kind.name(),
        plan.count,
        plan.mix.name(),
        encode(&target.expression)
    );
    block.push_str(&format!(
        "    // {}. All instances share gesture and texture bindings.\n",
        plan.kind.label()
    ));
    block.push_str(&format!(
        "    param {prefix}spacing: Scalar = {} in 0.1..1.5 step 0.01;\n    param {prefix}rotation: Angle = {}rad in -180deg..180deg step 0.5deg;\n    param {prefix}scale: Scalar = {} in 0.4..2 step 0.01;\n",
        plan.spacing, plan.rotation, plan.scale));
    for decl in &program.hir {
        match decl {
            Decl::Definition(def) if def.kind != DefinitionKind::Output => {
                let ty = program.item(&def.id).ok_or("Missing field type")?.ty;
                block.push_str(&format!(
                    "    component {prefix}field_{}({point}: Field<Point2>) -> {} = {};\n",
                    def.name,
                    ty.name(),
                    rewrite(&def.expr, &empty)
                ));
            }
            Decl::Component(component) => {
                let locals = component
                    .params
                    .iter()
                    .map(|param| param.name.as_str())
                    .collect();
                let parameters = component
                    .params
                    .iter()
                    .map(|param| format!(", {}: {}", param.name, param.ty.name()))
                    .collect::<String>();
                block.push_str(&format!("    component {prefix}component_{}({point}: Field<Point2>{parameters}) -> {} = {};\n",
                    component.name, component.return_type.name(), rewrite(&component.expr, &locals)));
            }
            _ => {}
        }
    }
    block.push_str(&format!(
        "    component {prefix}motif({point}: Field<Point2>) -> Field<Scalar> = {};\n",
        rewrite(root, &empty)
    ));
    block.push_str(&format!(
        "    frame {prefix}shared = rotate(world, {prefix}rotation);\n"
    ));
    let mut layers = Vec::new();
    for i in 0..plan.count {
        let (position, angle, factors, polarity) = plan.instance(i);
        block.push_str(&format!(
            "    frame {prefix}frame_{i} = scale(rotate({prefix}shared - point2({} * {prefix}spacing, {} * {prefix}spacing), {angle}rad), vec2({} * {prefix}scale, {} * {prefix}scale));\n    field {prefix}instance_{i} = {polarity} * {prefix}motif({prefix}frame_{i});\n",
            position[0], position[1], factors[0], factors[1]));
        layers.push(format!("{prefix}instance_{i}"));
    }
    let gain = 1.25 / (plan.count as f32).sqrt();
    let others = if layers.len() > 2 {
        layers[2..].join(" + ")
    } else {
        "0".into()
    };
    let mixture = match plan.mix {
        Mix::Sum => format!("{gain} * ({})", layers.join(" + ")),
        Mix::Weave => format!("1.2 * ({} * {} + 0.6 * ({others}))", layers[0], layers[1]),
        Mix::Modulate => format!("({} + {}) * (0.6 + 0.4 * ({others}))", layers[0], layers[1]),
    };
    block.push_str(&format!(
        "    field {prefix}composition = tanh({mixture});\n    {END}{prefix}\n"
    ));
    let replacement = match output {
        Some(palette) => format!("palette({}, {prefix}composition)", plain(palette)),
        None => format!("{prefix}composition"),
    };
    let mut source = base.source.clone();
    let end = source::program_end(&source)?;
    source.insert_str(end, &block);
    source.replace_range(target.expression_span.clone(), &replacement);
    let result = Snapshot {
        source,
        output: base.output.clone(),
    };
    result.program()?;
    Ok(result)
}

fn expression(
    expr: &Expr,
    prefix: &str,
    point: &str,
    definitions: &HashSet<&str>,
    components: &HashSet<&str>,
    locals: &HashSet<&str>,
) -> String {
    let recur = |expr| expression(expr, prefix, point, definitions, components, locals);
    match expr {
        Expr::Name(name) if !locals.contains(name.as_str()) && name == "world" => point.to_owned(),
        Expr::Name(name)
            if !locals.contains(name.as_str()) && definitions.contains(name.as_str()) =>
        {
            format!("{prefix}field_{name}({point})")
        }
        Expr::Name(name) => name.clone(),
        Expr::Number(value, ty) => literal(*value, *ty),
        Expr::UnaryMinus(value) => format!("(-{})", recur(value)),
        Expr::Binary(op, a, b) => format!("({} {op} {})", recur(a), recur(b)),
        Expr::Call(name, args) => {
            let args = args.iter().map(recur).collect::<Vec<_>>().join(", ");
            if components.contains(name.as_str()) {
                format!(
                    "{prefix}component_{name}({point}{}{args})",
                    if args.is_empty() { "" } else { ", " }
                )
            } else {
                format!("{name}({args})")
            }
        }
    }
}

fn plain(expr: &Expr) -> String {
    expression(
        expr,
        "",
        "world",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
    )
}
fn literal(value: f32, ty: Type) -> String {
    match ty {
        Type::Angle => format!("{value}rad"),
        Type::Frequency => format!("{value}cycles"),
        _ => value.to_string(),
    }
}
fn encode(value: &str) -> String {
    value.bytes().map(|byte| format!("{byte:02x}")).collect()
}
fn decode(value: &str) -> Result<String, String> {
    if value.len() % 2 != 0 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid saved arrangement expression".into());
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| "Invalid arrangement expression")
        })
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|_| "Arrangement expression is not UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        evaluate::{self, Evaluator, Value},
        program::ParameterBindings,
        render::{self, RenderRequest},
        variation::{self, Settings},
    };

    fn base() -> Snapshot {
        Snapshot {
            source: include_str!("../programs/curve_textures.sf").into(),
            output: "image".into(),
        }
    }
    fn mirror() -> Plan {
        Plan {
            kind: Kind::Mirror,
            count: 2,
            mix: Mix::Sum,
            spacing: 0.6,
            rotation: 0.0,
            scale: 1.0,
        }
    }
    fn scalar(value: Value) -> f32 {
        match value {
            Value::Scalar(value) => value,
            _ => panic!("Expected scalar"),
        }
    }

    #[test]
    fn reflection_matches_independent_sampling_and_shared_handle_edits() {
        let base = base();
        let arranged = compose(&base, &mirror()).unwrap();
        assert_eq!(unpack(&arranged).unwrap().base, base);
        let original = base.program().unwrap();
        let program = arranged.program().unwrap();
        let original_bindings = ParameterBindings::defaults(&original);
        let mut bindings = ParameterBindings::defaults(&program);
        for [x, y] in [[0.23, -0.41], [-0.7, 0.2], [0.4, 0.6]] {
            let a = scalar(
                Evaluator::new(&original, &original_bindings, "composition")
                    .unwrap()
                    .sample(x - 0.3, y),
            );
            let b = scalar(
                Evaluator::new(&original, &original_bindings, "composition")
                    .unwrap()
                    .sample(-x - 0.3, y),
            );
            let actual = scalar(
                Evaluator::new(&program, &bindings, "layout_composition")
                    .unwrap()
                    .sample(x, y),
            );
            assert!((actual - ((1.25 / 2f32.sqrt()) * (a + b)).tanh()).abs() < 2e-5);
            let reflected = scalar(
                Evaluator::new(&program, &bindings, "layout_composition")
                    .unwrap()
                    .sample(-x, y),
            );
            assert!((actual - reflected).abs() < 2e-5);
        }
        let guides = evaluate::curve_guides(&program, &bindings, "image").unwrap();
        assert_eq!(guides.len(), 4);
        assert!(guides.iter().all(|guide| (0..4).all(|i| guide.editable(i))));
        guides[0].move_point(1, [0.1, 0.4], &program, &mut bindings);
        let moved = evaluate::curve_guides(&program, &bindings, "image").unwrap();
        assert_ne!(guides[0].points[1], moved[0].points[1]);
        for i in 0..4 {
            assert!((moved[0].points[i][0] + moved[2].points[i][0]).abs() < 2e-5);
            assert!((moved[0].points[i][1] - moved[2].points[i][1]).abs() < 2e-5);
        }
        let rotation = program
            .parameters
            .iter()
            .position(|p| p.id == "layout_rotation")
            .unwrap();
        bindings.0[rotation] = 0.4;
        let captured = Snapshot::capture(&program, &bindings, "image").unwrap();
        let unpacked = unpack(&captured).unwrap();
        assert_eq!(unpacked.plan.as_ref().unwrap().rotation, 0.4);
        let rebuilt = compose(&unpacked.base, unpacked.plan.as_ref().unwrap()).unwrap();
        let image = |snapshot: &Snapshot| {
            let program = snapshot.program().unwrap();
            render::render(
                &program,
                &ParameterBindings::defaults(&program),
                RenderRequest {
                    output: "image",
                    width: 40,
                    height: 40,
                },
            )
            .unwrap()
            .rgba
        };
        assert_eq!(image(&captured), image(&rebuilt));
    }

    #[test]
    fn all_rules_render_and_remain_bounded_when_compositions_evolve() {
        let base = base();
        let mut images = HashSet::new();
        for kind in Kind::ALL {
            let mut plan = mirror();
            plan.kind = kind;
            plan.count = if matches!(kind, Kind::Radial | Kind::Broken) {
                3
            } else {
                2
            };
            let result = compose(&base, &plan).unwrap();
            assert_eq!(unpack(&result).unwrap().base, base);
            let program = result.program().unwrap();
            let guides =
                evaluate::curve_guides(&program, &ParameterBindings::defaults(&program), "image")
                    .unwrap();
            assert_eq!(guides.len(), plan.count * 2);
            let image = render::render(
                &program,
                &ParameterBindings::defaults(&program),
                RenderRequest {
                    output: "image",
                    width: 40,
                    height: 40,
                },
            )
            .unwrap()
            .rgba;
            assert!(
                images.insert(image),
                "Arrangement {:?} duplicates another rule",
                kind
            );
        }
        let mut parent = base.clone();
        let mut counts = HashSet::new();
        let mut kinds = HashSet::new();
        for seed in 0..16 {
            let settings = Settings {
                texture: false,
                composition: true,
                ..Default::default()
            };
            let child = variation::propose(&parent, settings, seed).unwrap();
            let unpacked = unpack(&child).unwrap();
            let plan = unpacked.plan.unwrap();
            counts.insert(plan.count);
            kinds.insert(plan.kind.name());
            assert_eq!(child.source.matches(START).count(), 1);
            assert!(child.source.len() < 40000);
            // Structural changes keep the authored controls and palette exact.
            let before = base.program().unwrap();
            let after = unpacked.base.program().unwrap();
            assert_eq!(
                before
                    .parameters
                    .iter()
                    .map(|p| p.initial.to_bits())
                    .collect::<Vec<_>>(),
                after
                    .parameters
                    .iter()
                    .map(|p| p.initial.to_bits())
                    .collect::<Vec<_>>()
            );
            assert_eq!(before.palettes[0].stops, after.palettes[0].stops);
            parent = child;
        }
        assert_eq!(counts.len(), 3);
        assert_eq!(kinds.len(), Kind::ALL.len());
    }

    #[test]
    fn locked_arrangements_survive_texture_and_gesture_variations() {
        let parent = compose(&base(), &mirror()).unwrap();
        let settings = Settings::default();
        let texture = variation::propose(&parent, settings, 83).unwrap();
        assert_eq!(unpack(&texture).unwrap().plan, Some(mirror()));
        assert_ne!(
            unpack(&texture).unwrap().base.source,
            unpack(&parent).unwrap().base.source
        );
        let gesture = variation::propose(
            &texture,
            Settings {
                texture: false,
                gesture: true,
                ..Default::default()
            },
            84,
        )
        .unwrap();
        assert_eq!(unpack(&gesture).unwrap().plan, Some(mirror()));
        let original = parent.program().unwrap();
        let changed = gesture.program().unwrap();
        let before =
            evaluate::curve_guides(&original, &ParameterBindings::defaults(&original), "image")
                .unwrap();
        let after =
            evaluate::curve_guides(&changed, &ParameterBindings::defaults(&changed), "image")
                .unwrap();
        assert_eq!(before.len(), after.len());
        assert_ne!(before[0].points, after[0].points);
        // The new gesture is still reflected, rather than drifting into independent copies.
        for i in 0..4 {
            assert!((after[0].points[i][0] + after[2].points[i][0]).abs() < 2e-5);
        }
    }

    #[test]
    fn generated_curve_sources_and_scalar_outputs_can_be_arranged() {
        for seed in 0..12 {
            let pair = crate::generate::comparison_pair(seed, 0);
            let base = Snapshot {
                source: pair.a.source(),
                output: "image".into(),
            };
            let result = compose(&base, &Plan::from_seed(seed)).unwrap();
            assert!(result.program().unwrap().nodes.len() < 4096);
        }
        let mut scalar_base = base();
        scalar_base.output = "braiding".into();
        let result = compose(&scalar_base, &mirror()).unwrap();
        assert_eq!(result.output, "braiding");
        assert_eq!(unpack(&result).unwrap().base, scalar_base);
        assert!(describe(&result.source).unwrap().starts_with("Reflected"));
        assert!(compose(&result, &mirror()).is_err());
        let corrupt = Snapshot {
            source: result.source.replace("arrangement-end", "missing-end"),
            ..result
        };
        assert!(unpack(&corrupt).is_err());
    }
}
