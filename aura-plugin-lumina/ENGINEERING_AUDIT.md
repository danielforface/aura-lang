# Lumina engineering audit — initial bounded source inspection (2026-10-28)

**LANE:** LUMINA · **Task:** AURA-Q13-002 preliminary experimental sub-slice · **Branch:** `qual/ui-viewport-bounds-headless` (based on proposed/unmerged Master V2) · **Status:** static source inspection plus headless geometry tests, **NOT** a complete platform or UI qualification.

## Renderer and window
- `aura-plugin-lumina/src/lib.rs` runs a single-window Raylib immediate drawing path with shader-based rounded rectangles, a 60 FPS target and pre-frame image texture loading. It formerly initialized 1920×1080 and repeatedly supplied that static rectangle to `render_node`.
- Candidate now enables `.resizable()` and derives the root bounds per frame from `RaylibHandle::get_screen_width/height`; `RootViewport` normalizes nonpositive dimensions without making them fictitiously 1920×1080.
- **Not fixed/tested:** independent framebuffer vs logical screen dimensions, shader `gl_FragCoord` scaling/coordinate origin on HiDPI, per-monitor DPI changes, fullscreen/minimize edge cases, OS event-loop/device loss, real native frame interaction. `GetRenderWidth/Height` are not substituted for logical bounds.

## Layout/measurement
- `render_node` owns placement *and* drawing; `measure_node` is recursively called during `VStack`/`HStack` traversal. There is no separate retained layout tree, constraints/measure/arrange/draw pass, or stable layout invalidation.
- `VStack` and `HStack` use intrinsic guesses and fixed padding/spacing; `Grid` uses equal-sized tracks, fixed spans and clamped minimum cells; `Box` renders only the first child. Explicit width/height and absolute `x/y` override parent space, without clipping or overflow strategy.
- **Risk:** a dynamic root rectangle alone cannot prevent child overflow, ensure responsive sizing, or prove touch/click geometry fidelity. Next architecture decision should isolate deterministic measure/arrange geometry independent of Raylib before broad component expansion.

## Events, input and state
- The runtime samples mouse click/position and basic keys/characters before drawing, dispatches `on_click`, `on_change`, `on_submit` through `UiRuntimeFeedback`; `FocusedTextInput` tracks buffer and a scalar-value caret with a callback ID.
- Char-vector edits handle ordinary Unicode scalar values, not grapheme cluster boundaries. No IME composition, selection, clipboard, Tab focus traversal, keyboard-only button activation, pointer capture, multi-touch or accessibility action tree was observed in the plugin.
- Focus and clicked animation are private to a single `LuminaWindow`, while application state ownership lies upstream in the Aura/AVM UI bridge. This audit does not presume the bridge is formally verified.

## Text and assets
- `draw_text` and rough `chars().count() * size * 0.6` width estimates are not Unicode shaping, bidirectional text, accurate font metrics or screen-reader labels; Hebrew/RTL and complex scripts remain unqualified.
- Image resources are loaded recursively before rendering into `HashMap<String, Texture2D>`, cached for the window lifetime. No observed eviction/budget, hot-reload invalidation, explicit stale-asset behavior or comprehensive resource lifecycle tests. Missing files have placeholder behavior.

## Test and platform boundary
- Observed test-first **source contract** RED on `1f984dce3` (GitHub Actions 40678322001) for the fixed viewport, then headless geometry (3 cases) + source contract GREEN on `98a34e3` (40678401971), Ubuntu `cargo test --locked -p aura-plugin-lumina --no-default-features`.
- A separate Linux Raylib-feature **compile-only** job was added at `ee056ac7` (40678540566); refer to live GitHub status rather than inferring a pass. None of these tests opens a window, renders pixels, performs a physical resize or exercises host DPI, Android/iOS/Web, screen readers or actual app E2E.
- **Decision boundary:** Keep proposed V2 Q13 NOT_STARTED; classify this as an independent component-local prototype. Before touching Nexus, AST, compiler, AVM or shared contracts, propose an API handoff on Issue #59 and obtain core-owner coordination.

