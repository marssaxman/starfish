# Save discoveries and explore relatives

Run `cargo run`, adjust a curve or slider, and click **Keep image**. This saves
a new, self-contained Starfish source with the current bindings as its defaults.
It also records the selected preview output, so a saved intermediate reopens
as the same grayscale image. The loaded source file is never overwritten.

**Make variations** saves the current working image as a parent, then generates
a set of related candidates in the background. The default is six pairs,
with **Texture** enabled and the other features fixed. The parent does not
have to be marked as a favorite to become a branch point.

In the comparison view, change the controls at the top and click **Regenerate**
to generate a new batch from the same saved parents. It uses the current Vary
checkboxes, Amount, Pairs, and Seed without requiring A/B judgments. Earlier
batches and their judgments remain in History. For imported study comparisons,
the saved compared images serve as the initial parents.

For each pair, choose **A**, **B**, **Both**, or **Neither**. Choices are saved
immediately. Click **Inspect A/B** or an image to return to Studio, adjust it,
keep it, or generate another branch. **Keep A/B** marks a candidate as a favorite
directly from the comparison. Inspect an image before using the Studio toolbar's
Keep image or Make variations commands.
Use **← Back to comparisons** at the top left of Studio to return to the same
batch and its saved judgments. The working image remains available through the
**Studio** tab.

Click **Explore picks** to generate the next round directly from your judgments.
**A** and **B** choose that side as a parent, **Both** retains both families, and
**Neither** or an unreviewed pair contributes no parent. Pairs cycle through the
selected families; when there are more selected parents than requested pairs,
the seed rotates the starting family. Each new pair compares two siblings from
one parent. The saved round records the exact selected parent pool, even if you
later change a judgment in the preceding round.

**History** opens earlier images and comparison rounds. Stars identify images
you kept; **Kept images only** filters the list. Each variation names its parent
and seed. Navigating to a saved image preserves the current working image as a
snapshot first. Use Keep image to explicitly save edits before closing the app.

## Variation controls

Checked features can vary; unchecked features remain fixed:

| Feature | What can change |
| --- | --- |
| Gesture | Control coordinates, rotation, bend, crossing angle, and spread |
| Texture | Frequency, phase, detail, modulation, pulse width, relative scale, and sine/triangle/saw profile choices |
| Palette | A shared RGB shift across each palette's stops, preserving their order |
| Composition | Reflected, repeated, alternating, rotational, nested, or interrupted families, with different instance counts and combinations |

**Amount** controls numeric perturbations and the chance of changing waveform
profiles. Composition choices are discrete alternatives. For Bézier programs
with a scalar or direct palette output, Composition constructs a reusable motif
from the current saved construction and changes its arrangement. The underlying
gesture, texture, palette, and internal weights stay fixed unless their own
controls are edited. For programs without curves, Composition retains its
earlier layer-weight and ingredient-combination variations.

## Curve arrangements

To compare arrangements of the same construction, enable **Composition**, leave
**Gesture**, **Texture**, and **Palette** unchecked, and click **Regenerate** or
**Make variations**. A twelve-image batch cycles through all six rule types:

- **Reflected family:** two or four instances reflected across shared axes.
- **Repeated sweeps:** translated instances with common orientation and scale.
- **Alternating family:** repeated positions with alternating reflection, scale,
  and field polarity.
- **Rotational family:** three or four instances at related angles around a center.
- **Nested echoes:** progressively scaled and turned instances of one motif.
- **Interrupted repetition:** a repeated family with one member displaced, turned,
  resized, and reversed in polarity.

Counts range from two to four. The mixture can add, weave, or modulate the
instances; reflected families use equal contributions to retain reflection.
Every instance remains a field over the whole image. These rules do not mask
curves into bounded strokes.

The saved source contains reusable, coordinate-parameterized components and
named `layout_frame_N` and `layout_instance_N` declarations. The rule and count
are recorded in a versioned `@starfish-arrangement-v1` source comment. Shared
spacing, rotation, and coordinate-scale parameters remain editable in Studio.
The preview names the arrangement, and **Curve guides** exposes all its curves.
Dragging a handle updates every related instance through shared bindings.

