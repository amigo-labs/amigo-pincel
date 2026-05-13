---
id: lessons
title: Lessons & Skill Tree
status: draft
version: 0.1.0
owner: daniel
created: 2026-05-13
last_updated: 2026-05-13
related:
  - canvas
  - replay
  - i18n
  - cli
---

# Lessons & Skill Tree

## Overview

Pincel ships with an integrated learning system that teaches both **app usage**
(where is what tool) and **pixel art craft** (color theory, shading, clustering,
animation). Lessons are delivered as a versioned static web resource, rendered
inside the app as an interactive skill tree, and validated locally via the
shared Rust core. Lesson authoring happens exclusively in the repository — there
is no in-app editor.

This spec defines the data model, validation API, replay format, skill-tree
rendering, i18n strategy, CDN deployment, feedback loop, CLI tooling, and the
monorepo layout that ties them together.

## Goals

- **Differentiator**: a learning experience no other pixel editor offers,
  combining stroke replay, constrained practice canvases, programmatic
  validation, and a gamified skill tree.
- **Community-extensible**: lessons authored as plain markdown + TOML, reviewable
  via PR, no toolchain required for content contributors.
- **Offline-first**: foundation lessons precached on first launch; any visited
  lesson available offline thereafter.
- **Multilingual**: DE and EN at v1, additional locales via translation PRs.
- **Decoupled release cycles**: lesson updates ship via CDN without an app
  release.

## Non-Goals

- In-app lesson authoring (CLI only).
- Server-side state, accounts, or cloud sync.
- Live multiplayer or social features.
- Tutorial content licensed from third parties (all original).

## Curriculum

Five learning paths, organised as a skill tree (not linear). The Foundation
path is a prerequisite; Color, Form & Light, and Animation can then be tackled
in any order. Subjects requires Color and Form & Light.

### Foundation (5 lessons)
1. `what-is-a-pixel` — canvas, zoom, resolution, crisp rendering
2. `first-line` — pixel-perfect lines, jaggies, perfect-line ratios
3. `clusters-and-banding` — cluster theory, banding as the cardinal anti-pattern
4. `shapes-before-details` — silhouette test, readability at small sizes
5. `first-sprite` — end-to-end capstone (apple or slime)

### Color (6 lessons)
6. `why-limited-palette` — historical, artistic, practical rationale
7. `hue-shifting` — shadows shift hue, not just darken
8. `value-and-contrast` — squint test, greyscale preview
9. `saturation-control` — when saturated, when muted
10. `building-color-ramps` — ramps as families, ramp sharing
11. `palette-workshop` — capstone: build an 8-colour palette

### Form & Light (5 lessons)
12. `light-source` — one light source, consequently applied
13. `shadows-and-highlights` — core shadow, reflected light, specular
14. `materials` — metal, fabric, skin, wood in few pixels
15. `selective-outlining` — dropping outlines where light hits
16. `manual-anti-aliasing` — when yes, when no

### Subjects (8 lessons)
17. `trees-and-foliage` — cluster theory applied
18. `water-and-reflections` — animation-ready thinking
19. `rocks-and-stones` — texture without noise
20. `character-proportions` — chibi vs realistic head counts
21. `face-in-16px` — every pixel is a design decision
22. `tiles-and-tilesets` — seamless tiling, auto-tile prep
23. `ui-elements` — buttons, health bars, 9-slice thinking
24. `items-and-icons` — inventory readability

### Animation (5 lessons)
25. `frame-timing-and-loops` — why four frames are often enough
26. `idle-animation` — 2–3 frame breathing/bob
27. `walk-cycle` — 4 and 8 frame cycles
28. `smear-frames` — squash, stretch, motion smear
29. `effects` — explosion, slash, sparkle

**Total**: 29 core lessons. Estimated 5–8 hours from beginner to competent.

A user who completes Foundation, Color, Form & Light, and the
`character-proportions` + `face-in-16px` lessons can produce a complete
Threadwalker-compatible character sprite set in Pincel alone.

## Skill Tree

### Data Model