## Next safe action
Review Raylib compile job logs; repair any actual compile regression only in `aura-plugin-lumina/`. Open a Draft PR targeting `plan/lumina-full-platform-qualification-v2`; keep the source-root fix isolated. A subsequent independently claimed and reviewed sub-slice can extract pure measure/arrange geometry and add deterministic clipping/overflow/interaction tests. Do not mark DPI, full Q13, desktop OS or mobile/web qualification complete.

## LANE: LUMINA — LAYOUT-002 experiment addendum (2026-10-28)

The isolated `qual/ui-layout-prepass-headless` branch provides a **real production callsite separation**: the Raylib frame first builds a renderer-independent `LayoutNode` tree via pure `measure_tree` and `arrange_measured`, and only then calls `begin_drawing`. Painting traverses that precomputed layout without invoking `measure_node`; Button and TextInput hit rects use the arranged rect. It preserves Box single-child semantics and Grid out-of-range child slots; nested stack intrinsic measurement now includes children instead of defaulting to zero. This changes MVP nested sizing intentionally and needs visual regression tests before stable UI claims.

**Evidence:** source-contract test RED [Actions 40679761908](https://github.com/danielforface/aura-lang/actions/runs/40679761908), initial headless+Linux Raylib compile GREEN [40680092045](https://github.com/danielforface/aura-lang/actions/runs/40680092045); bottom-up measured cache/test at `f3c01140` is additional candidate pending CI. Headless tests include padding, Grid span/clamping, viewport resize, click geometry, invalid geometry, idempotence and nested sizes. **Not validated:** live window draw, clipping/overlap hit priority, real DPI/scaling, locale/font shaping/RTL, accessibility, screenreader, touch/IME, multiple windows, GPU memory resource ownership, OS/Web/Mobile E2E. Q13 is still unapproved proposed roadmap and no certification.

## LANE: LUMINA — CLIP-HIT-003 implementation addendum (2026-10-28)

**Prior risk:** old `render_node` inspected `mouse_clicked` per Button/TextInput while painting, with first callback winning. No shared clipping geometry or Raylib scissor existed. A child outside its ancestor or the resized viewport could be clickable, and overlapping interactive siblings could disagree with visual stacking.

**Bounded experimental solution:** root viewport clip is mandatory; App/Box/Grid/VStack/HStack accept explicit `overflow: hidden` or `clip: true`, passed as `LayoutNode.clip` through the pure layout pass. `Bounds` has finite size and intersection, half-open hit boundaries, nonnegative raster scissor bounds. `layout::hit_test` resolves actions in reversed child paint order, ignoring skipped children and non-actionable nodes; input focus and click callbacks are chosen before native drawing. Each widget's painting (including rounded shader paths) runs inside a Raylib scissor that exits before child drawing, avoiding nested scissor reset. Geometry/hit and paint share the same inherited clip. No AST/IR/Nexus/Compiler/AVM changes.

**Evidence:** deliberate source-contract RED [40681500635](https://github.com/danielforface/aura-lang/actions/runs/40681500635); 21 headless unit + four contract tests and separate Linux feature compile GREEN [40681881293](https://github.com/danielforface/aura-lang/actions/runs/40681881293); extra half-open + malformed callback tests GREEN [40682070143](https://github.com/danielforface/aura-lang/actions/runs/40682070143). Xvfb real Raylib window smoke separately added in [40682173339](https://github.com/danielforface/aura-lang/actions/runs/40682173339) pending observation at audit update.

**Remaining:** opt-in rectangle-only clipping does not equal rounded-corner/alpha stencil clipping; sibling opaque overlay interception, keyboard navigation/a11y, precise font/RTL/IME selection, physical framebuffer-to-logical DPI scaling, GPU shader coordinates and native resize events are unqualified. Xvfb smoke is not pixel/screenshot proof or physical Window/OS target certification. Q13 tasks remain proposed/NOT_STARTED.
