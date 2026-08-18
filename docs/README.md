# Aura Documentation Index

Aura has accumulated design docs, completion reports, SDK references, generated release copies and platform-specific guides. This index identifies where readers should start.

## Start here

From repository root:

- [`../PROJECT_STATUS.md`](../PROJECT_STATUS.md) — current implementation/status reconciliation
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — architecture
- [`../LANGUAGE_GUIDE.md`](../LANGUAGE_GUIDE.md) — current language tour
- [`../VERIFICATION_MODEL.md`](../VERIFICATION_MODEL.md) — verification/trust model
- [`../TOOLCHAIN.md`](../TOOLCHAIN.md) — actual CLI/backend surface
- [`../ECOSYSTEM.md`](../ECOSYSTEM.md) — package/IDE/plugin/release ecosystem
- [`../ANDROID_SUPPORT.md`](../ANDROID_SUPPORT.md) — Android scope
- [`../PUBLIC_CLAIMS_POLICY.md`](../PUBLIC_CLAIMS_POLICY.md) — evidence vocabulary
- [`../REPOSITORY_MAP.md`](../REPOSITORY_MAP.md) — monorepo navigation
- [`../ROADMAP_PUBLIC.md`](../ROADMAP_PUBLIC.md) — concise forward roadmap

## Current compact language reference

Prefer:

- [`../sdk/docs/reference.md`](../sdk/docs/reference.md)
- [`../sdk/docs/verifier-guide.md`](../sdk/docs/verifier-guide.md)
- [`../sdk/docs/z3-gate.md`](../sdk/docs/z3-gate.md)
- [`../sdk/docs/lsp-stability.md`](../sdk/docs/lsp-stability.md)

## Focused subsystem docs

- [`debug-protocol.md`](debug-protocol.md)
- [`effects-ownership-model.md`](effects-ownership-model.md)
- [`gc-design.md`](gc-design.md)
- [`lumina-ui.md`](lumina-ui.md)
- [`lumina-media.md`](lumina-media.md)
- [`cookbook-lumina-ui.md`](cookbook-lumina-ui.md)
- [`release-channels.md`](release-channels.md)

## Book material

`book/` contains longer-form guides including verification, debugging, package management and UI material.

Important: some book/package examples can describe a broader designed command surface than the current CLI. Check `aura --help` / `aura-pkg --help` before treating a command as implemented.

## Historical reports

Files named around:

```text
PHASE-*
WEEK-*
*_COMPLETE*
*_SUMMARY*
JANUARY-2026-STATUS
```

are development/milestone evidence. They are not automatically normative current documentation.

## Generated copies

Search may also return copies under distribution trees such as:

```text
dist-release/
dist-complete/
```

Prefer canonical source docs outside generated distribution trees.

## Documentation rule

When a historical document and current code disagree:

> current implementation and compact reference win.
