# Release channels

This repo supports multiple release channels to balance stability vs iteration speed.

## Channels
- **nightly**: automated builds from main; may be unstable.
- **beta**: release candidates for the next stable; limited breaking changes.
- **stable**: tagged releases with compatibility expectations.

## Artifacts
- SDK zip
- Sentinel installer
- VSCode VSIX

## Versioning
- Prefer SemVer for tooling and SDK artifacts.
- Language features should be gated (feature flag or edition) when behavior could break existing code.

## Promotion policy (suggested)
- nightly → beta when CI is green and manual smoke checks pass
- beta → stable when:
  - no known regressions in proofs/differential tests
  - installer + VSIX build and install cleanly
  - release notes are written (see docs/release-notes.md)
