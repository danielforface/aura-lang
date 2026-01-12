# Lumina UI (Aura) — current syntax & runtime

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
