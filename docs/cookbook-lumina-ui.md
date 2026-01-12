# Aura Lumina Cookbook: Grid, Image Fit Modes & Audio

This cookbook provides practical examples for Grid layout, Image fit modes, and Audio playback in Aura Lumina applications.

---

## Recipe 1: Grid Layout with Responsive Cards

**Goal:** Build a 2-column card grid that adapts with different content.

```aura
import aura::lumina

cell main():
    layout:
        VStack(spacing: 16, alignment: "center", style: Style { padding: 24, bg: "#1a1a2e" }) {
            render: Text(text: "Product Grid", style: Style { fg: "White", size: 32 })

            render:
                Grid(columns: 2, gap: 16, style: Style { padding: 16, bg: "#0f3460", radius: 8 }) {
                    # Card 1
                    render: Box(style: Style { width: 280, height: 200, bg: "#16213e", radius: 6, padding: 12 }) {
                        render: VStack(spacing: 8) {
                            render: Text(text: "Product A", style: Style { fg: "White", size: 18 })
                            render: Text(text: "$29.99", style: Style { fg: "#00d4ff", size: 16 })
                            render: Text(text: "Premium quality item", style: Style { fg: "#999", size: 12 })
                        }
                    }

                    # Card 2
                    render: Box(style: Style { width: 280, height: 200, bg: "#16213e", radius: 6, padding: 12 }) {
                        render: VStack(spacing: 8) {
                            render: Text(text: "Product B", style: Style { fg: "White", size: 18 })
                            render: Text(text: "$39.99", style: Style { fg: "#00d4ff", size: 16 })
                            render: Text(text: "Best seller item", style: Style { fg: "#999", size: 12 })
                        }
                    }

                    # Card 3 (spans 2 columns)
                    render: Box(style: Style { width: 576, height: 120, bg: "#16213e", radius: 6, padding: 12 }) {
                        render: HStack(spacing: 16) {
                            render: Text(text: "Featured Deal", style: Style { fg: "#FFD700", size: 20 })
                            render: Text(text: "Up to 50% off!", style: Style { fg: "White", size: 14 })
                        }
                    }
                }
        }
```

---

## Recipe 2: Image with Different Fit Modes

**Goal:** Display images using stretch, contain, and cover fit modes side-by-side.

```aura
import aura::lumina

cell main():
    layout:
        VStack(spacing: 16, alignment: "center", style: Style { padding: 24 }) {
            render: Text(text: "Image Fit Modes Demo", style: Style { fg: "White", size: 32 })

            render:
                HStack(spacing: 16, alignment: "center") {
                    # Stretch fit
                    render: Box(style: Style { width: 200, height: 150, bg: "Black", radius: 4 }) {
                        render: VStack(spacing: 8) {
                            render: Text(text: "Stretch", style: Style { fg: "#999", size: 12 })
                            render: Text(text: "Fills entire space", style: Style { fg: "Gray", size: 10 })
                        }
                    }

                    # Contain fit
                    render: Box(style: Style { width: 200, height: 150, bg: "Black", radius: 4 }) {
                        render: VStack(spacing: 8) {
                            render: Text(text: "Contain", style: Style { fg: "#999", size: 12 })
                            render: Text(text: "Letterbox with aspect", style: Style { fg: "Gray", size: 10 })
                        }
                    }

                    # Cover fit
                    render: Box(style: Style { width: 200, height: 150, bg: "Black", radius: 4 }) {
                        render: VStack(spacing: 8) {
                            render: Text(text: "Cover", style: Style { fg: "#999", size: 12 })
                            render: Text(text: "Crop to fill space", style: Style { fg: "Gray", size: 10 })
                        }
                    }
                }

            render: Text(text: "✓ Use fit='stretch'|'contain'|'cover' in Image props", style: Style { fg: "Gold", size: 12 })
        }
```

---

## Recipe 3: Audio Player UI with Play/Stop Controls

**Goal:** Build a minimal audio player UI with button controls.

