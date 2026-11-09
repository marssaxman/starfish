# The curve-family generator experiment

Generate a study and browse it in the native gallery:

```sh
cargo run -- generate generated/curves-42
cargo run -- gallery generated/curves-42
```

Defaults are batch seed 42 and twelve pairs. Optional arguments are a decimal
unsigned 64-bit seed and a pair count from 1 to 64:

```sh
cargo run -- generate generated/another-study 123 24
```

The destination must be new. Generated studies are ignored by Git. Generation
validates and renders all candidates before creating the output directory.

## Seed → plan → components → composition → source

The generator is a pure module: it constructs a `DesignPlan`, then emits ordinary
version-1 Starfish source. It does not render or write files. The batch command
uses the existing compiler and CPU renderer; the native gallery is another client
of the same APIs.

The source-based exploration policy additionally supports structural arrangements:
enable **Composition** in Studio or use `cargo run -- arrange program.sf session`.
It builds reflected, repeated, alternating, rotational, nested, and interrupted
families from an existing construction while retaining its shared controls.
See [curve arrangements](EXPLORATION.md#curve-arrangements).

A coordinated plan chooses a scene frame, base frequency, shared phase, anchor,
palette, composition, and a parent cubic gesture. It instantiates two to four
relatives of that gesture:

- Parent gestures come from four shapes: a sweep, an arch, an S bend, and a shallow
  curve. Control coordinates vary within bounded ranges; their x coordinates
  remain ordered to avoid tight loops in the initial vocabulary.
- Each relative opens or tightens the parent's two handles. These concrete control
  points and the family variation remain in the plan and source comments.
- Orientations relate through a shared crossing angle of 30°, 60°, or 90°.
  Instance scales are 1×, 1½×, ½×, and optionally 2×. Phases advance by quarter
  cycles. Placements use the origin, anchor, and its mirror.
- Composition multiplies the first two layers and adds other relatives, adds
  every layer, or modulates the first two with the remaining layers. Instance
  weights are explicit.

Profiles cycle through sine, triangle, saw, pulse, additive harmonics, and
coordinate modulation, from a seed-dependent starting profile. Every pair uses
at least two distinct profiles. A separate random stream chooses the starting
profile and detail values. Pulse width, detail, scale, weight, and all eight
control coordinates are exposed per instance. Each pair keeps its profile and
detail choices identical on both sides.

The harmonic component combines a 0.7-weight fundamental with second and third
harmonics weighted by `0.2 × detail` and `0.1 × detail`. The modulated component
offsets its coordinate by `0.14 × detail` times a half-frequency sine, with a
quarter-cycle phase offset.

## Texture across the whole field

Each curve exposes along/across coordinates and unsigned distance. Distance bends
a global linear coordinate, rather than masking a stroke or ribbon. The resulting
wave field stays active throughout the image. A second coordinate combines curve
distance with a crossing linear coordinate, extending variation beyond endpoints.
A slow along-curve wave modulates the finer grain.

Each plan chooses one of three texture recipes:

- **Etched:** fine crosswise waves with a slower variation in their strength.
- **Braided:** crossing grain and a second carrier interact multiplicatively.
- **Rippled:** a carrier modulates fine crosswise waves, with an added overtone.

The broad carrier also receives a small grain-driven coordinate modulation.
Fine detail is added to each motif before layers are composed. **Bend** controls
the broad sweep; **Texture amount** controls fine detail and its coordinate
modulation; **Texture scale** controls its relative frequency. Individual
**Detail N** values also affect harmonic/modulated carriers where applicable.
These are ordinary wave expressions and reusable components in version-1 source,
with no new language built-ins or rendering special cases.

These are choices in the plan, rather than relationships inferred from an
anonymous expression graph. Source comments retain the plan choices. Named
`oriented_N`, `placed_N`, and `curve_N` frames, plus `along_N`, `across_N`,
`distance_N`, `sweep_N`, `coordinate_N`, `cross_coordinate_N`, `grain_N`,
`motif_N`, `texture_N`, and `layer_N` fields remain inspectable after compilation.
`composition` is the final scalar field; `image` is the declared color output.

Frequency ratios scale the coordinate field, phases use uniform angle addition,
and placements use coordinate translation. Rotation, frequency, phase, bend,
texture amount, and texture scale affect all instances.

Enable **Curve guides** in Studio to see the curves over the selected field.
Select a curve and drag its points; the guides account for rotation and placement.
Generated control points have direct parameter bindings, so all can be edited.
The family relationships describe generated defaults; an individual handle edit
can deliberately depart from them. Binding changes remain separate from saved
source defaults. The handwritten [Curve texture study](../programs/curve_textures.sf)
instead shares one set of gesture parameters between two instances, so its handle
edits propagate through the family.

## What each pair controls

The independent plan is derived from its coordinated partner. It keeps the same
motif count, waveform profiles, detail values, texture recipe, texture controls,
bend, composition, weights, palette, scene rotation, base frequency, and global
phase. It independently samples each motif's gesture from the same shape
vocabulary, plus orientation, scale, phase offset, and placement. Scale sampling
uses 80–120% of each instance's coordinated scale, preserving broad scale bands
while dropping exact harmonic constraints.

Both sources use the same field operations and have the same per-pixel operation
budget. Uniform constant nodes can differ because negative literals introduce
negation nodes. This experiment compares constraints on relative choices;
global scene controls and component reuse remain present in both conditions.
Matching graph operations does not match render time: adaptive curve subdivision
can produce different segment counts for different gestures.

Gesture types are discrete choices with continuous coordinate variation, and
independent transforms are sampled from continuous ranges. This is an exploratory
comparison, not a statistically matched test of correlations alone.
It also does not implement the expression-tree, mutation, or CPPN baselines from
the longer novelty review. Those are possible later experiments.

Compare before inspecting programs, since source comments reveal the condition.
Useful questions are whether motifs recur recognizably, whether distant areas
feel related, whether fine texture supports the larger sweeps, whether several
scales coexist, and whether inspecting the program explains something about its
image. A/B assignment is shuffled per pair; neither position identifies a
condition. The gallery records preference judgments with snapshots in the study's
`feedback/` session. These comparison records are marked separately from local
variation rounds. See [the exploration guide](EXPLORATION.md).

## Artifacts and reproduction

Each study contains:

- `pair-NN-a.sf` and `pair-NN-b.sf`: self-contained programs.
- Matching `.ppm` files: 256 × 256 final previews from the CPU renderer.
- `pairs.tsv`: neutral pair numbers, derived seeds, and source filenames.
- `answers.tsv`: the coordinated/independent condition for each slot.
- `README.txt`: batch seed, generator version, and comparison instructions.

Open any source directly with `cargo run -- path/to/pair-NN-a.sf`, or render an
intermediate without the gallery:

```sh
cargo run -- render generated/curves-42/pair-01-a.sf texture_0 /tmp/texture.ppm
```

The source includes batch seed, pair number, slot, derived pair seed, condition,
generator version, and plan choices. The source itself is sufficient to load and
render an image even if the generator later changes.

`starfish-curves-v3` uses fixed SplitMix64 streams with separate domains for
planning, waveform selection, ablation, and slot assignment. The same version,
batch seed, and pair count produce identical filenames, source, and pixels on
the same runtime.
Increasing the count preserves earlier pairs. Floating-point rendering follows
the existing CPU semantics; cross-platform bit-for-bit image parity is not
promised. Future changes to synthesis behavior should change the generator
version. Repeating a study requires a new destination directory.

Earlier `starfish-relations-v1` and `starfish-waveforms-v2` sources still load
unchanged. Their identities describe the sine-only and richer waveform studies.
Version 3 uses its own identity because the same seed now produces curve families
and textured fields. The source language remains version 1.