The skill tree is **inferred at build time** from the `requires` fields of all
lessons. There is no separate tree manifest — lessons are the single source of
truth.

Each lesson declares its position in the tree explicitly via
`tree_position = { x, y }`. Force-directed layouts look chaotic for curated
learning paths; hand-positioning is worth the small authoring cost.

### Node States

A lesson node is in exactly one of these states, derived from local progress:

| State | Condition |
|---|---|
| `locked` | At least one `requires` lesson is not `completed` |
| `available` | All prerequisites completed, lesson never attempted |
| `in_progress` | Lesson attempted, not all steps validated |
| `completed` | All required validations passed |
| `mastered` | All required + bonus validations passed |

### Rendering

- SVG, no external graph library
- Nodes positioned via the `tree_position` field
- Edges as simple cubic Béziers between parent and child
- Pan/zoom via a single CSS transform on a wrapper `<g>`
- Node visual encodes state (locked/available/in_progress/completed/mastered)
  via colour and icon overlay

### Path Grouping

Each path (`foundation`, `color`, `form-and-light`, `subjects`, `animation`)
has a colour theme and a bounding region in the tree. Path names are
localised; path identifiers are stable English strings.

## Lesson Format

Each lesson lives in its own directory under `packages/lessons/<path>/<id>/`.

```
packages/lessons/foundation/what-is-a-pixel/
  lesson.toml          # metadata, prerequisites, steps, validations
  content.de.md        # German content
  content.en.md        # English content
  reference.png        # optional, for overlay/diff
  solution.replay      # optional, stroke replay JSON
  thumbnail.png        # optional, skill-tree node icon
```

### `lesson.toml`

```toml
id = "foundation/what-is-a-pixel"
version = "1.0.0"
path = "foundation"
title_key = "lesson.foundation.what_is_a_pixel.title"
estimated_minutes = 10
tree_position = { x = 0, y = 0 }
requires = []
unlocks_achievements = ["first-steps"]

[canvas]
width = 16
height = 16
palette = "endesga-4"           # references packages/lessons/_palettes/endesga-4.toml
locked_tools = []               # empty = all tools available

[[steps]]
id = "intro"
type = "read"                   # read | draw | validate
content_anchor = "step-intro"   # links to a heading in content.{lang}.md

[[steps]]
id = "draw-line"
type = "draw"
content_anchor = "step-draw-line"
hints_after_attempts = [3, 6]   # show progressive hints after N failed attempts

[[steps.validations]]
fn = "has_line_at_angle"
args = { angle = 45.0, min_length = 8, tolerance_px = 0 }
message_key = "lesson.foundation.what_is_a_pixel.validation.line_45"
required = true

[[steps.validations]]
fn = "count_colors_used"
args = { op = "le", value = 2 }
message_key = "lesson.foundation.what_is_a_pixel.validation.max_colors"
required = false                # bonus → contributes to mastery, not completion
```

### Content files

Markdown files per locale, named `content.{locale}.md`. Lesson rendering
splits the markdown by `## step-*` anchor headings and pairs each section
with the matching `[[steps]]` entry from the TOML.

Markdown extensions supported by the renderer:
- `:::demo` blocks embed inline canvas demonstrations
- `:::replay file="solution.replay" speed="2x":::` embeds a replay player
- `:::canvas-snapshot id="reference":::` shows the reference image inline
- Standard markdown otherwise (headings, bold, lists, code blocks)

### Palettes

Palettes live in `packages/lessons/_palettes/` and are referenced by ID from
lessons. Format:

```toml
id = "endesga-4"
name_key = "palette.endesga_4.name"
attribution = "ENDESGA / lospec.com (CC0)"
colors = ["#1a1c2c", "#5d275d", "#b13e53", "#ef7d57"]
```

This makes attribution explicit and reuses common palettes across lessons.

## Validation API

Validations are **named Rust functions** in a shared registry. No DSL.

### Registry

