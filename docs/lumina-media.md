# Lumina Media (Roadmap)

This document tracks the planned media APIs for Lumina apps (images, audio, and video).

Status: roadmap / design draft.

## Goals

- Keep the core language/runtime small.
- Provide a clean Aura-level API.
- Make assets easy to bundle for release builds.
- Avoid blocking the UI thread (decode/load off-thread where needed).

## Images

### UI node

- `Image(src: String, width: Int = 256, height: Int = 256)`

Planned additions:
- `fit: "stretch"|"contain"|"cover"` (implemented)
- `tint: Color` (implemented)
- `radius` and `clip` for rounded images

### Caching

- Window-local texture cache keyed by `src`.
- Optional file-watch for hot reload in dev mode.

## Audio

### Current AVM MVP

- `audio.play(path: String) -> U32`
- `audio.load(path: String) -> U32`
- `audio.play_loaded(clip: U32) -> U32`
- `audio.stop(handle: U32) -> Unit`

Notes:
- Implemented in the AVM interpreter via `rodio`.
- IDs/handles are small integers.

### Longer-term API sketch

- `audio.load(path: String) -> AudioClip`
- `audio.play(clip: AudioClip) -> AudioHandle`
- `audio.stop(handle: AudioHandle) -> Unit`
- `audio.set_volume(handle: AudioHandle, vol: Float) -> Unit`

Notes:
- Keep clips small in-memory (SFX).
- Use streaming for music.

## Video

### API sketch

- `video.open(path: String) -> Video`
- `video.play(v: Video) -> Unit`
- `video.pause(v: Video) -> Unit`
- `video.seek(v: Video, seconds: Float) -> Unit`
- `video.frame_texture(v: Video) -> ImageHandle` (or similar)

Notes:
- MVP can be decode-to-frames and blit into an `Image`/texture.
- Audio/video sync is a separate milestone.
