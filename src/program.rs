//! Source declarations are retained for inspection; these nodes are their executable form.

use std::collections::{HashMap, HashSet};

use crate::source::{Component, Decl, Definition, Expr, ParsedProgram, parse};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Scalar,
    Angle,
    Frequency,
    Vec2,
    Point2,
    FieldScalar,
    FieldPoint,
    FieldColor,
}

impl Type {
    pub fn name(self) -> &'static str {
        match self {
            Self::Scalar => "Scalar",
            Self::Angle => "Angle",
            Self::Frequency => "Frequency",
            Self::Vec2 => "Vec2",
            Self::Point2 => "Point2",
            Self::FieldScalar => "Field<Scalar>",
            Self::FieldPoint => "Field<Point2>",
            Self::FieldColor => "Field<Color>",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionKind {
    Frame,
    Axis,
    Field,
    Output,
}

impl DefinitionKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Frame => "Frame",
            Self::Axis => "Axis",
            Self::Field => "Field",
            Self::Output => "Output",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub id: String,
    pub name: String,
    pub ty: Type,
    pub initial: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Clone, Debug)]
pub struct ParameterBindings(pub Vec<f32>);

impl ParameterBindings {
    pub fn defaults(program: &StarfishProgram) -> Self {
        Self(program.parameters.iter().map(|p| p.initial).collect())
    }
}

#[derive(Clone, Debug)]
pub enum Operation {
    Dummy,
    Scalar(f32),
    Vec2,
    World,
    Parameter(usize),
    Rotate,
    Dot,
    Sub,
    Length,
    Wave,
    Add,
    Multiply,
    Negate,
    Tanh,
    Palette(usize),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub operation: Operation,
    pub inputs: Vec<usize>,
    pub ty: Type,
}

#[derive(Clone, Debug)]
pub struct ProgramItem {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub kind: DefinitionKind,
    pub ty: Type,
    pub node: usize,
    pub expression: String,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub name: String,
    pub stops: Vec<[f32; 3]>,
}

#[derive(Clone, Debug)]
pub struct StarfishProgram {
    pub id: String,
    pub name: String,
    pub source: String,
    pub hir: Vec<Decl>,
    pub parameters: Vec<Parameter>,
    pub items: Vec<ProgramItem>,
    pub components: Vec<String>,
    pub palettes: Vec<Palette>,
    pub nodes: Vec<Node>,
    pub output: String,
}

impl StarfishProgram {
    pub fn from_source(source: &str) -> Result<Self, String> {
        let parsed = parse(source)?;
        Compiler::new(parsed, source)?.compile()
    }

    pub fn item(&self, id: &str) -> Option<&ProgramItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn sample() -> Self {
        Self::from_source(include_str!("../programs/shared_tides.sf"))
            .expect("bundled sample must be valid")
    }
}

struct Compiler<'a> {
    parsed: ParsedProgram,
    source: &'a str,
    symbols: HashMap<String, usize>,
    param_indices: HashMap<String, usize>,
    def_nodes: HashMap<String, (usize, Type)>,
    active: HashSet<String>,
    nodes: Vec<Node>,
    parameters: Vec<Parameter>,
    palettes: Vec<Palette>,
    components: Vec<String>,
}

impl<'a> Compiler<'a> {
    fn new(parsed: ParsedProgram, source: &'a str) -> Result<Self, String> {
        let mut symbols = HashMap::new();
        let mut ids = HashSet::new();
        ids.insert(parsed.id.clone());
        let mut parameters = Vec::new();
        let mut param_indices = HashMap::new();
        let mut palettes = Vec::new();
        let mut components = Vec::new();
        for (index, decl) in parsed.decls.iter().enumerate() {
            let name = decl.symbol();
            if [
                "world", "rotate", "dot", "length", "wave", "tanh", "vec2", "point2", "palette",
                "rgb", "smooth",
            ]
            .contains(&name)
                || symbols.insert(name.to_owned(), index).is_some()
            {
                return Err(format!("Duplicate or reserved name: {name}"));
            }
            if !ids.insert(decl.id().to_owned()) {
                return Err(format!("Duplicate stable ID: {}", decl.id()));
            }
            match decl {
                Decl::Parameter(p) => {
                    if !matches!(p.ty, Type::Scalar | Type::Angle | Type::Frequency) {
                        return Err(format!(
                            "Parameter {} must have a uniform numeric type",
                            p.name
                        ));
                    }
                    if ![p.initial, p.min, p.max, p.step]
                        .iter()
                        .all(|v| v.is_finite())
                        || p.min >= p.max
                        || p.initial < p.min
                        || p.initial > p.max
                        || p.step <= 0.0
                    {
                        return Err(format!("Invalid range or default for parameter {}", p.name));
                    }
                    param_indices.insert(p.name.clone(), parameters.len());
                    parameters.push(Parameter {
                        id: p.id.clone(),
                        name: display_name(&p.name),
                        ty: p.ty,
                        initial: p.initial,
                        min: p.min,
                        max: p.max,
                        step: p.step,
                    });
                }
                Decl::Palette(p) => {
                    if p.stops.len() < 2
                        || !p.stops.iter().flatten().all(|v| (0.0..=1.0).contains(v))
                    {
                        return Err(format!(
                            "Palette {} needs at least two RGB stops in [0, 1]",
                            p.name
                        ));
                    }
                    palettes.push(Palette {
                        name: p.name.clone(),
                        stops: p.stops.clone(),
                    });
                }
                Decl::Component(c) => components.push(c.name.clone()),
                Decl::Definition(_) => {}
            }
        }
        Ok(Self {
            parsed,
            source,
            symbols,
            param_indices,
            def_nodes: HashMap::new(),
            active: HashSet::new(),
            nodes: Vec::new(),
            parameters,
            palettes,
            components,
        })
    }