```aura
import aura::lumina

cell main():
    val audio_playing: String = "false"

    layout:
        VStack(spacing: 24, alignment: "center", style: Style { padding: 32, bg: "#0d0d0d" }) {
            render: Text(text: "🎵 Audio Player", style: Style { fg: "White", size: 36 })

            render:
                Box(style: Style { width: 320, height: 240, bg: "#1a1a2e", radius: 8, padding: 24 }) {
                    render: VStack(spacing: 20, alignment: "center") {
                        render: Text(text: "Now Playing", style: Style { fg: "White", size: 20 })
                        render: Text(text: "Track Title Here", style: Style { fg: "#00d4ff", size: 16 })
                        render: Text(text: "00:45 / 03:22", style: Style { fg: "#999", size: 12 })

                        render:
                            HStack(spacing: 12, alignment: "center") {
                                render: Button(label: "▶ Play", style: Style { bg: "#00d4ff", fg: "Black", width: 100, height: 40, radius: 4 }) {
                                    on_click: ~> {
                                        val io = "io"
                                        io.println("Audio: Play pressed")
                                    }
                                }

                                render: Button(label: "⏸ Pause", style: Style { bg: "Gray", fg: "White", width: 100, height: 40, radius: 4 }) {
                                    on_click: ~> {
                                        val io = "io"
                                        io.println("Audio: Pause pressed")
                                    }
                                }

                                render: Button(label: "⏹ Stop", style: Style { bg: "#ff4757", fg: "White", width: 100, height: 40, radius: 4 }) {
                                    on_click: ~> {
                                        val io = "io"
                                        io.println("Audio: Stop pressed")
                                    }
                                }
                            }
                    }
                }

            render: Text(text: "Use audio.load(path) to load and audio.play(id) to play", style: Style { fg: "Gold", size: 11 })
        }
```

---

## Recipe 4: Hybrid Grid + Image Gallery

**Goal:** Combine Grid layout with styled image placeholders.

```aura
import aura::lumina

cell main():
    layout:
        VStack(spacing: 20, alignment: "center", style: Style { padding: 20 }) {
            render: Text(text: "Photo Gallery", style: Style { fg: "White", size: 36 })

            render:
                Grid(columns: 3, gap: 12, style: Style { padding: 16, bg: "#111111" }) {
                    # Row 1: 3 images
                    render: Box(style: Style { width: 160, height: 160, bg: "#222", radius: 4 }) {
                        render: VStack(spacing: 4) {
                            render: Text(text: "IMG-001", style: Style { fg: "White", size: 12 })
                        }
                    }

                    render: Box(style: Style { width: 160, height: 160, bg: "#222", radius: 4 }) {
                        render: VStack(spacing: 4) {
                            render: Text(text: "IMG-002", style: Style { fg: "White", size: 12 })
                        }
                    }

                    render: Box(style: Style { width: 160, height: 160, bg: "#222", radius: 4 }) {
                        render: VStack(spacing: 4) {
                            render: Text(text: "IMG-003", style: Style { fg: "White", size: 12 })
                        }
                    }

                    # Row 2: Wide item spanning 2 columns
                    render: Box(style: Style { width: 340, height: 120, bg: "#333", radius: 4 }) {
                        render: HStack(spacing: 8) {
                            render: Text(text: "Featured", style: Style { fg: "#FFD700", size: 16 })
                            render: Text(text: "Wide image banner", style: Style { fg: "White", size: 12 })
                        }
                    }

                    render: Box(style: Style { width: 160, height: 120, bg: "#222", radius: 4 }) {
                        render: VStack(spacing: 4) {
                            render: Text(text: "IMG-004", style: Style { fg: "White", size: 12 })
                        }
                    }
                }
        }
```

---

## Recipe 5: Advanced Audio + Grid Playlist

**Goal:** Grid-based playlist with audio controls.

