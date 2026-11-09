# Codex task: Starfish Studio native-Linux exploratory slice

Implement a very small but functional prototype of **Starfish Studio**.

This task is intentionally time-boxed to roughly 20 minutes of implementation
effort. Prefer a working, coherent toy over infrastructure or architectural purity.

Before coding, read:

- `docs/Starfish System Architecture.txt`
- `docs/Artistic Novelty Review.txt`

Those files describe the intended long-term architecture and the reason the Studio
matters. They are background, not a mandate to implement the whole system.

## Non-negotiable platform constraint

This is a native desktop application for Linux.

Do **not** use:

- JavaScript or TypeScript
- Node/npm
- a browser UI
- Electron
- Tauri
- a webview
- HTML/CSS
- a local web server

Use **Rust + eframe/egui** unless the repository already contains a clearly better
native Rust GUI scaffold.

The program should build to an ordinary Linux executable with Cargo.

For this spike, egui's own widgets are acceptable even though they are not GTK
native widgets. Do not spend the time budget replacing egui with GTK. Keep domain
logic independent enough that a later GTK front end would be feasible.

## Goal

Produce a Studio where I can:

1. see a procedural image;
2. see the small functional computation that produces it;
3. select intermediate outputs and render them;
4. manipulate meaningful parameters with sliders and immediately see the image
   change;
5. return to the final output;
6. inspect enough information about the selected computation to understand what I
   am looking at.

The intended first impression is:

> This image is a little program, and I can poke around inside that program.

Do not implement generation, evolution, search, training, a textual language, a
parser, shader compilation, or WebGPU.

## Repository behavior

First inspect the repository.

- If a Cargo workspace or Rust app already exists, integrate with it.
- Otherwise create the smallest normal Cargo binary crate needed for this prototype.
- Keep dependencies minimal.
- `cargo run` should launch the Studio.
- `cargo check` must pass before finishing.

Do not introduce a frontend build system.

## Prototype program model

Represent one tiny Starfish program directly as Rust data.

The exact types are your decision, but the model should preserve these concepts:

```rust
enum ValueType {
    Scalar,
    Color,
}

struct Parameter {
    id: &'static str,
    name: &'static str,
    value: f32,
    initial: f32,
    min: f32,
    max: f32,
    step: f32,
}

struct Node {
    id: &'static str,
    name: &'static str,
    op: &'static str,
    value_type: ValueType,
    inputs: Vec<&'static str>,
}

struct StarfishProgram {
    name: &'static str,
    parameters: Vec<Parameter>,
    nodes: Vec<Node>,
    output: &'static str,
}
```

Change this shape freely if another representation is simpler.

This is **not** the final Starfish IR.

Stable node IDs matter. The Studio should select an output by stable node ID.

## Sample image program

Hand-author one deterministic sample program with approximately this structure:

```text
x,y
 │
 └── rotate ─────────── x', y'
                         │
             ┌───────────┴───────────┐
             │                       │
             ▼                       ▼
       sine pattern A          sine pattern B
             │                       │
             └──────── combine ──────┘
                         │
                         ▼
                   interference

x',y' ─────────── radial field

interference + radial field
             │
             ▼
          modulate
             │
             ▼
           field
             │
             ▼
          palette
             │
             ▼
           color
```

Expose at least:

- rotation
- frequency A
- frequency B
- radial frequency
- interference strength
- radial strength
- phase

Use normalized continuous coordinates around `[-1, 1]`.

The default image should have clearly visible large- and small-scale structure.
Parameter changes should be continuous and visually obvious.

No rendering randomness. Reloading must reproduce the same image.

## Evaluation and rendering

Implement a tiny CPU evaluator.

For each pixel:

```text
pixel
  -> normalized x,y
  -> evaluate requested node/output
  -> convert result to display RGB/RGBA
```

A render request conceptually looks like:

```rust
render(
    &program,
    &bindings,
    RenderRequest {
        output: selected_node_id,
        width: 512,
        height: 512,
    },
)
```

The implementation does not need this exact API, but preserve the separation.

Important:

- Selecting an intermediate node must **not** modify the program's final-output
  declaration.
- Runtime parameter values should live in bindings/UI state rather than being
  destructively rewritten into graph structure.
- Evaluation/rendering code must not live inside egui widget code.

Use a modest render resolution, around 400–512 square.

A scalar node should render as grayscale.

The final color node should render as color.

If vector-valued intermediate nodes make the prototype materially more complex,
do not expose them as selectable outputs.

## Studio UI

Use a three-pane layout that fills the window:

