# Aura Android Support

Aura has real Android-oriented tooling in the repository, but the public claim should be scoped precisely.

## What exists

### Android CI workflow

The repository contains an Android workflow that can:

- provision Java 17,
- install/cache Android SDK/NDK tooling,
- build a sample APK on Windows,
- upload the APK as a workflow artifact,
- cross-compile `aura-rt` for Android targets.

### Runtime targets exercised by the workflow

```text
aarch64-linux-android
armv7-linux-androideabi
```

### APK tooling

The current tree/latest main work includes helper tooling for:

- setup/readiness checks,
- APK building,
- emulator management,
- build/run automation.

### Lumina/media example

The current example set includes `grid_image_audio.aura`, demonstrating application-level work around grid layout, images and audio controls.

## What the repository evidence supports saying

Good public wording:

> Aura includes Android SDK/NDK tooling, runtime cross-compilation paths, sample APK automation, and emulator/build helpers.

or:

> Android is an active platform target with a repository-backed sample APK pipeline.

## What should not be claimed without additional release evidence

Avoid:

> full production Android support for all Aura applications

unless a release matrix demonstrates end-to-end compiler/runtime/library/UI behavior across supported Android ABIs/devices.

Avoid:

> Aura compiles directly to APK

if the actual path includes generated/native artifacts plus Android Gradle/SDK packaging. Prefer to describe the pipeline, not compress it into an inaccurate phrase.

## Suggested Android qualification matrix

Before promoting Android to stable platform status, publish a matrix like:

| Layer | aarch64 | armv7 | x86_64 emulator | Device run | CI |
|---|---:|---:|---:|---:|---:|
| `aura-rt` build | required | required | recommended | n/a | required |
| hello-world Aura app | required | required | required | required | required |
| FFI | required | required | required | required | required |
| Lumina basic UI | required | optional | required | required | required |
| input | required | optional | required | required | required |
| image/audio | required | optional | required | required | required |
| release APK install | required | optional | required | required | required |

## Android documentation links

Keep detailed setup documentation close to the existing SDK/APK tooling rather than duplicating commands here. This file is the public scope statement.