```aura
import aura::lumina

cell main():
    layout:
        VStack(spacing: 16, alignment: "center", style: Style { padding: 24, bg: "#0a0e27" }) {
            render: Text(text: "🎵 Playlist", style: Style { fg: "White", size: 40 })

            render:
                Grid(columns: 1, gap: 8, style: Style { width: 500, padding: 12, bg: "#1a1f3a" }) {
                    # Track 1
                    render: Box(style: Style { width: 476, height: 60, bg: "#252d48", radius: 4, padding: 8 }) {
                        render: HStack(spacing: 12) {
                            render: Text(text: "1.", style: Style { fg: "Gold", size: 14 })
                            render: VStack(spacing: 2) {
                                render: Text(text: "Track One", style: Style { fg: "White", size: 14 })
                                render: Text(text: "3:22", style: Style { fg: "#999", size: 10 })
                            }
                            render: Button(label: "▶", style: Style { bg: "#00d4ff", fg: "Black", width: 40, height: 40, radius: 2 }) {
                                on_click: ~> {
                                    val io = "io"
                                    io.println("Playing track 1")
                                }
                            }
                        }
                    }

                    # Track 2
                    render: Box(style: Style { width: 476, height: 60, bg: "#252d48", radius: 4, padding: 8 }) {
                        render: HStack(spacing: 12) {
                            render: Text(text: "2.", style: Style { fg: "Gold", size: 14 })
                            render: VStack(spacing: 2) {
                                render: Text(text: "Track Two", style: Style { fg: "White", size: 14 })
                                render: Text(text: "4:15", style: Style { fg: "#999", size: 10 })
                            }
                            render: Button(label: "▶", style: Style { bg: "#00d4ff", fg: "Black", width: 40, height: 40, radius: 2 }) {
                                on_click: ~> {
                                    val io = "io"
                                    io.println("Playing track 2")
                                }
                            }
                        }
                    }

                    # Track 3
                    render: Box(style: Style { width: 476, height: 60, bg: "#252d48", radius: 4, padding: 8 }) {
                        render: HStack(spacing: 12) {
                            render: Text(text: "3.", style: Style { fg: "Gold", size: 14 })
                            render: VStack(spacing: 2) {
                                render: Text(text: "Track Three", style: Style { fg: "White", size: 14 })
                                render: Text(text: "2:58", style: Style { fg: "#999", size: 10 })
                            }
                            render: Button(label: "▶", style: Style { bg: "#00d4ff", fg: "Black", width: 40, height: 40, radius: 2 }) {
                                on_click: ~> {
                                    val io = "io"
                                    io.println("Playing track 3")
                                }
                            }
                        }
                    }
                }
        }
```

---

## API Reference

### `Grid` Constructor

```aura
Grid(
    columns: Int,              # Number of columns (required)
    rows: Int,                 # Number of rows (optional; auto-inferred if omitted)
    gap: Int,                  # Space between cells (default: 0)
    gap_x: Int,                # Horizontal gap (optional)
    gap_y: Int,                # Vertical gap (optional)
    padding: Int,              # Interior padding (optional)
    bg: String,                # Background color (optional)
    border: String,            # Border color (optional)
    border_width: Int,         # Border thickness (optional)
    radius: Int                # Corner radius (optional)
) {
    # Children placed using col/row/col_span/row_span
}
```

### `Image` Constructor with Fit Modes

```aura
Image(
    src: String,               # File path to image
    path: String,              # Alternative to `src`
    width: Int,                # Width in pixels (default: 256)
    height: Int,               # Height in pixels (default: 256)
    fit: String,               # "stretch" | "contain" | "cover" (default: "stretch")
    tint: String,              # Color tint (default: "white")
    color: String              # Alternative to `tint`
)
```

**Fit Modes:**
- `"stretch"` (default): Fills the entire space, ignoring aspect ratio.
- `"contain"`: Preserves aspect ratio, letterboxes with background color if needed.
- `"cover"`: Preserves aspect ratio, crops to fill the space.

### Audio Built-ins (AVM)

```aura
# Load audio file and return clip ID
val clip_id: Int = audio.load("path/to/audio.mp3")

# Play an already-loaded clip, return playback handle
val handle: Int = audio.play_loaded(clip_id)

# Play directly from file path, return playback handle
val handle: Int = audio.play("path/to/audio.wav")

# Stop playback by handle
audio.stop(handle)
```

---

## Summary

- **Grid**: Perfect for responsive layouts, cards, galleries, and data tables.
- **Image.fit**: Choose `stretch` for fullscreen, `contain` for preserving aspect ratio, `cover` for filling space.
- **Audio**: Load and play audio via `audio.load()`, `audio.play()`, and `audio.stop()` in AVM mode.
