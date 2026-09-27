# Starfish source language, version 1

A .sf file is UTF-8 source for one self-contained image program. See
[Shared tides](../programs/shared_tides.sf) and
[Crosscurrent](../programs/crosscurrent.sf) for complete examples. The source
is authoritative; the parser retains declarations and expressions as a
structured program and lowers them to an executable graph. Studio displays
the original source verbatim.

The file starts with `starfish 1;`, followed by one `program "Title" { ... }`.
Declarations may refer to names declared later in the file. Names use ASCII
letters, digits, and underscores, starting with a letter or underscore.
The language is case-sensitive. Semicolons end declarations. `//` and
`/* ... */` comments are accepted.

Named declarations may carry `@id("stable-id")`. If omitted, the source
identifier is the ID. IDs are unique throughout the file and use ASCII
letters, digits, underscores, or hyphens. Studio and the renderer select
intermediate outputs by ID, so a source identifier can be renamed while
retaining its explicit ID. The display label is currently derived from the
identifier.

## Values and fields

The uniform types are `Scalar`, `Angle`, `Frequency`, `Vec2`, and
`Point2`. A `Field<T>` is a pure function from the world point to `T`.
Version 1 supports `Field<Point2>`, `Field<Scalar>`, and
`Field<Color>`. The built-in `world` is a `Field<Point2>`.

The world is sampled continuously in `[-1, 1]²`, with positive y upward.
Rendering samples pixel centers. Thus the source has no fixed resolution.
A `frame` declaration must produce `Field<Point2>`: at each world point
it yields coordinates in another frame. `rotate(world, angle)` applies
the positive rotation matrix to sample coordinates. The apparent image
rotation is therefore the inverse of that coordinate rotation.

`Angle` literals require `deg` or `rad` suffixes. Internally they are
radians. `Frequency` is cycles per normalized coordinate unit; expression
literals use the `cycles` suffix. Parameter defaults and ranges may use
plain numbers when the declared type is `Frequency`. A bare number in an
expression is `Scalar`.

A `param` declares a runtime binding, its default, and its slider range:

    param rotation: Angle = 24deg in -180deg..180deg step 0.5deg;
    param frequency: Frequency = 6 in 0.5..20 step 0.05;

The default must be within the range, and the step must be positive. Slider
changes update a separate binding vector. They do not rewrite source.

## Declarations and expressions

`axis` must produce `Vec2`; `field` must produce `Field<Scalar>`;
`output` must produce `Field<Color>`. Exactly one output is required.
The spelling `field name: Scalar` is shorthand for
`field name: Field<Scalar>`. All named scalar fields are selectable in
Studio. A preview request can select one without changing the declared
output.

Components are pure, typed expressions. A call expands into the lowered
graph, while its definition and the call remain visible in the structured
program:

    component stripe(p: Field<Point2>, axis: Vec2,
                     frequency: Frequency, phase: Angle) -> Field<Scalar> =
        wave(dot(p, axis), frequency, phase);

Components cannot recurse. Definitions also cannot form cycles. The compiler
checks names, types, IDs, parameter ranges, palette stops, and graph size
before rendering. Unused component bodies are checked too.

The built-ins in version 1 are:

| Expression | Meaning |
| --- | --- |
| `vec2(x, y)`, `point2(x, y)` | Uniform vector and point constructors |
| `rotate(value, angle)` | Rotate a vector or a coordinate field |
| `dot(point_field, vector)` | Pointwise dot product, yielding a scalar field |
| `length(point_field)` | Pointwise distance from the origin |
| `wave(coordinate, frequency, phase)` | `sin(2π × frequency × coordinate + phase)` |
| `tanh(field)` | Pointwise hyperbolic tangent |
| `palette(palette_name, field)` | Map a scalar field to a color field |

`+`, `-`, and `*` operate on supported scalar and scalar-field
combinations, with pointwise lifting when a field is involved.
`Field<Point2> - Point2` translates coordinates by a constant point.
There are no implicit conversions between `Scalar`, `Angle`, and
`Frequency`.

A palette contains at least two RGB stops, with each channel in `[0, 1]`:

    palette ink = smooth [rgb(0.05, 0.1, 0.2), rgb(0.9, 0.8, 0.6)];

Palette input is clamped from `[-1, 1]` to the stop interval. Each
adjacent pair is interpolated channel by channel using
`t²(3 - 2t)`. The RGB channel values are written directly to 8-bit
output with rounding. Scalar previews map −1 to black and +1 to white.

## Current boundary

Version 1 has no imports, mutable state, loops, recursive definitions,
text editing in Studio, or syntax for linear/Bézier elements. A future
composition vocabulary can add named geometric components and lower their
coverage or distance to fields, while keeping the source program, stable
IDs, runtime bindings, and render-request model intact. Language versioning
gives those additions a place to define their semantics explicitly.