    fn compile(mut self) -> Result<StarfishProgram, String> {
        let defs: Vec<Definition> = self
            .parsed
            .decls
            .iter()
            .filter_map(|d| match d {
                Decl::Definition(def) => Some(def.clone()),
                _ => None,
            })
            .collect();
        let outputs: Vec<_> = defs
            .iter()
            .filter(|d| d.kind == DefinitionKind::Output)
            .collect();
        if outputs.len() != 1 {
            return Err("A program must declare exactly one color output".into());
        }
        let output = outputs[0].id.clone();
        let mut items = Vec::new();
        for def in &defs {
            let (node, ty) = self.compile_definition(&def.name)?;
            let expected = match def.kind {
                DefinitionKind::Frame => Type::FieldPoint,
                DefinitionKind::Axis => Type::Vec2,
                DefinitionKind::Field => Type::FieldScalar,
                DefinitionKind::Output => Type::FieldColor,
            };
            if ty != expected
                || def
                    .annotated_type
                    .is_some_and(|annotation| annotation != ty)
            {
                return Err(format!(
                    "{} has type {}; expected {}",
                    def.name,
                    ty.name(),
                    expected.name()
                ));
            }
            let mut deps = Vec::new();
            collect_dependencies(&def.expr, &self.symbols, &mut deps);
            items.push(ProgramItem {
                id: def.id.clone(),
                symbol: def.name.clone(),
                name: display_name(&def.name),
                kind: def.kind,
                ty,
                node,
                expression: def.expression.clone(),
                dependencies: deps,
            });
        }
        // Check component bodies even when a source file has not instantiated them yet.
        let components: Vec<Component> = self
            .parsed
            .decls
            .iter()
            .filter_map(|d| match d {
                Decl::Component(component) => Some(component.clone()),
                _ => None,
            })
            .collect();
        for component in &components {
            let mut scope = HashMap::new();
            for param in &component.params {
                let value = self.push(Operation::Dummy, vec![], param.ty)?;
                scope.insert(param.name.clone(), value);
            }
            let (_, actual) = self
                .compile_expr(&component.expr, &scope, 0)
                .map_err(|error| format!("in component {}: {error}", component.name))?;
            if actual != component.return_type {
                return Err(format!(
                    "Component {} returns {}, declared {}",
                    component.name,
                    actual.name(),
                    component.return_type.name()
                ));
            }
        }
        Ok(StarfishProgram {
            id: self.parsed.id,
            name: self.parsed.name,
            source: self.source.to_owned(),
            hir: self.parsed.decls,
            parameters: self.parameters,
            items,
            components: self.components,
            palettes: self.palettes,
            nodes: self.nodes,
            output,
        })
    }

    fn compile_definition(&mut self, name: &str) -> Result<(usize, Type), String> {
        if let Some(&cached) = self.def_nodes.get(name) {
            return Ok(cached);
        }
        if !self.active.insert(name.to_owned()) {
            return Err(format!("Cyclic definition involving {name}"));
        }
        let index = *self
            .symbols
            .get(name)
            .ok_or_else(|| format!("Unknown name: {name}"))?;
        let def = match &self.parsed.decls[index] {
            Decl::Definition(def) => def.clone(),
            _ => return Err(format!("{name} is not a field or frame")),
        };
        let result = self
            .compile_expr(&def.expr, &HashMap::new(), 0)
            .map_err(|error| format!("in {}: {error}", def.name))?;
        self.active.remove(name);
        self.def_nodes.insert(name.to_owned(), result);
        Ok(result)
    }

