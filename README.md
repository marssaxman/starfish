# Starfish Studio

A native Linux workspace for exploring continuous image programs. The first
prototype was a single image authored in Rust. Studio now loads versioned
Starfish source files, validates them, and renders named intermediate fields
through the same CPU evaluator used by the command-line renderer.

## Run the Studio

From this directory, with Rust 1.88 or newer and a Linux desktop session:

```sh
cargo run
cargo run -- programs/crosscurrent.sf
```

The first command opens [Curve texture study](programs/curve_textures.sf), a
pair of related cubic curves shaping interacting wave fields. The second
opens [Crosscurrent](programs/crosscurrent.sf). In the left pane, type a source
path and press **Open**, or use the sample buttons. An invalid file leaves
the current program open and shows the validation error. Select a named field
to inspect its grayscale output, then select **Final Image** to return to the
declared color output. The right pane shows the field expression, its direct
dependencies, program source, and live parameter bindings.
Frames, axes, and component definitions in the left pane can also be selected
for inspection while the image preview stays on the last selected field.

Enable **Curve guides** above the image to see the paths and control handles.
Choose a curve from the adjacent menu and drag its points. Guides follow shared
rotation and placement, and are available when inspecting generated programs too.
Dragging changes preview bindings; **Reset parameters** restores the saved defaults.
Computed control points are shown in gray and remain adjustable through their
parameters. Off-image handles can also be adjusted through sliders.

In Curve textures, both curves reuse one gesture, so dragging either changes
both. **Crossing** and **Spread** vary their relationship. Adjust **Bend**,
**Texture amount**, and **Texture scale**, or inspect **Etching**, **Braiding**,
and **Ripples** to see how fine detail relates to the broader waves.

The **Bézier study** button opens the earlier single-curve example.
Move **Start**, **End**, and **Handle a/b** x/y sliders to
reshape the curve. Adjust **Bend**, **Frequency**, **Cross frequency**, and
**Modulation** to change how the curve shapes the waves throughout the image.
Select **Along**, **Across**, **Distance**, **Sweep**, or **Modulated** to inspect
its fields. Curve distance bends a global coordinate, and crossing waves add
detail everywhere. Every pixel receives the resulting pattern.

The **Wave lab** button opens [Wave laboratory](programs/wave_lab.sf), with
sine, triangle, saw, pulse, harmonic, and modulated profiles. Select **Sine**,
**Triangular**, **Sawtooth**, or **Pulses** to compare profiles. **Harmonics**
adds second and third harmonics; **Modulated**
bends a carrier with a crossing wave. Adjust **Overtones**, **Modulation**, and
**Pulse width**, then return to **Final Image** to see their combined effect.
The **Shared tides** and **Crosscurrent** buttons open the earlier examples.

The first build downloads and compiles eframe/egui. X11 and Wayland are enabled.
On a minimal Debian/Ubuntu installation the typical prerequisites are
`build-essential`, `pkg-config`, `libx11-dev`, `libxi-dev`,
`libxrandr-dev`, `libxcursor-dev`, `libxinerama-dev`,
`libwayland-dev`, `libxkbcommon-dev`, and `libegl1-mesa-dev`.

## Render a saved program

```sh
cargo run -- render programs/shared_tides.sf final /tmp/shared-tides.ppm 512 512
cargo run -- render programs/crosscurrent.sf fine /tmp/fine.ppm 512 512
cargo run -- render programs/wave_lab.sf modulated /tmp/modulated.ppm 512 512
cargo run -- render programs/bezier_study.sf final /tmp/bezier.ppm 512 512
cargo run -- render programs/curve_textures.sf final /tmp/textures.ppm 768 768
```

`final` selects the program's declared output. A stable field ID such as
`fine` selects an intermediate. Width and height default to 512 if omitted.
The files use binary PPM (P6) to keep the CLI independent of image libraries.

## Generate a comparison study

```sh
cargo run -- generate generated/curves-42
cargo run -- gallery generated/curves-42
```

The first command creates twelve reproducible A/B pairs with batch seed 42.
Each pair combines two to four cubic curves using sine, triangle, saw, pulse,
harmonic, or modulated profiles, with etched, braided, or rippled detail.
The output directory must be new. Each pair includes two self-contained `.sf`
programs and their 256 × 256 PPM previews. The native gallery displays the
images with shuffled A/B labels; click an image or **Inspect** to open its
fields, source, and sliders in Studio. **Back to pairs** returns to the gallery.
The conditions are revealed in source comments and `answers.tsv`.

Within a pair, one program varies a common curve gesture with related orientations,
harmonic scales, phase offsets, and planned placements; the other samples gestures
and those relationships independently. Both use the same motif count, waveform
profiles, texture recipe and controls, bend, detail, composition, weights, palette,
and number of field operations. Across pairs, profiles, texture, motif count, and
composition also vary. Every pattern fills the image.

To choose another seed or pair count:

```sh
cargo run -- generate generated/study-123 123 12
cargo run -- generated/study-123/pair-01-a.sf
```

See [the generator experiment](docs/GENERATION.md) for the design plan,
reproduction rules, and what the comparison controls.

See [the language guide](docs/LANGUAGE.md) for syntax and semantics. The source
file is the authored artifact. The compiler retains named declarations and
component definitions for inspection while lowering them to a validated graph
for execution. Parameter values from sliders are separate runtime bindings;
they do not rewrite source or change the declared output.

## Checks

```sh
cargo fmt --check
cargo check
cargo test
```

Tests cover parsing and validation, the saved programs, deterministic wave
behavior, changed bindings, shared rotation, grayscale intermediate rendering,
phase and frequency semantics, pulse-width limits, the final color output,
invalid render requests, reproducible generated pairs,
matched field-operation budgets, and batch/gallery loading.
Curve checks include straight and curved coordinates, an independent dense
reference, finite endpoints, collapsed handles, shared rotation, live controls,
visible variation throughout the image, transformed guides, handle dragging,
shared gesture editing, and restoration of full preview resolution after a drag.

## Current scope

The language deliberately has a small field vocabulary: coordinate transforms,
sine/triangle/saw/pulse waves, Bézier coordinate fields, arithmetic, composition,
and palettes. Richer signals use additive harmonics and scalar coordinate
modulation. It supports
typed reusable components and named frames, plus a small deterministic
composition generator.
It has no imports, loops, recursive components, source editor, graph editor,
persistence of slider presets, or multi-segment paths yet.
Bézier curves currently enter through four uniform control points and expose
along/across coordinates through the existing field and component model.

Curve guides support world coordinates with rotation and translation. Curves
inside another curve's coordinate system render but have no world-space guide.
Control points with two distinct direct scalar parameter bindings can be dragged;
computed coordinates are read-only in the overlay.

Rendering runs synchronously at 448 × 448 in Studio, using 224 × 224 while
dragging controls and restoring full resolution on release. Color stops are normalized
RGB values and palette interpolation is specified in the language guide.
The Studio shows loaded source verbatim; it does not yet reformat or edit it.
The legacy application is independent background material.
