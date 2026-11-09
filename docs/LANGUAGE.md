# Starfish source language, version 1

A .sf file is UTF-8 source for one self-contained image program. See
[Curve texture study](../programs/curve_textures.sf),
[Bézier wave study](../programs/bezier_study.sf),
[Wave laboratory](../programs/wave_lab.sf),
[Shared tides](../programs/shared_tides.sf), and
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
**Keep image** makes a new source snapshot with the current bindings as parameter
defaults. The original file and declared final output remain intact; the selected
preview output is recorded separately in the exploration session.

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
| `scale(point_field, vector)` | Multiply coordinate x/y by the corresponding uniform vector components |
| `bezier(point_field, start, handle_a, handle_b, end)` | Curve-relative coordinate field for a cubic Bézier |
| `abs(value)` | Absolute value of a `Scalar` or scalar field |
| `dot(point_field, vector)` | Pointwise dot product, yielding a scalar field |
| `length(point_field)` | Pointwise distance from the origin |
| `wave(coordinate, frequency, phase)` | `sin(2π × frequency × coordinate + phase)` |
| `triangle(coordinate, frequency, phase)` | Piecewise linear triangular wave in `[-1, 1]` |
| `saw(coordinate, frequency, phase)` | Rising sawtooth in `[-1, 1)` |
| `pulse(coordinate, frequency, phase, width)` | `+1` for the first `width` fraction of each cycle, `−1` otherwise |
| `tanh(field)` | Pointwise hyperbolic tangent |
| `palette(palette_name, field)` | Map a scalar field to a color field |

`+`, `-`, and `*` operate on supported scalar and scalar-field
combinations, with pointwise lifting when a field is involved.
`Field<Point2> - Point2` translates coordinates by a constant point.
There are no implicit conversions between `Scalar`, `Angle`, and
`Frequency`.

## Waveforms and modulation

All four waves take a scalar coordinate field, a uniform `Frequency`, and a
uniform `Angle`. Phase adds to the wave's argument: increasing it by 90° moves
the profile forward by a quarter cycle. Wrapping works for negative coordinates
and frequencies too. `wave` keeps its original sine semantics.

Let `c = frequency × coordinate + phase / (2π)` and `fract(c) = c − floor(c)`.
The exact profiles are:

- `triangle`: `1 − 4 × abs(fract(c + ¼) − ½)`. Like sine, it starts at zero
  rising, reaches +1 at ¼ cycle, and −1 at ¾ cycle.
- `saw`: `2 × fract(c + ½) − 1`. It starts at zero rising, jumps from +1 to
  −1 at ½ cycle, and takes the value −1 at the jump.
- `pulse`: +1 when `fract(c) < width`, otherwise −1. The uniform `Scalar`
  width must be finite and in `[0, 1]`. Width 0 is constant −1; width 1 is
  constant +1. At width ½ it is a square wave.

Pulse-width domains are checked when preparing the selected render output,
including computed widths and runtime bindings. Saw and pulse have hard edges;
the renderer samples pixel centers without filtering, so fine patterns can alias.

Richer signals use ordinary scalar expressions and reusable components:

    field c: Scalar = dot(world, vec2(1, 0));
    field harmonics: Scalar = 0.7 * wave(c, 3cycles, 0rad)
        + 0.2 * wave(2 * c, 3cycles, 0rad)
        + 0.1 * wave(3 * c, 3cycles, 0rad);
    field modulator: Scalar = wave(dot(world, vec2(0, 1)), 1cycles, 0rad);
    field modulated: Scalar = wave(c + 0.15 * modulator, 3cycles, 0rad);

The first expression adds harmonics at 1×, 2×, and 3× frequency. The second
offsets the carrier coordinate with another wave, producing local phase changes
without introducing a field-valued angle. Multiplication of two fields supplies
amplitude modulation. All intermediate fields remain selectable in Studio.

