# Lumina UI: Grid, Image, and Layout

## Overview

Lumina is Aura's modern UI system, providing responsive layouts, rich media support, and real-time proof visualization. This guide covers **Grid** (multi-column layouts), **Image** (fit modes), and **Audio** controls.

## Grid Layout

**Grid** is a responsive multi-column container for building complex UI layouts. It supports:
- **Multi-column** responsive design
- **Row/column spanning** for complex arrangements
- **Flexible spacing** and padding
- **Theming** with colors, borders, and radius

### Basic Grid

```aura
Grid(
  columns: 2,
  gap: 8,
  padding: 12,
  bg: "#f5f5f5",
  border: "#ddd",
  radius: 6
)
```

### Grid Properties

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `columns` | Int | ✓ | — | Number of grid columns |
| `rows` | Int | — | auto | Explicit row count (auto-calculated if omitted) |
| `gap` | Int | — | 8 | Spacing between cells (pixels) |
| `padding` | Int | — | 0 | Interior padding (all sides) |
| `bg` | String | — | transparent | Background color (named or hex) |
| `border` | String | — | transparent | Border color |
| `radius` | Int | — | 0 | Corner radius (pixels) |

### Grid Child Placement

Children of Grid can specify placement using:

```aura
Box(
  col: 0,          // Column index (0-indexed)
  row: 0,          // Row index (0-indexed)
  col_span: 2,     // Columns to span (default 1)
  row_span: 1      // Rows to span (default 1)
)
```

### Example: 2-Column Card Layout

```aura
Grid(
  columns: 2,
  gap: 16,
  padding: 20,
  bg: "white"
) {
  Box(bg: "#e3f2fd", padding: 16, radius: 4) {
    Text("Card 1")
  },
  Box(bg: "#f3e5f5", padding: 16, radius: 4) {
    Text("Card 2")
  },
  Box(col: 0, row: 1, col_span: 2, bg: "#e8f5e9", padding: 16, radius: 4) {
    Text("Full Width Footer")
  }
}
```

### Example: Responsive Gallery

```aura
Grid(
  columns: 3,
  gap: 12,
  padding: 12
) {
  Image(src: "photo1.png", fit: "cover"),
  Image(src: "photo2.png", fit: "cover"),
  Image(src: "photo3.png", fit: "cover"),
  Image(src: "photo4.png", fit: "cover", col: 0, row: 1, col_span: 2),
  Image(src: "photo5.png", fit: "cover", col: 2, row: 1)
}
```

## Image Widget

**Image** displays raster images with multiple fit modes for responsive design.

### Image Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `src` / `path` | String | — | Image file path (PNG, JPEG, BMP, TGA) |
| `width` | Int | 256 | Width in pixels |
| `height` | Int | 256 | Height in pixels |
| `fit` | String | "stretch" | Resize mode: "stretch" \| "contain" \| "cover" |
| `tint` | String | "white" | Color overlay (blend mode) |

### Fit Modes

- **`"stretch"`** (default): Ignore aspect ratio, stretch to fill container
  - Use for: Logos, abstract graphics where aspect ratio doesn't matter
  - Pro: Fills all available space
  - Con: Image may appear distorted

- **`"contain"`**: Preserve aspect ratio, fit entirely within container
  - Use for: Photos, artwork, UI elements where distortion is unacceptable
  - Pro: No cropping, natural appearance
  - Con: May have empty space (letterboxing)

- **`"cover"`**: Preserve aspect ratio, fill container (crops edges if needed)
  - Use for: Backgrounds, thumbnails, hero images
  - Pro: Fills all space, natural appearance
  - Con: May crop content

### Basic Image

```aura
Image(
  src: "assets/logo.png",
  width: 200,
  height: 200,
  fit: "contain"
)
```

### Example: Image Fit Comparison

```aura
Grid(columns: 3, gap: 16, padding: 16) {
  Box(bg: "#f0f0f0", padding: 16) {
    Image(src: "photo.jpg", width: 150, height: 100, fit: "stretch"),
    Text("stretch")
  },
  Box(bg: "#f0f0f0", padding: 16) {
    Image(src: "photo.jpg", width: 150, height: 100, fit: "contain"),
    Text("contain")
  },
  Box(bg: "#f0f0f0", padding: 16) {
    Image(src: "photo.jpg", width: 150, height: 100, fit: "cover"),
    Text("cover")
  }
}
```