```rust
// packages/core/src/validation/mod.rs
pub trait ValidationFn: Send + Sync {
    fn validate(&self, canvas: &Canvas, args: &ValidationArgs) -> ValidationResult;
}

pub struct ValidationResult {
    pub passed: bool,
    pub message_key: Option<String>,
    pub highlight_pixels: Vec<(u32, u32)>,
    pub metrics: HashMap<String, Value>,
}
```

### Initial Function Set

| Function | Purpose |
|---|---|
| `count_colors_used` | number of distinct non-transparent colours, with comparison op |
| `has_line_at_angle` | detect a straight line at a given angle, min length, tolerance |
| `is_pixel_perfect_line` | no double-pixel corners along a stroke |
| `cluster_analysis` | returns banding score, cluster size distribution |
| `silhouette_readability` | greyscale + downscale to N px, check edge contrast |
| `palette_matches` | all used colours come from a referenced palette |
| `has_symmetry` | horizontal/vertical/diagonal symmetry within tolerance |
| `coverage_ratio` | fraction of canvas with non-transparent pixels |
| `frame_count` | number of frames in an animation |
| `has_hue_shift` | shadow/highlight colours shift hue, not just value |
| `tile_seamless` | edges tile without seams horizontally/vertically |

Functions are registered via a `linkme`-style distributed slice so packages
can add new ones without editing a central match block. Lesson TOMLs are
validated at build time against the registry — unknown function names break CI.

### Result Reporting

Validation results carry pixel coordinates for highlight overlays. The UI
draws coloured markers on the canvas at those positions when a validation
fails, so users see *where* the rule was violated.

Results never contain localised strings. The UI layer resolves
`message_key` against the active locale.

## Stroke Replay

Pincel's canvas already uses a Command pattern for undo/redo. Replays are the
serialised command stream.

### Format

```json
{
  "schema_version": 1,
  "canvas": { "width": 16, "height": 16 },
  "palette": "endesga-4",
  "commands": [
    {
      "t": 0,
      "op": "draw_pixel",
      "x": 5, "y": 7,
      "color_idx": 2,
      "layer": 0,
      "annotation_key": "lesson.color.hue_shifting.step3.note"
    },
    {
      "t": 120,
      "op": "draw_line",
      "from": [3, 3], "to": [10, 3],
      "color_idx": 1,
      "layer": 0
    }
  ]
}
```

- `t` is milliseconds since replay start
- `op` mirrors the editor's command vocabulary
- `annotation_key` is optional; references a locale string for narration
- `color_idx` references the palette by index, keeping replays palette-agnostic

### Player

A small scheduler in the core re-executes commands against a blank canvas.
Controls: play, pause, step forward, step backward, speed (0.25x–4x), seek.

### Generation

Replays are generated by `pincel-lessons record` from an Aseprite source
file with undo history intact, or authored manually as JSON. The CLI extracts
the undo stack from the `.aseprite` binary format and translates it into the
replay command vocabulary.

## Achievements

Event-driven, evaluated lazily.

### Event Stream

The core emits structured events:

```rust
enum Event {
    PixelDrawn { x: u32, y: u32, color: Color, lesson_id: Option<String> },
    LessonCompleted { id: String, attempts: u32, duration_ms: u64 },
    LessonMastered { id: String },
    PaletteSizeChanged { from: u32, to: u32 },
    ValidationPassed { lesson_id: String, fn_name: String },
    ValidationFailed { lesson_id: String, fn_name: String },
    PathCompleted { path: String },
    // ...
}
```

Events are persisted in the local store (see Storage below).

### Achievement Definition

```toml
# packages/lessons/_achievements/cluster-master.toml
id = "cluster-master"
title_key = "achievement.cluster_master.title"
description_key = "achievement.cluster_master.description"
icon = "cluster-master.svg"
hidden = false                   # hidden until unlocked?
triggers = ["LessonCompleted", "ValidationPassed"]
check_fn = "check_cluster_master"
```

The `check_fn` is a Rust function evaluated against the event store when one
of the `triggers` fires. Functions live in
`packages/core/src/achievements/`.

### Initial Achievements