## Bézier coordinates

`bezier` takes a coordinate field and four uniform `Point2` controls. The first
and last points are endpoints; the middle points are tangent handles. It returns
a `Field<Point2>` that can be named with `frame`, transformed, and projected
using ordinary `dot` calls:

    frame curve = bezier(world, point2(-0.7, -0.6), point2(1.1, -0.6),
                         point2(-1.1, 0.6), point2(0.7, 0.6));
    field along: Scalar = dot(curve, vec2(1, 0));
    field across: Scalar = dot(curve, vec2(0, 1));
    field distance: Scalar = abs(across);
    field sweep: Scalar = dot(world, vec2(1, 0)) + 0.7 * distance;
    field pattern: Scalar = wave(sweep, 5cycles, 0rad);

The curve coordinates are defined throughout the plane. The example uses the
continuous distance to bend a global coordinate, filling the image with waves.
The along coordinate can also drive patterns directly, although changes in the
nearest curve location can introduce seams away from the curve.

For the nearest location on the finite curve:

- x is approximate arc length from the start, minus half the curve's total
  length. It is zero at the halfway distance and clamps at each endpoint.
- y has the magnitude of distance to that location, positive on the left of
  the directed curve and negative on the right. Beyond an endpoint its magnitude
  includes the distance past that endpoint.

Both coordinates use the input frame's units. Scaling `along` changes repeat
spacing; `abs(across)` supplies a continuous distance field. A curve
whose controls all coincide yields x = 0 and a nonnegative distance to that
point. Repeated handles and coincident endpoints are supported. At a zero
tangent the nearest sampled segment supplies the side direction.

This prototype prepares an adaptive polyline from the uniform controls once
per render/binding set, targeting 0.0005 input units of flatness, with at most
ten subdivision levels. The nearest segment seeds up to five safeguarded
Newton steps on the cubic; the polyline supplies approximate arc length.
Results are numerical approximations. Equal-distance segment ties start from
the first segment in traversal order. At folds or self-intersections, nearest
locations and side signs may change abruptly; the side sign also changes
across the tangent extension past an endpoint. Symmetric `abs(across)` patterns
avoid that endpoint sign seam. Controls and prepared curve values must be finite.

Coordinate scaling supports reflection and different scales along each axis:

    frame reflected = scale(world, vec2(-1, 1));
    frame compressed = scale(world, vec2(2, 0.5));

These transform sample coordinates: doubling x compresses the apparent image
horizontally. Negative factors reflect it. Zero factors collapse an axis.
Factors must be finite, including computed factors and runtime bindings.

Studio's optional **Curve guides** overlay displays curves reachable from the
selected output. It maps control points back through rotation and translation
to their world positions, including curves instantiated through components.
Scaling and reflection are included when mapping the guides back to world space.
Singular frames, such as an axis collapsed to zero, have no invertible guide.
Select a guide to expose its handles. Points constructed from two distinct direct
scalar parameters can be dragged; constants and computed coordinates remain
read-only in the overlay. Shared parameters update every instance that uses them.
Bindings change without rewriting source or changing the declared output.
Nested, non-affine curve coordinates have no world-space guide.

## Palettes

A palette contains at least two RGB stops, with each channel in `[0, 1]`:

    palette ink = smooth [rgb(0.05, 0.1, 0.2), rgb(0.9, 0.8, 0.6)];

Palette input is clamped from `[-1, 1]` to the stop interval. Each
adjacent pair is interpolated channel by channel using
`t²(3 - 2t)`. The RGB channel values are written directly to 8-bit
output with rounding. Scalar previews map −1 to black and +1 to white.

## Current boundary

Version 1 has no imports, mutable state, loops, recursive definitions,
text editing in Studio, multi-segment paths, or general shape/fill vocabulary.
Cubic Bézier frames supply local coordinates to the existing scalar operators;
the comparison generator currently continues to use stripes and rings.
