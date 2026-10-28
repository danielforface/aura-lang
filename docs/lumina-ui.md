# Lumina UI (Aura) — current syntax & runtime

> **Implemented MVP versus long-term qualified app platform:** This is the evolving Raylib runtime's current API, not full UI certification. The proposed [qualification Master V2](qualification/MASTER_ROADMAP_V2.md) specifies Q12–Q18 for typed state/components, advanced layout/text/RTL/accessibility, modern Desktop, browser/Web, genuine Android/iOS UI, IDE tooling, app release and independent E2E. These plans do **not** qualify or implement Web/Mobile today; active development stays on Q03-001.


Lumina is Aura’s UI runtime plugin. The UI is expressed as a tree of nodes ("widgets") built by Aura code and rendered frame-by-frame by the Lumina plugin.

This document describes **what works today** in this repo (not a long-term locked spec).

## Running a Lumina app

### Build (Windows)

The `aura` binary enables the Raylib-backed Lumina window by default.

- Build: `cargo build -p aura`
- Run an example (Dev-VM / AVM):
  - `target\\debug\\aura.exe run examples\\root-aura\\shop_list_tool.aura --mode avm`

### Useful environment variables

- `AURA_UI_DEBUG=1`
  - Prints a small amount of UI-loop debug for the first frames.
- `AURA_UI_MAX_FRAMES=N`
  - Runs at most `N` UI frames, then exits.
  - Handy for scripted runs / CI to avoid hanging waiting for you to close the window.

## UI node model

A UI tree node has:
- `kind`: string (e.g. `"VStack"`, `"Button"`)
- `props`: string/int props (e.g. `padding: 12`, `bg: "#0D1117"`)
- `children`: nested nodes

### Colors

Color props accept:
- Named colors (e.g. `"white"`, `"black"`, `"gray"`, `"transparent"`)
- Hex: `"#RRGGBB"` and `"#RRGGBBAA"`
- `rgb(r,g,b)` and `rgba(r,g,b,a)`
  - where `r/g/b` are 0–255, and `a` is 0–1

## Supported nodes (current)

### `App`

Root container.

Props:
- `bg` / `background`: window clear color

Children:
- Any

### `Box` (MVP)

Single-child container that implements a basic box model (padding + background + border).

Props:
- `bg` / `background` (string color)
- Border: `border` / `stroke` (string color)
- Border width: `border_width` / `stroke_width` (int)
- `radius` (int)
- `padding` (int)
- `padding_x`, `padding_y` (int)
- `padding_left`, `padding_top`, `padding_right`, `padding_bottom` (int)
- `width`, `height` (int, optional)

Children:
- 0 or 1 child (more than one is currently ignored)

### `Image` (MVP)

Draws an image loaded from disk.

Props:
- `src` / `path` (string): filesystem path to an image file
- `width` / `height` (int, default 256)
- `fit` (string, default `"stretch"`): `"stretch"` | `"contain"` | `"cover"`
- `tint` / `color` (string color, default white)

Notes:
- Textures are cached in-memory for the lifetime of the window.
- `contain` preserves aspect ratio and letterboxes.
- `cover` preserves aspect ratio and crops.

Children:
- Ignored

### `Grid`

Simple grid layout for placing children in rows/columns.

```aura
Grid(cols: 3, gap: 12, padding: 12, bg: "#101010") {
  Box(col: 0, row: 0, bg: "#222", height: 60) { Text(value: "A") }
  Box(col: 1, row: 0, bg: "#222", height: 60) { Text(value: "B") }
  Box(col: 2, row: 0, bg: "#222", height: 60) { Text(value: "C") }

  Box(col: 0, row: 1, col_span: 2, bg: "#333", height: 80) { Text(value: "Spans 2 cols") }
  Box(col: 2, row: 1, row_span: 2, bg: "#333", height: 160) { Text(value: "Spans 2 rows") }
}
```