```text
┌──────────────────┬──────────────────────────┬──────────────────────┐
│ PROGRAM          │                          │ INSPECTOR            │
│                  │                          │                      │
│ Final Image      │        PREVIEW           │ selected node        │
│ Pattern A        │                          │ operation            │
│ Pattern B        │                          │ inputs               │
│ Interference     │                          │ type                 │
│ Radial           │                          │                      │
│ Field            │                          │ PARAMETERS           │
│ Palette          │                          │ sliders...           │
│                  │                          │                      │
│                  │                          │ diagnostics          │
└──────────────────┴──────────────────────────┴──────────────────────┘
```

Do not implement a graphical node editor.

### Left: program/output list

Show:

- `Final Image`
- the useful intermediate outputs

Each selectable row should show a readable name. Showing the operation in smaller
text is useful but optional.

Clearly indicate the selected row.

Clicking `Final Image` sets the preview output back to the program's declared final
output.

### Center: preview

Display the selected rendered output.

Also show:

- program name
- selected output name
- render dimensions

Preserve square image aspect ratio.

### Right: inspector

For the selected node show:

- name
- stable ID
- operation
- value type
- input node names

Below this, show sliders for all exposed program parameters.

Each slider should display its numeric value.

Slider movement should trigger rerendering.

If trivial, add a reset button for each parameter or a single `Reset parameters`
button. Otherwise skip it.

## Interaction requirements

These should visibly work:

### Inspect Pattern A

Selecting Pattern A shows its scalar output as grayscale.

### Inspect Radial

Selecting Radial shows concentric/radial structure as grayscale.

### Return to final output

Selecting Final Image restores the composed color image.

### Rotate

Changing rotation visibly rotates the shared coordinate frame and therefore
multiple structures together.

### Change frequencies

Changing frequency A or B visibly changes the corresponding pattern.

### Rebalance composition

Changing radial/interference strengths alters their relative contribution to the
final composition.

This shared rotation behavior is important: even this toy should hint that multiple
parts of the program can participate in one coordinated visual relationship.

## Rendering policy

Simple and responsive beats sophisticated.

It is acceptable to rerender the entire 512x512 image whenever a slider changes.

Use `std::time::Instant` to measure render duration.

Convert the generated pixel buffer into whatever egui texture/image representation
is simplest.

Avoid unsafe code unless there is a compelling reason.

## Minimal diagnostics

Show something like:

```text
render: 14.8 ms
nodes: 8
selected: radial
```

No profiling system is required.

## Suggested organization

Keep the project small. Something approximately like:

```text
src/
    main.rs
    program.rs
    evaluate.rs
    render.rs
    studio.rs
```

Fewer files are fine.

The important boundary is:

```text
program model
     ↓
evaluator/render service
     ↓
Studio UI
```

The UI owns interaction state, not Starfish semantics.

## Explicit non-goals

Do NOT implement:

- textual Starfish syntax
- parser
- compiler passes
- WGSL
- WebGPU
- GPU rendering
- CPU/GPU semantic parity
- optimizer
- random program generation
- design genomes
- global generator/coordinator
- mutation
- evolutionary search
- image analyzer / FFT metrics
- machine learning
- aesthetic scoring
- persistence
- undo/redo
- project files
- graph auto-layout
- graphical graph editing
- drag and drop
- server/backend
- database
- plugin system

If tempted to implement any of these, don't.

This spike is an instrument panel, not the engine factory.

## One architectural seam worth preserving

The background architecture makes intermediate rendering a first-class operation.
Preserve that now.

Conceptually:

```text
StarfishProgram
      +
ParameterBindings
      +
RenderRequest(selected output ID)
      ↓
renderer/evaluator
      ↓
pixel image
```

That seam matters more than whether the toy node structs resemble the eventual IR.

## Visual character

Do not make this look like a web dashboard transplanted into a desktop window.

Prefer:

- compact controls
- restrained dark/neutral presentation
- modest padding
- clear hierarchy
- an image-first workspace
- monospaced IDs/diagnostics where appropriate

Do not spend much time theming.

The goal is a technical creative instrument.

## Acceptance criteria

When I run:

```bash
cargo run
```

I should quickly reach a desktop window containing a procedural image.

Without reading the Rust source I should be able to discover that:

1. the image is composed from several named computations;
2. clicking them reveals intermediate images;
3. the intermediate images intuitively resemble ingredients of the final image;
4. sliders alter the image interactively;
5. rotation affects several parts coherently rather than independently;
6. Final Image recombines the ingredients;
7. the program feels like the embryo of a procedural-image IDE, not a random-image
   demo.

## Tests

Do not add a testing framework.

If cheap, add ordinary Rust unit tests for:

- deterministic evaluation at a known coordinate;
- a changed binding causing a changed sampled value;
- a simple known operator result.

Skip tests rather than expanding the time budget.

## Finish

Before stopping:

1. run `cargo fmt`;
2. run `cargo check`;
3. if practical, run the app and fix obvious startup failures;
4. summarize what was implemented;
5. list shortcuts that matter if this prototype is extended.

Make reasonable implementation decisions yourself. Do not stop to ask about minor
details.