- `first-steps` — complete `what-is-a-pixel`
- `cluster-master` — complete 10 foundation+color lessons with zero banding
  warnings
- `hue-shifter` — complete `hue-shifting` with bonus validation
- `palette-architect` — build and save 5 custom palettes
- `tile-wizard` — pass `tile_seamless` on three different tiles
- `path-complete-foundation` / `path-complete-color` / etc. — finish a path
- `polyglot` — switch the app locale at least once
- (extensible via PRs)

## Storage

### Abstraction

```rust
#[async_trait]
pub trait LessonStore {
    async fn get_progress(&self, lesson_id: &str) -> Option<LessonProgress>;
    async fn set_progress(&self, lesson_id: &str, progress: LessonProgress) -> Result<()>;
    async fn get_achievements(&self) -> Vec<AchievementUnlock>;
    async fn unlock_achievement(&self, id: &str) -> Result<()>;
    async fn append_event(&self, event: Event) -> Result<()>;
    async fn query_events(&self, filter: EventFilter) -> Vec<Event>;
}
```

### Backends

- **PWA**: Dexie.js on IndexedDB. WASM-Rust bridges to JS via a thin
  wasm-bindgen layer. Schema: `lesson_progress`, `achievements`, `events`.
- **Tauri**: SQLite via `tauri-plugin-sql`. Same schema, native access.

The trait keeps the rest of the codebase backend-agnostic.

## i18n

### Code Language

English throughout: identifiers, comments, log messages, TOML keys, file
names, error messages, commit messages, branch names, issue/PR titles.

### Content Language

Lessons ship per-locale markdown (`content.de.md`, `content.en.md`). UI
strings live in JSON bundles under `packages/locales/`.

### Library Choice

**Paraglide JS** for the Svelte app: tree-shakeable, type-safe, ICU message
format. Missing-key warnings in development, fallback to EN in production.

### Coverage at v1

- UI: DE, EN
- Lessons: DE, EN
- Achievement names: DE, EN
- Palette names: DE, EN (where named)

### Fallback Chain

`active locale → en → key as literal`. The key fallback is a build warning, not
a silent failure.

### Author Workflow

1. Author writes lesson in their primary language plus EN as canonical
2. Both files committed in the same PR
3. CI checks all required locales present, all referenced `*_key` strings exist
4. Additional locales added as separate PRs by translation contributors

## Web Resource Delivery

### CDN Layout

```
https://lessons.pincel.app/
  index.json                                # manifest
  v1/foundation/what-is-a-pixel/
    lesson.toml
    content.de.md
    content.en.md
    reference.png
    solution.replay
    thumbnail.png
  v1/foundation/first-line/
    ...
  v1/_palettes/endesga-4.toml
  v1/_achievements/cluster-master.toml
  v1/locales/de.json
  v1/locales/en.json
```

### `index.json`

```json
{
  "schema_version": 1,
  "generated_at": "2026-05-13T12:00:00Z",
  "lessons": [
    {
      "id": "foundation/what-is-a-pixel",
      "version": "1.0.0",
      "path": "v1/foundation/what-is-a-pixel/",
      "checksum": "sha256:abc123...",
      "requires": [],
      "tree_position": { "x": 0, "y": 0 },
      "available_locales": ["de", "en"],
      "estimated_minutes": 10
    }
  ],
  "achievements": [/* … */],
  "palettes": [/* … */],
  "tree_paths": ["foundation", "color", "form-and-light", "subjects", "animation"]
}
```

### Caching

Three layers:

1. **HTTP/CDN**: `index.json` with `Cache-Control: max-age=300`. Lesson files
   versioned in the path and served with `Cache-Control: public, max-age=31536000, immutable`.
2. **Service Worker** (shared PWA + Tauri WebView): cache-first for lesson
   files, network-first with stale-while-revalidate for the index.
3. **App-data dir** (Tauri only): optional persistent disk cache surviving
   service-worker resets.

### Offline Strategy

- First launch precaches Foundation path (5 lessons)
- Each visited lesson is cached for offline use
- Settings toggle: "Download all lessons" triggers full precache
- Locked / uncached lessons show an offline indicator in the tree

