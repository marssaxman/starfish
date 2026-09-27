//! A small, versioned Starfish source language. The parser retains named structure.

use crate::program::{DefinitionKind, Type};

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f32, Type),
    Name(String),
    UnaryMinus(Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug)]
pub struct ParameterDecl {
    pub name: String,
    pub id: String,
    pub ty: Type,
    pub initial: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub kind: DefinitionKind,
    pub name: String,
    pub id: String,
    pub annotated_type: Option<Type>,
    pub expr: Expr,
    pub expression: String,
}

#[derive(Clone, Debug)]
pub struct ComponentParam {
    pub name: String,
    pub ty: Type,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub name: String,
    pub id: String,
    pub params: Vec<ComponentParam>,
    pub return_type: Type,
    pub expr: Expr,
    pub expression: String,
}

#[derive(Clone, Debug)]
pub struct PaletteDecl {
    pub name: String,
    pub id: String,
    pub stops: Vec<[f32; 3]>,
}

#[derive(Clone, Debug)]
pub enum Decl {
    Parameter(ParameterDecl),
    Definition(Definition),
    Component(Component),
    Palette(PaletteDecl),
}

impl Decl {
    pub fn symbol(&self) -> &str {
        match self {
            Self::Parameter(d) => &d.name,
            Self::Definition(d) => &d.name,
            Self::Component(d) => &d.name,
            Self::Palette(d) => &d.name,
        }
    }
    pub fn id(&self) -> &str {
        match self {
            Self::Parameter(d) => &d.id,
            Self::Definition(d) => &d.id,
            Self::Component(d) => &d.id,
            Self::Palette(d) => &d.id,
        }
    }
}

pub struct ParsedProgram {
    pub name: String,
    pub id: String,
    pub decls: Vec<Decl>,
}

#[derive(Clone, Debug)]
enum TokenKind {
    Ident(String),
    Number(String),
    String(String),
    Symbol(char),
    Arrow,
    Range,
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

fn lex(source: &str) -> Result<Vec<Token>, String> {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            let start = i - 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return Err(format!("Unclosed comment at byte {start}"));
            }
            i += 2;
            continue;
        }
        let start = i;
        let kind = if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            TokenKind::Ident(source[start..i].to_owned())
        } else if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if bytes.get(i) == Some(&b'.') && bytes.get(i + 1) != Some(&b'.') {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            TokenKind::Number(source[start..i].to_owned())
        } else if bytes[i] == b'"' {
            i += 1;
            let content_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    return Err(format!("Escapes are not supported in strings at byte {i}"));
                }
                i += 1;
            }
            if i >= bytes.len() {
                return Err(format!("Unclosed string at byte {start}"));
            }
            let value = source[content_start..i].to_owned();
            i += 1;
            TokenKind::String(value)
        } else if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'>') {
            i += 2;
            TokenKind::Arrow
        } else if bytes[i] == b'.' && bytes.get(i + 1) == Some(&b'.') {
            i += 2;
            TokenKind::Range
        } else if b"@:=()[]{};,+-*<>".contains(&bytes[i]) {
            i += 1;
            TokenKind::Symbol(bytes[start] as char)
        } else {
            return Err(format!("Unexpected character at byte {i}"));
        };
        tokens.push(Token {
            kind,
            start,
            end: i,
        });
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        start: i,
        end: i,
    });
    Ok(tokens)
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    at: usize,
}

pub fn parse(source: &str) -> Result<ParsedProgram, String> {
    let tokens = lex(source)?;
    Parser {
        source,
        tokens,
        at: 0,
    }
    .program()
}