## Audio Controls

Play, pause, and stop audio with simple built-in functions:

### Audio Built-ins

```aura
// Load audio file into memory (returns clip ID)
let clip_id: U32 = audio.load("assets/music.wav")

// Play pre-loaded clip (returns playback handle)
let handle: U32 = audio.play_loaded(clip_id)

// Play directly from file (returns playback handle)
let handle: U32 = audio.play("assets/sound.ogg")

// Stop playback
audio.stop(handle)
```

### Audio Properties

| Function | Parameters | Returns | Description |
|----------|-----------|---------|-------------|
| `audio.load` | `path: String` | `U32` | Load audio file, return clip ID |
| `audio.play_loaded` | `clip_id: U32` | `U32` | Play loaded clip, return handle |
| `audio.play` | `path: String` | `U32` | Play file directly, return handle |
| `audio.stop` | `handle: U32` | `()` | Stop playback by handle |

### Example: Simple Audio Player

```aura
let clip_id: U32 = 0

Box(
  padding: 20,
  bg: "#fff",
  radius: 8
) {
  Button(label: "Play") on_click {
    if clip_id == 0 {
      clip_id = audio.load("assets/music.wav")
    }
    let handle = audio.play_loaded(clip_id)
  },
  Button(label: "Stop") on_click {
    // Note: In production, track the handle separately
    audio.stop(0)
  }
}
```

### Example: Playlist UI

```aura
let tracks = ["track1.mp3", "track2.mp3", "track3.mp3"]
let current_index: U32 = 0
let current_handle: U32 = 0

Box(padding: 16, bg: "#f9f9f9") {
  Text(tracks[current_index]),
  Box(padding: 12, gap: 8) {
    Button(label: "◀ Prev") on_click {
      if current_index > 0 {
        audio.stop(current_handle)
        current_index = current_index - 1
        current_handle = audio.play(tracks[current_index])
      }
    },
    Button(label: "▶ Play") on_click {
      current_handle = audio.play(tracks[current_index])
    },
    Button(label: "Next ▶") on_click {
      if current_index < 2 {
        audio.stop(current_handle)
        current_index = current_index + 1
        current_handle = audio.play(tracks[current_index])
      }
    }
  }
}
```

## Combining Grid, Image, and Audio

Here's a complete example combining all features:

```aura
Grid(columns: 2, gap: 16, padding: 20, bg: "white") {
  // Left: Image gallery
  Box(col: 0, row: 0, col_span: 1) {
    Image(src: "album_art.png", width: 200, height: 200, fit: "cover")
  },
  
  // Right: Metadata and controls
  Box(col: 1, row: 0) {
    Text("Now Playing"),
    Text("Album Title"),
    Text("Artist Name"),
    
    // Audio controls
    Box(padding: 12) {
      Button(label: "⏮ Prev") on_click { audio.stop(0) },
      Button(label: "▶ Play") on_click { let h = audio.play("song.mp3") },
      Button(label: "⏭ Next") on_click { audio.stop(0) }
    }
  },
  
  // Bottom: Playlist
  Box(col: 0, row: 1, col_span: 2) {
    Text("Playlist"),
    // (Iterate over tracks here in production)
  }
}
```

## Best Practices

1. **Use Grid for complex layouts** — Nested Boxes work but Grid is cleaner
2. **Choose fit modes wisely** — "cover" for backgrounds, "contain" for photos
3. **Load audio once** — Use `audio.load()` at startup, then `audio.play_loaded()` for playback
4. **Set reasonable dimensions** — Grid cells auto-size; set Image widths explicitly
5. **Test responsiveness** — Try resizing the window to ensure layouts adapt

## Sentinel Visualization

Aura's **Sentinel** IDE includes live Grid layout inspection:
- **Grid Visualizer**: See layout grid, cell spanning, gaps
- **Image Inspector**: Preview fit modes and rendering
- **Audio Debugger**: Monitor playback state, handle IDs

See [LSP + Aura Sentinel](../lsp-and-sentinel) for IDE integration details.

## See Also

- [Lumina Media (Audio, Video, Files)](../lumina-media)
- [Cookbook: Lumina UI Patterns](../cookbook-lumina-ui)
- [The Proof System](../proof-system) (for Z3 integration with UI)
