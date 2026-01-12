# UI Guide (Lumina)

This guide documents Aura’s current UI stack (**Lumina**, Raylib backend) and the near-term plan to reach “real app” capability: keyboard/mouse input, components, theming, and strong layout.

## 1. Principles

- **Declarative UI**: the UI tree is rebuilt each frame from Aura.
- **Callbacks must be non-blocking**: do not wait for stdin, do not sleep, do not perform long I/O in `on_click`.
- **State lives outside the UI tree**: UI reads state, callbacks mutate state.

## 2. Styling

### 2.1. Colors

Lumina accepts:

- Named colors (small palette): `"Black"`, `"White"`, `"Gold"`, `"Gray"`, `"DarkGray"`, etc.
- Hex: `"#RRGGBB"`, `"#RRGGBBAA"`
- `rgb(...)` / `rgba(...)`

### 2.2. App background

You can theme the full window background:

- `App(title: "My App", bg: "#0B0F14") { ... }`

## 3. Input

### 3.1. Current prototype

Some examples use staged console input to simulate text entry while we build real UI inputs.

### 3.2. Planned feature set

- `TextInput` / `PasswordInput`
- Focus + tab navigation
- Clipboard + selection
- IME support (later)

## 4. Components (planned)

- Reusable UI components with parameters and local state
- Lists with stable keys (diffing) and virtualization
- Modals, toasts, menus

## 5. Layout (planned)

- Flex row/column (grow/shrink, align/justify)
- Grid
- Scroll containers

## 6. Tooling (planned)

- UI inspector overlay
- Hot reload of Aura UI
- Widget test harness