### Versioning

- `schema_version` on the manifest enforces forward compatibility
- Each lesson has its own SemVer
- App tracks per-lesson local version, shows "Lesson updated" hint on minor/major
  bumps — but does not invalidate progress
- Breaking schema changes increment manifest `schema_version` and use a new
  path prefix (`v1/`, `v2/`)

### CDN Choice

**Cloudflare Pages**. Global edge cache, free for our size, integrated builds
from the GitHub repo. GitHub Pages is a fallback.

## Feedback Loop

All feedback flows through GitHub Issues — no backend in the app.

### Issue Templates

Located in `.github/ISSUE_TEMPLATE/`:

- `lesson-feedback.yml` — typos, unclear explanations, factual errors
- `translation.yml` — translation quality issues, locale field
- `validation-bug.yml` — user disputes a validation result; expects canvas snapshot
- `lesson-request.yml` — proposed new lessons or improvements

Each template has dropdowns prefilled by URL query params (lesson ID, version,
locale).

### App Entry Points

1. Lesson header — "Give feedback" button (opens `lesson-feedback`)
2. Failed validation — "Should this pass?" link (opens `validation-bug`)
3. Tree footer — "Suggest a lesson" (opens `lesson-request`)
4. Settings → Language — "Report a translation issue" (opens `translation`)

### Canvas Snapshot Workflow

For validation bugs, the app copies a PNG of the current canvas to the
clipboard before opening the issue. The template body instructs the user
to paste the screenshot — GitHub natively supports image paste in issues.

For replay-level feedback, the same mechanism is used with a JSON replay
block instead of a PNG, pasted into a fenced code block in the template.

### Body Language

The vorausgefüllte issue body uses EN ("User-Locale: de"), the user types
their own description in whatever language they prefer. Maintainers translate
or LLM-translate as needed.

## CLI Tooling (`pincel-lessons`)

The CLI replaces what would otherwise be in-app authoring. Written in Rust,
shares the `core` crate for validation logic.

### Commands

| Command | Purpose |
|---|---|
| `pincel-lessons new <path>/<id>` | scaffold a new lesson directory with templates |
| `pincel-lessons lint [<id>]` | validate TOML schema, locales, `requires` chain, key references |
| `pincel-lessons build` | produce CDN bundle in `dist/`, including `index.json` and checksums |
| `pincel-lessons preview [<id>]` | dev server with live reload, renders lessons as the app would |
| `pincel-lessons record <aseprite-file>` | extract stroke history from `.aseprite` to `solution.replay` |
| `pincel-lessons check-tree` | verify tree connectivity, unreachable lessons, position collisions |
| `pincel-lessons stats` | curriculum stats: lesson count per path, estimated total time, locale coverage |

### Lint Checks

- TOML schema valid
- All `requires` resolve to existing lesson IDs
- No cycles in the prerequisite graph
- All locale files present for required locales
- All `*_key` references resolve in the locale bundles
- All `fn` references exist in the validation registry
- All `check_fn` references exist in the achievement function set
- `tree_position` values do not collide
- Palette references resolve

## Monorepo Layout

```
pincel/
├── packages/
│   ├── core/                  # Rust: canvas, validation, replay, achievements
│   ├── app/                   # Svelte 5 + Tauri shell
│   │   ├── src/
│   │   └── src-tauri/
│   ├── lessons/               # Lesson content
│   │   ├── foundation/
│   │   ├── color/
│   │   ├── form-and-light/
│   │   ├── subjects/
│   │   ├── animation/
│   │   ├── _palettes/
│   │   └── _achievements/
│   ├── lessons-cli/           # `pincel-lessons` binary
│   ├── locales/               # UI translation bundles
│   │   ├── de.json
│   │   └── en.json
│   └── schema/                # Shared types (Rust → TS via ts-rs)
├── .github/
│   ├── ISSUE_TEMPLATE/
│   └── workflows/
│       ├── core-test.yml      # paths: packages/core, packages/schema
│       ├── app-build.yml      # paths: packages/app, packages/core
│       └── lessons-deploy.yml # paths: packages/lessons, packages/locales
├── docs/
│   └── specs/
│       └── lessons.md         # this file
├── Cargo.toml                 # workspace root
├── pnpm-workspace.yaml
└── README.md
```

