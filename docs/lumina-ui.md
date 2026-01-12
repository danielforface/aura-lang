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
- Layout is simple stack layout (no flex/grid).