Props:
- `cols` / `columns` (int, default 1)
- `rows` (int, optional; if omitted/0, rows are inferred from children)
- `gap` (int, default 0)
- `gap_x` / `gap_y` (int, optional)
- `padding` / `padding_*` (int)
- `bg` / `background` (string color, optional)
- `border` (string color, optional)
- `border_width` (int, optional)
- `radius` (int, optional)

Child placement props:
- `col` (int, default 0)
- `row` (int, default 0)
- `col_span` (int, default 1)
- `row_span` (int, default 1)

Notes:
- Indices are 0-based.

### `VStack`

Vertical stack layout.

Props:
- `spacing` (int, default 0)
- `padding` (int, default 0)
- `alignment` (string, default `"start"`)
  - `"start"` or `"center"`

Children:
- Any

### `HStack`

Horizontal stack layout.

Props:
- `spacing` (int, default 0)
- `padding` (int, default 0)

Children:
- Any

### `Text`

Draws a single line of text.

Props:
- `text` / `content` (string)
- `size` (int, default 20)
- `color` / `fg` (string color)

Children:
- Ignored

### `Rect`

Draws a filled rectangle.

Props:
- `width` / `height` (int)
  - defaults to the available bounds
- `color` / `fg` / `fill` (string color)
- `radius` (int)

Children:
- Ignored

### `Button`

Clickable button.

Props:
- `label` (string)
- `width` (int, default 200)
- `height` (int, default 50)
- `bg` / `background` (string color)
- `fg` / `color` (string color)
- `radius` (int)
- `on_click` (callback id string, internal)

Children:
- Ignored

### `TextInput` (MVP)

Single-line text input with click-to-focus and basic typing.

Props:
- `value` / `text` (string)
- `placeholder` (string)
- `width` (int, default 360)
- `height` (int, default 46)
- `bg` / `background` (string color)
- `fg` / `color` (string color)
- `border` (string color)
- `radius` (int, default 12)
- `size` (int, default 18)
- `on_change` (callback id string, internal)
- `on_submit` (callback id string, internal; Enter)

Children:
- Ignored

### Absolute positioning (experimental)

Any node can set:
- `x` (int)
- `y` (int)

If present, the node is rendered at that absolute position (overriding the container bounds).

## Aura-side UI helpers (AVM)

The interpreter provides a small state bridge for controlled inputs:

- `ui.get_text(key: String) -> String`
- `ui.set_text(key: String, value: String) -> Unit`
- `ui.event_text() -> String`

TextInput callbacks receive the latest text via `ui.event_text()`.

## Known limitations (current)

- No scrolling containers.
- TextInput caret is end-of-text only; no selection.
- No stable key/diffing for lists (re-renders everything each frame).
- Grid is MVP: equal-size cells only (no content-based track sizing, alignment, or overflow handling yet).
- Image rendering is MVP: `fit` works (`stretch`/`contain`/`cover`), but there’s no clipping/radius yet; missing files draw a placeholder.
- Box is single-child only (for now); use `VStack/HStack` inside it.

## Experimental viewport-root resize groundwork (Agent B, 2026-10-28)

The isolated, unmerged `qual/ui-viewport-bounds-headless` branch (candidate for Draft review against proposed V2) sets the Raylib window as resizable and samples current **logical window dimensions** each frame for the root draw bounds. Initial dimensions remain 1920×1080 but no longer determine subsequent root bounds. Renderer-free geometry tests (including window resize sequences and invalid/nonpositive dimensions) and a source-contract regression run under `cargo test --locked -p aura-plugin-lumina --no-default-features`. This is a **prototype repair**, not qualification of AURA-Q13-002. There is no verified window interaction test, DPI/framebuffer conversion, responsive component/breakpoint engine, native OS matrix, visual pixel snapshot or accessibility proof. Existing `measure_node` happens inside rendering recursion, so true measurement/layout/painting separation remains a significant follow-up. See `aura-plugin-lumina/ENGINEERING_AUDIT.md` and the latest `docs/qualification/` evidence.

## LANE: LUMINA — experimental headless layout prepass (2026-10-28)