impl Parser<'_> {
    fn current(&self) -> &TokenKind {
        &self.tokens[self.at].kind
    }
    fn bump(&mut self) -> TokenKind {
        let token = self.tokens[self.at].kind.clone();
        self.at += 1;
        token
    }
    fn error(&self, message: &str) -> String {
        let before = &self.source[..self.tokens[self.at].start];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        format!("{message} at {line}:{column}")
    }
    fn symbol(&mut self, symbol: char) -> bool {
        if matches!(self.current(), TokenKind::Symbol(c) if *c == symbol) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect_symbol(&mut self, symbol: char) -> Result<(), String> {
        if self.symbol(symbol) {
            Ok(())
        } else {
            Err(self.error(&format!("Expected '{symbol}'")))
        }
    }
    fn keyword(&mut self, word: &str) -> bool {
        if matches!(self.current(), TokenKind::Ident(s) if s == word) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect_keyword(&mut self, word: &str) -> Result<(), String> {
        if self.keyword(word) {
            Ok(())
        } else {
            Err(self.error(&format!("Expected '{word}'")))
        }
    }
    fn ident(&mut self) -> Result<String, String> {
        if matches!(self.current(), TokenKind::Ident(_)) {
            let TokenKind::Ident(name) = self.bump() else {
                unreachable!()
            };
            Ok(name)
        } else {
            Err(self.error("Expected identifier"))
        }
    }
    fn string(&mut self) -> Result<String, String> {
        if matches!(self.current(), TokenKind::String(_)) {
            let TokenKind::String(s) = self.bump() else {
                unreachable!()
            };
            Ok(s)
        } else {
            Err(self.error("Expected string"))
        }
    }
    fn id(&mut self, default: &str) -> Result<String, String> {
        let id = if self.symbol('@') {
            self.expect_keyword("id")?;
            self.expect_symbol('(')?;
            let value = self.string()?;
            self.expect_symbol(')')?;
            value
        } else {
            default.to_owned()
        };
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(self.error("Stable IDs must use ASCII letters, digits, '_' or '-'"));
        }
        Ok(id)
    }
    fn ty(&mut self) -> Result<Type, String> {
        let name = self.ident()?;
        if name == "Field" {
            self.expect_symbol('<')?;
            let inner = self.ident()?;
            self.expect_symbol('>')?;
            return match inner.as_str() {
                "Scalar" => Ok(Type::FieldScalar),
                "Point2" => Ok(Type::FieldPoint),
                "Color" => Ok(Type::FieldColor),
                _ => Err(self.error("Unsupported field type")),
            };
        }
        match name.as_str() {
            "Scalar" => Ok(Type::Scalar),
            "Angle" => Ok(Type::Angle),
            "Frequency" => Ok(Type::Frequency),
            "Vec2" => Ok(Type::Vec2),
            "Point2" => Ok(Type::Point2),
            _ => Err(self.error(&format!("Unknown type {name}"))),
        }
    }
    fn signed_number(&mut self, expected: Type) -> Result<f32, String> {
        let sign = if self.symbol('-') { -1.0 } else { 1.0 };
        let TokenKind::Number(raw) = self.current() else {
            return Err(self.error("Expected numeric literal"));
        };
        let (value, ty) = parse_number(raw).map_err(|e| self.error(&e))?;
        if ty != expected && !(ty == Type::Scalar && expected == Type::Frequency) {
            return Err(self.error(&format!(
                "Expected {}, found {}",
                expected.name(),
                ty.name()
            )));
        }
        self.bump();
        Ok(sign * value)
    }
    fn program(&mut self) -> Result<ParsedProgram, String> {
        self.expect_keyword("starfish")?;
        if self.signed_number(Type::Scalar)? != 1.0 {
            return Err(self.error("Only Starfish source version 1 is supported"));
        }
        self.expect_symbol(';')?;
        self.expect_keyword("program")?;
        let name = self.string()?;
        let default_id = name.to_lowercase().replace(' ', "_");
        let id = self.id(&default_id)?;
        self.expect_symbol('{')?;
        let mut decls = Vec::new();
        while !self.symbol('}') {
            if matches!(self.current(), TokenKind::Eof) {
                return Err(self.error("Unclosed program"));
            }
            decls.push(self.decl()?);
        }
        if !matches!(self.current(), TokenKind::Eof) {
            return Err(self.error("Unexpected text after program"));
        }
        Ok(ParsedProgram { name, id, decls })
    }
    fn decl(&mut self) -> Result<Decl, String> {
        let kind = self.ident()?;
        let name = self.ident()?;
        let id = self.id(&name)?;
        match kind.as_str() {
            "param" => {
                self.expect_symbol(':')?;
                let ty = self.ty()?;
                self.expect_symbol('=')?;
                let initial = self.signed_number(ty)?;
                self.expect_keyword("in")?;
                let min = self.signed_number(ty)?;
                if !matches!(self.current(), TokenKind::Range) {
                    return Err(self.error("Expected '..'"));
                }
                self.bump();
                let max = self.signed_number(ty)?;
                let step = if self.keyword("step") {
                    self.signed_number(ty)?
                } else {
                    match ty {
                        Type::Angle => 1.0_f32.to_radians(),
                        _ => 0.01,
                    }
                };
                self.expect_symbol(';')?;
                Ok(Decl::Parameter(ParameterDecl {
                    name,
                    id,
                    ty,
                    initial,
                    min,
                    max,
                    step,
                }))
            }
            "frame" | "axis" | "field" | "output" => {
                let kind = match kind.as_str() {
                    "frame" => DefinitionKind::Frame,
                    "axis" => DefinitionKind::Axis,
                    "field" => DefinitionKind::Field,
                    _ => DefinitionKind::Output,
                };
                let annotated_type = if self.symbol(':') {
                    let ty = self.ty()?;
                    Some(match (kind, ty) {
                        (DefinitionKind::Field, Type::Scalar) => Type::FieldScalar,
                        _ => ty,
                    })
                } else {
                    None
                };
                self.expect_symbol('=')?;
                let start = self.tokens[self.at].start;
                let expr = self.expr(0)?;
                let end = self.tokens[self.at - 1].end;
                self.expect_symbol(';')?;
                Ok(Decl::Definition(Definition {
                    kind,
                    name,
                    id,
                    annotated_type,
                    expr,
                    expression: self.source[start..end].trim().to_owned(),
                }))
            }
            "component" => {
                self.expect_symbol('(')?;
                let mut params = Vec::new();
                if !self.symbol(')') {
                    loop {
                        let param_name = self.ident()?;
                        self.expect_symbol(':')?;
                        let ty = self.ty()?;
                        if params.iter().any(|p: &ComponentParam| p.name == param_name) {
                            return Err(self.error("Duplicate component argument"));
                        }
                        params.push(ComponentParam {
                            name: param_name,
                            ty,
                        });
                        if self.symbol(')') {
                            break;
                        }
                        self.expect_symbol(',')?;
                    }
                }
                if !matches!(self.current(), TokenKind::Arrow) {
                    return Err(self.error("Expected '->'"));
                }
                self.bump();
                let return_type = self.ty()?;
                self.expect_symbol('=')?;
                let start = self.tokens[self.at].start;
                let expr = self.expr(0)?;
                let end = self.tokens[self.at - 1].end;
                self.expect_symbol(';')?;
                Ok(Decl::Component(Component {
                    name,
                    id,
                    params,
                    return_type,
                    expr,
                    expression: self.source[start..end].trim().to_owned(),
                }))
            }
            "palette" => {
                self.expect_symbol('=')?;
                self.expect_keyword("smooth")?;
                self.expect_symbol('[')?;
                let mut stops = Vec::new();
                loop {
                    self.expect_keyword("rgb")?;
                    self.expect_symbol('(')?;
                    let r = self.signed_number(Type::Scalar)?;
                    self.expect_symbol(',')?;
                    let g = self.signed_number(Type::Scalar)?;
                    self.expect_symbol(',')?;
                    let b = self.signed_number(Type::Scalar)?;
                    self.expect_symbol(')')?;
                    stops.push([r, g, b]);
                    if self.symbol(']') {
                        break;
                    }
                    self.expect_symbol(',')?;
                }
                self.expect_symbol(';')?;
                Ok(Decl::Palette(PaletteDecl { name, id, stops }))
            }
            _ => Err(self.error(&format!("Unknown declaration kind: {kind}"))),
        }
    }
    fn expr(&mut self, minimum: u8) -> Result<Expr, String> {
        let mut left = if self.symbol('-') {
            Expr::UnaryMinus(Box::new(self.expr(3)?))
        } else if self.symbol('(') {
            let inner = self.expr(0)?;
            self.expect_symbol(')')?;
            inner
        } else {
            match self.current().clone() {
                TokenKind::Number(raw) => {
                    self.bump();
                    let (value, ty) = parse_number(&raw).map_err(|e| self.error(&e))?;
                    Expr::Number(value, ty)
                }
                TokenKind::Ident(name) => {
                    self.bump();
                    if self.symbol('(') {
                        let mut args = Vec::new();
                        if !self.symbol(')') {
                            loop {
                                args.push(self.expr(0)?);
                                if self.symbol(')') {
                                    break;
                                }
                                self.expect_symbol(',')?;
                            }
                        }
                        Expr::Call(name, args)
                    } else {
                        Expr::Name(name)
                    }
                }
                _ => return Err(self.error("Expected expression")),
            }
        };
        loop {
            let (op, precedence) = match self.current() {
                TokenKind::Symbol('+') => ("+", 1),
                TokenKind::Symbol('-') => ("-", 1),
                TokenKind::Symbol('*') => ("*", 2),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.bump();
            let right = self.expr(precedence + 1)?;
            left = Expr::Binary(op.to_owned(), Box::new(left), Box::new(right));
        }
        Ok(left)
    }
}

fn parse_number(raw: &str) -> Result<(f32, Type), String> {
    let split = raw
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(raw.len());
    let value: f32 = raw[..split]
        .parse()
        .map_err(|_| format!("Invalid number {raw}"))?;
    if !value.is_finite() {
        return Err(format!("Non-finite number {raw}"));
    }
    match &raw[split..] {
        "" => Ok((value, Type::Scalar)),
        "deg" => Ok((value.to_radians(), Type::Angle)),
        "rad" => Ok((value, Type::Angle)),
        "cycles" => Ok((value, Type::Frequency)),
        _ => Err(format!("Unknown numeric unit in {raw}")),
    }
}