### Workspace Configuration

`Cargo.toml`:

```toml
[workspace]
members = [
  "packages/core",
  "packages/lessons-cli",
  "packages/schema",
  "packages/app/src-tauri",
]
resolver = "2"
```

`pnpm-workspace.yaml`:

```yaml
packages:
  - "packages/app"
```

Rust and pnpm coexist cleanly: Rust ignores `node_modules`, pnpm ignores `target/`.

### Build Pipelines

- **`core-test.yml`** → `cargo test`, `clippy`, `fmt` on `packages/core` and `packages/schema` changes
- **`app-build.yml`** → PWA build + Tauri builds per platform, GitHub Releases
- **`lessons-deploy.yml`** → `pincel-lessons lint && pincel-lessons build`, deploy `dist/` to Cloudflare Pages

Lesson updates do not trigger app builds, and vice versa.

## Implementation Phases

### Phase 1 — Foundations (no lessons yet)
- Monorepo scaffolding (cargo + pnpm workspaces)
- `core` crate: canvas, command pattern, basic validation registry with 3 functions
- `schema` crate with `ts-rs` codegen
- `lessons-cli` skeleton: `new`, `lint`, `build`
- TOML parsing, locale loading
- CI: `core-test.yml`

### Phase 2 — Player
- Svelte 5 lesson player UI
- Markdown renderer with custom blocks (`:::demo`, `:::replay`)
- Validation execution against the core (via WASM)
- Local store (PWA: Dexie, Tauri: SQLite) behind `LessonStore` trait
- Skill tree SVG renderer
- Two real lessons end-to-end: `what-is-a-pixel`, `first-line`

### Phase 3 — Replay + Achievements
- Replay player with controls
- `pincel-lessons record` from `.aseprite`
- Event store + achievement evaluation
- Three initial achievements wired up

### Phase 4 — Content
- Full Foundation path (5 lessons)
- Full Color path (6 lessons)
- DE + EN translations

### Phase 5 — CDN + Feedback
- `lessons-deploy.yml` workflow
- Cloudflare Pages setup, `lessons.pincel.app` domain
- Service Worker caching
- GitHub Issue templates wired up to app entry points

### Phase 6 — Remaining Curriculum
- Form & Light, Subjects, Animation paths
- Additional achievements
- `pincel-lessons stats`, `check-tree`

## Open Questions

1. **Aseprite undo extraction**: the `.aseprite` binary format includes an
   undo history, but the spec is not fully public. Worst case `record` falls
   back to a custom Pincel-native recording format. Investigate before Phase 3.
2. **WASM size budget for `core`**: validation registry could grow significant.
   Target: <500 KB gzip for PWA bundle. Tree-shake unused validation fns?
3. **Achievement event-store growth**: bound the event log (e.g. last 10k
   events, plus aggregated counters)? Or keep unbounded for now and revisit?
4. **Locale fallback for missing lesson translations**: if `content.fr.md`
   does not exist, render `content.en.md` with a banner, or hide the lesson
   from FR users entirely? Lean banner.
5. **Tree-position collision avoidance**: enforced by lint, but no auto-layout
   suggestion. Acceptable for curated curriculum size.

## References

- `docs/specs/canvas.md` — canvas command pattern that replays build on
- `docs/specs/i18n.md` — global i18n conventions
- `docs/specs/cli.md` — general CLI conventions

## Out of Scope

The following are explicitly **not** part of this spec and belong elsewhere:

- The pixel editor itself (`docs/specs/canvas.md`)
- The `.ait` runtime asset format (Amigo Engine spec)
- General app shell, routing, settings (`docs/specs/app-shell.md`)
- Aseprite import/export pipeline (`docs/specs/aseprite-io.md`)