With Composition unchecked, subsequent texture, palette, and gesture variations
retain the current arrangement, including edited layout controls. With Composition
enabled, a new arrangement can replace it and add or remove instances. Each
generation rebuilds the related instances from the saved underlying motif,
preventing an ever-growing stack of copied arrangements.

The parameter groups recognize the names emitted by the generator and the bundled
examples. Control parameters use `curve_N_…`, `start_…`, `end_…`, and `handle_…`;
`rotation`, `bend`, `crossing`, and `spread` are also gestures. `weight_…` and
`…_strength` belong to composition. Other numeric parameters belong to texture.
Stable IDs are preserved even when their spelling differs from the source name.

Curve instances receive common perturbations for corresponding control points,
and relative frequency scales receive a common multiplier. This retains family
relationships where parameter ranges permit. Changes stay inside declared
ranges; invalid and pixel-identical candidates are rejected. If the enabled
features cannot produce enough distinct images, the UI reports the failure
instead of filling a comparison with duplicate candidates.

Variation source remains ordinary version-1 Starfish. Its header records the
variation policy, seed, enabled groups, and amount. Older generator comments
describe ancestry; the current declarations are authoritative.

## Sessions and reopening

The default session lives in `explorations/default`. Launching `cargo run` resumes
its last saved image or comparison. Opening an explicit source path starts with
that source and uses the default session for subsequent exploration actions.
Merely opening a source does not create a session.

Start or resume a named session:

```sh
cargo run -- explore explorations/my-session
```

Create a batch without opening a window:

```sh
cargo run -- vary programs/curve_textures.sf explorations/first-study 42 6
cargo run -- explore explorations/first-study
```

The optional seed and pair count default to 42 and 6. Counts range from 1 to 12.
This command uses the same default texture-only variation policy as Studio.
It can append a new branch to an existing session without overwriting records.

For composition-only variations, use:

```sh
cargo run -- arrange programs/curve_textures.sf explorations/arrangement-families-42 42 6
cargo run -- explore explorations/arrangement-families-42
```

This holds the gesture, texture, and palette fixed while exploring arrangements.

The older coordinated/independent comparison gallery also records judgments:

```sh
cargo run -- gallery generated/curves-42
cargo run -- explore generated/curves-42/feedback
```

Its feedback session is created on the first judgment or exploration action.
Both judged programs are copied into the session before a vote is recorded,
so later changes to the original study cannot change what was judged. Study
comparisons are marked separately from local variation rounds.

## Saved artifacts and reproducibility

A session is a directory with a versioned `session.txt` marker and immutable
record directories:

- `c-…/program.sf`, `preview.ppm`, and `meta.tsv`: one candidate, its selected output,
  parent ID, proposal seed, and policy identity.
- `r-…/meta.tsv`: ordered A/B candidate IDs, parent pool, batch seed, variation settings,
  and comparison context.
- `e-…/meta.tsv`: a keep, navigation, or preference event. Preference events
  identify the exact round and pair, plus A, B, Both, or Neither.

Records are written to a temporary directory and published by rename after
their files are complete. Interrupted unpublished directories are ignored.
Preference corrections append new events; the latest judgment is displayed
while earlier judgments remain available.

Saved defaults round-trip the runtime values exactly, including angles in
radians. Sliders preserve those values until edited. The selected preview output
does not change the program's declared final output: render an intermediate
using its ID from the candidate metadata. Previews are 192 × 192; source programs
can be rendered at any supported resolution.

The same parent source and selected output, variation policy version, settings,
seed, and pair count reproduce the same accepted source programs and pixels
on the same runtime. Candidate IDs identify saved occurrences and intentionally
differ across runs. The current policy is `starfish-variations-v2`, which adds
structural arrangements and cycles relationship types across composition batches.
Earlier saved sources and session histories continue to load; regeneration uses
the current policy. Child records retain the actual proposal seed, including
deterministic retries, so individual proposals can be replayed.

Copy a whole session folder to move or back up the history. Local `explorations/`
and generated studies are ignored by Git. No model is trained yet: your choices
steer exploration by selecting the parents you branch from, and the recorded
judgments provide data for later preference learning. Explore picks already uses
those judgments to choose the parents of the next round.