A subsequent *unmerged* topic branch `qual/ui-layout-prepass-headless` (stacked on the viewport Draft PR #60, itself stacked on the V2 plan Draft PR #58) extracts renderer-independent intrinsic measurement and placement. `App`, `Box`, `VStack`, `HStack`, `Grid` and existing leaf rectangles receive a deterministic arranged tree per frame before the paint pass; nested Stack intrinsic sizes and the same Button/TextInput click geometry are covered by narrow headless tests. The renderer no longer measures children during drawing. This is an **experimental implementation** requiring review, not production responsive layout or release. Equal-grid tracks and single-child Boxes are still deliberate MVP limits; no flex grow/shrink, overflow clipping, UI tree snapshots from real pixels, DPI, IME, RTL, accessibility or multi-platform qualification are implied.

## Experimental clipping, viewport scissor and action hit testing (Agent B, 2026-10-28)

The isolated, unmerged `qual/ui-clipping-hit-testing-headless` topic branch (stacked on Layout PR #61, Viewport PR #60 and proposed/unapproved V2 PR #58) adds a **prototype** clipping contract to the existing UI tree: the root viewport always clips, and container `App`, `Box`, `Grid`, `VStack` and `HStack` can optionally use `overflow: "hidden"` or `clip: "true"` to clip descendants to their own bounds. The default remains overflow-visible for compatibility, within the root viewport.

Bounds are prepared before native painting, with per-widget `begin_scissor_mode` and matching renderer-independent pointer hit checks. For overlapping interactive widgets, the last painted actionable widget receives the click. A TextInput hit receives its callback IDs and current controlled value before any drawing. Off-grid and ignored extra Box children are not interactive. This does not implement a full event capture/bubble system: non-actionable overlays do not automatically block clicks; no keyboard focus traversal, rounded clipping, DPI/framebuffer shader guarantees, pixel snapshots or a11y tree are claimed. Headless tests, Linux Raylib compile and an independently attempted virtual X11 smoke are distinct evidence levels, not a cross-platform UI certification.

## Experimental framebuffer-coordinate, resize and native input tests (Agent B, 2026-10-28)

The unmerged `qual/ui-framebuffer-resize-pixel-e2e` branch ([Draft #63](https://github.com/danielforface/aura-lang/pull/63), based on Draft #62→#61→#60→proposed #58) samples both live logical window size and physical render-target size on every frame. Layout, mouse targeting and Raylib scissor continue to consume logical coordinates. The custom rounded-rectangle shader now maps `gl_FragCoord` (physical bottom-left) into logical top-left using live X/Y scale and framebuffer height. Do **not** pre-scale scissor bounds: the Raylib backend handles that transform. The `lumina-diagnostics` Cargo feature (off by default) adds test-only resizing, measurements and native screen pixel readback; it is not a general public production UI API.

One isolated native Xvfb/llvmpipe suite asserts actual interior shader color, visible and clipped pixels, App background, and live resize at 640×420, 800×520 and 420×310; another sends real X11 XTest mouse events and asserts topmost-actionable callback with overflow-hidden prevention. The native tests are restricted to a **virtual Linux 1× software-GL environment**. Synthetic pure unit tests cover 1.5× and 2× transform calculations, NOT actual high-DPI monitor/GPU parity. No full Windows/macOS/mobile/Web, font/RTL/IME, accessibility, screenshot/golden pixel parity or general Q13 qualification is implied; all changes remain Draft.

## Experimental native keyboard/editing boundary (Agent B, 2026-10-28)

[Draft #64](https://github.com/danielforface/aura-lang/pull/64) (stacked on #63→#62→#61→#60→unapproved proposed #58) introduces a pure UTF-8 **Unicode scalar-index** TextInput editor: left/right/home/end navigation, backspace/delete, printable Unicode string insertion and safe byte-boundary slicing with caret clamp. Real Raylib keyboard input is sampled, applied and feedback is emitted **before painting** in the same frame. A focused input emits on_change only for mutated text and on_submit on Enter; clicks outside blur the field. Xvfb test drives actual XTest keydown/up and Raylib on_ui_render, checking real `UiTextInputEvent` after character, Home/Delete/End/Backspace, Return and blur. Headless tests exercise Hebrew/emoji scalar edit safety without claiming true grapheme sequence or RTL shaping.

This is NOT a full native text field: character boundaries are **Unicode scalars**, not user-perceived grapheme clusters, so combining marks and emoji ZWJ sequences need separate segmentation; visual cursor uses an approximate font-width estimate and is not bidi/RTL glyph-shaped. No IME candidate composition, marked text, clipboard/selection, Tab focus order, screen reader/accessibility, mobile/Web or OS target-matrix qualification, and Q13 remains proposed/unmerged.

## Experimental UAX #29 grapheme + native focus navigation (Agent B, 2026-10-28)

[Draft #65](https://github.com/danielforface/aura-lang/pull/65) (stack #64→#63→#62→#61→#60→proposed #58) upgrades the prior UTF-8 Unicode-*scalar* input engine to Unicode UAX #29 **extended grapheme clusters** using pinned `unicode-segmentation=1.12.0`: cursor, Left/Right, Backspace/Delete and insertion snap operate on complete graphemes (decomposed accents, combining marks, flags and family ZWJ emoji), with headless regression cases. Real KEY_TAB and Shift+Tab drive document-order wrapped focus through the measured renderer's visible, un-clipped and enabled TextInput nodes, not all raw children. Mouse hitting the `disabled="true"` input is also blocked. Real virtual X11/XTest native test sends Tab and Shift+Tab and checks on_change IDs and text, plus clicking disabled and typing no event; previous live rendering, GPU pixel/resize and keyboard tests preserved.

This is *logical grapheme* editing, **not** measured rendered font glyph/bidi navigation, semantic accessibility keyboard order, IME input composition, proper Unicode line-break/selection, uncontrolled input value persistence or full OS platform certification. Current controlled text fields require the host to consume callbacks and update their `value` prop for edits to persist across focus changes. The native tests are a virtual Linux 1× software OpenGL display; V2 Q13 remains unapproved/unqualified, and no release/merge is implied.

## Experimental local edit sessions, selection, undo and host-value reconciliation (Agent B, 2026-10-28)

[Draft #66](https://github.com/danielforface/aura-lang/pull/66), stacked on #65→#64→#63→#62→#61→#60→unapproved V2 #58, adds per-TextInput **ephemeral in-window edit sessions**. Local edits remain visible across Tab/Shift+Tab, mouse blur and refocus even when a static controlled prop `value` is not yet updated; when the parent changes the prop to a new authoritative value, its value overrides the draft and resets undo. Reuses the UAX29 grapheme editor with Shift navigation selection, Ctrl+A SelectAll, selection-aware Backspace/Delete/replacement and Ctrl+Z/Y bounded undo/redo (64 snapshots). Selection highlighting is an approximate logical overlay, not glyph-shaped. Real X11 XTest drives a two-input edit sequence and host rebase. A duplicate `on_change` callback ID is ambiguous: instead of sharing drafts, **both duplicate fields are excluded** from focus order and reject mouse focus; use unique callback IDs.

**Contract warning:** `on_change` callback IDs must be stable and unique while a TextInput exists. The host still owns permanent application state; local draft cache dies with native window/plugin. A delayed host echo of an intermediate edit is not distinguishable from a genuinely authoritative new prop without a version/ACK protocol. Full async host reconciliation, IME, visual RTL/BiDi glyph shaping, complete text selection pixel precision, clipboard, accessibility, per-input undo memory byte budget and native OS device matrix remain separate unqualified gates. Q13 not yet approved or certified. No merge/release from this Draft.

## Experimental opt-in value_revision contract (Agent B, 2026-10-28)

[Draft #67](https://github.com/danielforface/aura-lang/pull/67) (stacked on #66→#65→#64→#63→#62→#61→#60→proposed/unapproved #58) adds an **optional** TextInput prop `value_revision` as a decimal unsigned 64-bit external-authority epoch. For example `TextInput(value="A", value_revision="7", on_change="cb:101")`. The host **keeps revision 7 unchanged** while echoing user input events (e.g. `value="Aq"` arrives late after the user already typed `Aqe`); this echo must never erase the newer local draft. To force an external authoritative replacement, the host **increments** the revision (e.g. `value_revision="8", value="SERVER"`). Old revision packets are ignored; unknown values at the same revision are ignored rather than treated as authority. Same-text new revision clears pre-revision undo. Invalid revision tokens reject input focus. Without `value_revision`, the prior legacy controlled changed-value semantics continue.

Native software-GL Xvfb/XTest exercises actual delayed echo, stale lower rev and new authority with real keyboard callbacks, cross-field isolation and invalid token; two more native cases exercise new rev equal current draft / offscreen duplicate callback revision not poisoning visible one. This is **observational echo matching by text, not exact ACK** — `UiTextInputEvent` has no edit sequence or ACK field, repeated identical edit values cannot be disambiguated. Recent echo record is bounded to 32 values / 128KiB; oversized or unknown echoes fail closed. Hosts must deliberately adopt the revision epoch convention; no multi-writer conflict resolution, durable undo across app restart, IME/RTL font shaping/a11y/platform certification or stable Q13 qualification is claimed. All PRs remain Draft/unmerged.

## Experimental measured TextInput viewport (Agent B, 2026-10-28)

[Draft #68](https://github.com/danielforface/aura-lang/pull/68) adds a separate pure `text_viewport.rs` and native default-Raylib-font TextInput layout path. Caret and selection are calculated from the actual `RaylibHandle::measure_text` widths of UAX29 grapheme-boundary prefixes, not the previous constant-width estimate. Active inputs automatically scroll horizontally to keep the cursor visible, scroll is retained across field blur/focus and clamped when text shrinks. The widget background/border and padded text/selection/caret use **two sequential, nonnested Raylib scissor scopes**, so long text cannot overpaint outside its own content while its rounded border is retained. Native opt-in `diagnostic_text_visual` reports measured caret/scroll and clipped selection for Xvfb tests, not a general release API.

The scoped real X11 test types or navigates via Tab, Home, End and Ctrl+A and samples actual GPU pixels inside and beyond an overflowing narrow TextInput, including resize. This covers **ASCII text using the default Raylib bitmap font on software OpenGL Xvfb**, not Arabic/Hebrew RTL glyph shaping, Unicode fallback, IME, accessibility, physical high-DPI or Windows/macOS/mobile/Web parity. Source and tests remain unmerged, proposed V2 Q13 NOT_STARTED/NOT QUALIFIED.

## Experimental explicit external Unicode fonts (Agent B, 2026-10-28)

[Draft #69](https://github.com/danielforface/aura-lang/pull/69) (stack #68→#67→#66→#65→#64→#63→#62→#61→#60→unapproved V2 #58) adds a per-widget optional `font_path` for Text and TextInput, e.g. `font_path="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", size="24"`. **The app supplies its own licensed local TTF or OTF**; Lumina does not bundle fonts. When supplied, the native renderer loads a bounded glyph atlas for actual Unicode text + printable ASCII, dynamically grows it if input values change, and uses that exact `Font` to measure caret and selection prefixes and draw the text. A missing/unavailable font retains previous default-font rendering. Bounds: at most eight external GPU font entries, 1024 distinct glyph requests each, 256KiB scanned text per value, 16MiB maximum local font file and 512-byte font path, 10–96px sizes; exceeded inputs fall back rather than pretending atlas coverage. GPU fonts are dropped while native GL context is still alive.

The scoped Xvfb native test (system DejaVu installed in CI, no font redistributed) proves actual red pixel rasterization of Hebrew Text `אבג` and TextInput `שלום`, actual custom-font measure/caret parity, real XTest keyboard appended q and updated authoritative `שלום עולםx` after dynamic atlas extension, and missing-font baseline fallback. **Hebrew glyph pixels do NOT prove visual RTL reading order or Hebrew BiDi caret**, complex shaping, mark placement, Arabic shaping, font fallback, IME, accessibility or hardware 2×/native Win/mac/mobile/Web. V2 Q13 remains proposed NOT_STARTED/NOT QUALIFIED; Draft unmerged.