    fn push(
        &mut self,
        operation: Operation,
        inputs: Vec<usize>,
        ty: Type,
    ) -> Result<(usize, Type), String> {
        if self.nodes.len() >= 4096 {
            return Err("Program exceeds 4096 lowered nodes".into());
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            operation,
            inputs,
            ty,
        });
        Ok((index, ty))
    }

    fn compile_expr(
        &mut self,
        expr: &Expr,
        locals: &HashMap<String, (usize, Type)>,
        depth: usize,
    ) -> Result<(usize, Type), String> {
        if depth > 32 {
            return Err("Component expansion exceeds 32 calls".into());
        }
        match expr {
            Expr::Number(value, ty) => self.push(Operation::Scalar(*value), vec![], *ty),
            Expr::Name(name) => {
                if let Some(&local) = locals.get(name) {
                    return Ok(local);
                }
                if name == "world" {
                    return self.push(Operation::World, vec![], Type::FieldPoint);
                }
                if let Some(&p) = self.param_indices.get(name) {
                    return self.push(Operation::Parameter(p), vec![], self.parameters[p].ty);
                }
                self.compile_definition(name)
            }
            Expr::UnaryMinus(value) => {
                let (input, ty) = self.compile_expr(value, locals, depth)?;
                if !matches!(
                    ty,
                    Type::Scalar | Type::Angle | Type::Frequency | Type::FieldScalar
                ) {
                    return Err(format!(
                        "Unary minus requires a numeric value, got {}",
                        ty.name()
                    ));
                }
                self.push(Operation::Negate, vec![input], ty)
            }
            Expr::Binary(op, lhs, rhs) => {
                let (a, ta) = self.compile_expr(lhs, locals, depth)?;
                let (b, tb) = self.compile_expr(rhs, locals, depth)?;
                let result = match (op.as_str(), ta, tb) {
                    ("+", Type::FieldScalar, Type::FieldScalar | Type::Scalar)
                    | ("+", Type::Scalar, Type::FieldScalar) => Type::FieldScalar,
                    ("+", Type::Scalar, Type::Scalar) => Type::Scalar,
                    ("+", Type::Angle, Type::Angle) => Type::Angle,
                    ("*", Type::FieldScalar, Type::FieldScalar | Type::Scalar)
                    | ("*", Type::Scalar, Type::FieldScalar) => Type::FieldScalar,
                    ("*", Type::Scalar, Type::Scalar) => Type::Scalar,
                    ("-", Type::FieldPoint, Type::Point2) => Type::FieldPoint,
                    ("-", Type::FieldScalar, Type::FieldScalar | Type::Scalar)
                    | ("-", Type::Scalar, Type::FieldScalar) => Type::FieldScalar,
                    ("-", Type::Scalar, Type::Scalar) => Type::Scalar,
                    _ => {
                        return Err(format!(
                            "Operator {op} cannot combine {} and {}",
                            ta.name(),
                            tb.name()
                        ));
                    }
                };
                let operation = match op.as_str() {
                    "+" => Operation::Add,
                    "-" => Operation::Sub,
                    "*" => Operation::Multiply,
                    _ => unreachable!(),
                };
                self.push(operation, vec![a, b], result)
            }
            Expr::Call(name, args) => self.compile_call(name, args, locals, depth),
        }
    }

    fn compile_call(
        &mut self,
        name: &str,
        args: &[Expr],
        locals: &HashMap<String, (usize, Type)>,
        depth: usize,
    ) -> Result<(usize, Type), String> {
        if name == "palette" {
            if args.len() != 2 {
                return Err("palette expects a palette and a scalar field".into());
            }
            let Expr::Name(palette_name) = &args[0] else {
                return Err("palette's first argument must be a palette name".into());
            };
            let palette = self
                .palettes
                .iter()
                .position(|p| &p.name == palette_name)
                .ok_or_else(|| format!("Unknown palette: {palette_name}"))?;
            let (field, ty) = self.compile_expr(&args[1], locals, depth)?;
            if ty != Type::FieldScalar {
                return Err("palette expects a scalar field".into());
            }
            return self.push(Operation::Palette(palette), vec![field], Type::FieldColor);
        }
        if let Some(&index) = self.symbols.get(name) {
            if let Decl::Component(component) = &self.parsed.decls[index] {
                let component: Component = component.clone();
                if args.len() != component.params.len() {
                    return Err(format!(
                        "{} expects {} arguments",
                        name,
                        component.params.len()
                    ));
                }
                let mut scope = HashMap::new();
                for (arg, param) in args.iter().zip(&component.params) {
                    let result = self.compile_expr(arg, locals, depth)?;
                    if result.1 != param.ty {
                        return Err(format!(
                            "Argument {} of {} requires {}",
                            param.name,
                            name,
                            param.ty.name()
                        ));
                    }
                    scope.insert(param.name.clone(), result);
                }
                let result = self.compile_expr(&component.expr, &scope, depth + 1)?;
                if result.1 != component.return_type {
                    return Err(format!(
                        "Component {} returns {}, declared {}",
                        name,
                        result.1.name(),
                        component.return_type.name()
                    ));
                }
                return Ok(result);
            }
        }
        let mut compiled = Vec::new();
        for arg in args {
            compiled.push(self.compile_expr(arg, locals, depth)?);
        }
        let types: Vec<Type> = compiled.iter().map(|x| x.1).collect();
        let indices = compiled.iter().map(|x| x.0).collect();
        let (op, ty) = match (name, types.as_slice()) {
            ("vec2", [Type::Scalar, Type::Scalar]) => (Operation::Vec2, Type::Vec2),
            ("point2", [Type::Scalar, Type::Scalar]) => (Operation::Vec2, Type::Point2),
            ("rotate", [Type::FieldPoint, Type::Angle]) => (Operation::Rotate, Type::FieldPoint),
            ("rotate", [Type::Vec2, Type::Angle]) => (Operation::Rotate, Type::Vec2),
            ("dot", [Type::FieldPoint, Type::Vec2]) => (Operation::Dot, Type::FieldScalar),
            ("length", [Type::FieldPoint]) => (Operation::Length, Type::FieldScalar),
            ("wave", [Type::FieldScalar, Type::Frequency, Type::Angle]) => {
                (Operation::Wave, Type::FieldScalar)
            }
            ("tanh", [Type::FieldScalar]) => (Operation::Tanh, Type::FieldScalar),
            _ => {
                return Err(format!(
                    "Unknown call or invalid argument types: {name}({})",
                    types
                        .iter()
                        .map(|t| t.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        };
        self.push(op, indices, ty)
    }
}

fn collect_dependencies(expr: &Expr, symbols: &HashMap<String, usize>, out: &mut Vec<String>) {
    match expr {
        Expr::Name(name) => {
            if symbols.contains_key(name) && !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::UnaryMinus(a) => collect_dependencies(a, symbols, out),
        Expr::Binary(_, a, b) => {
            collect_dependencies(a, symbols, out);
            collect_dependencies(b, symbols, out);
        }
        Expr::Call(name, args) => {
            if symbols.contains_key(name) && !out.contains(name) {
                out.push(name.clone());
            }
            for arg in args {
                collect_dependencies(arg, symbols, out);
            }
        }
        Expr::Number(..) => {}
    }
}

pub fn display_name(name: &str) -> String {
    let mut result = name.replace('_', " ");
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_saved_programs_compile_with_distinct_structures() {
        let first = StarfishProgram::sample();
        let second =
            StarfishProgram::from_source(include_str!("../programs/crosscurrent.sf")).unwrap();
        assert_eq!(first.output, "color");
        assert_eq!(second.output, "image");
        assert!(first.item("pattern_a").is_some());
        assert!(second.item("fine").is_some());
        assert!(
            first
                .items
                .iter()
                .any(|item| item.kind == DefinitionKind::Frame)
        );
        assert!(first.components.iter().any(|name| name == "stripe"));
    }

    #[test]
    fn validation_rejects_bad_links_types_and_cycles() {
        let base = "starfish 1; program \"Test\" {
            field value: Scalar = wave(dot(world, vec2(1, 0)), 2, 0rad);
            palette ink = smooth [rgb(0, 0, 0), rgb(1, 1, 1)];
            output image: Field<Color> = palette(ink, value);
        }";
        assert!(
            StarfishProgram::from_source(base)
                .unwrap_err()
                .contains("wave")
        );
        assert!(StarfishProgram::from_source(&base.replace(", 2,", ", 2cycles,")).is_ok());
        let valid = base.replace(
            "wave(dot(world, vec2(1, 0)), 2, 0rad)",
            "tanh(dot(world, vec2(1, 0)))",
        );
        assert!(StarfishProgram::from_source(&valid).is_ok());
        assert!(
            StarfishProgram::from_source(&valid.replace("dot(world", "dot(missing"))
                .unwrap_err()
                .contains("Unknown name")
        );
        assert!(
            StarfishProgram::from_source(&valid.replace("tanh(dot(world, vec2(1, 0)))", "value"))
                .unwrap_err()
                .contains("Cyclic")
        );
        assert!(
            StarfishProgram::from_source(&valid.replace("output image", "output value"))
                .unwrap_err()
                .contains("Duplicate")
        );
        let unused_bad_component = valid.replace(
            "field value",
            "component broken(x: Scalar) -> Field<Scalar> = x; field value",
        );
        assert!(
            StarfishProgram::from_source(&unused_bad_component)
                .unwrap_err()
                .contains("Component broken returns")
        );
        assert!(
            StarfishProgram::from_source("starfish 1; program \"Broken\" { field x =").is_err()
        );
    }
}
