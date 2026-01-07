# Package Management Guide

## Introduction

Aura packages provide reusable libraries with integrated **verification, security scanning, and dependency resolution**.

### Key Features

- **Deterministic dependencies** — `aura.lock` pins exact versions
- **Signature verification** — Optional ed25519 signing for security-critical packages
- **Trusted boundary reports** — Understand what code must be trusted vs. verified
- **Offline-first** — Cache dependencies locally; work without network
- **Version resolution** — Semver constraints with conflict detection

---

## Quick Start

### 1. Create a New Aura Project

```bash
aura new my_app
cd my_app
cat aura.toml
```

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2026"

[dependencies]
# Add dependencies here

[verify]
profile = "fast"     # or "thorough" for CI
cache = true         # Enable incremental caching
```

### 2. Add a Dependency

```bash
aura pkg add serde
```

This:
1. Queries registry for latest `serde` version
2. Downloads source + checks SHA256
3. Writes entry to `aura.lock` (commit this!)
4. Makes `serde` available to import

You can also specify version:

```bash
aura pkg add serde@1.0.0
aura pkg add serde@">=1.0,<2.0"
aura pkg add serde --git https://github.com/serde-rs/serde.git
```

### 3. Use the Dependency

```aura
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

fn main() {
    let p = Point { x: 1.0, y: 2.0 };
    let json = serde::to_json(&p);
    println!("{}", json);
}
```

### 4. Build and Run

```bash
aura build
aura run
```

---

## Manifest File (`aura.toml`)

### Basic Structure

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2026"
authors = ["Your Name <you@example.com>"]
description = "A cool Aura application"
license = "MIT OR Apache-2.0"

[dependencies]
serde = "1.0"                                    # Any 1.x version
tokio = { version = "1.0", features = ["rt"] }  # With features
my_lib = { path = "../my_lib" }                 # Local path

[dev-dependencies]
criterion = "0.5"  # Testing only

[build]
profile = "release"
opt_level = 3

[verify]
profile = "thorough"   # fast | thorough
cache = true
timeout = 5000         # ms per function
```

### Dependency Specification

You can specify dependencies in multiple ways:

```toml
# 1. Simple version
serde = "1.0"

# 2. Version range (semver)
tokio = ">=1.0,<2.0"
bytes = "^1.0"     # Means >=1.0,<2.0

# 3. Path (local development)
my_lib = { path = "../my_lib" }

# 4. Git
clap = { git = "https://github.com/clap-rs/clap.git", branch = "master" }

# 5. With features
serde = { version = "1.0", features = ["derive", "json"] }
```

### Build Profiles

```toml
[profile.dev]
opt_level = 0          # No optimization
debug = true           # Debug symbols
lto = false            # Link-time optimization off

[profile.release]
opt_level = 3          # Maximum optimization
debug = false
lto = true

[profile.verify]       # Verification-specific
profile = "thorough"   # Solver profile
cache = true
```

---

## Lock File (`aura.lock`)

The lock file records **exact versions and hashes**:

```toml
version = 1
created = "2026-01-07T14:23:00Z"

[[packages]]
name = "serde"
version = "1.0.195"
url = "https://pkg.auralang.org/serde/1.0.195.zip"
sha256 = "a1c9...8f2c"
signature = "3021...7c5a"  # Optional
signature_key_id = "2024-jan"

[[packages]]
name = "tokio"
version = "1.35.0"
url = "https://pkg.auralang.org/tokio/1.35.0.zip"
sha256 = "f19d...4e6b"
```

### Why Lock Files Matter

1. **Reproducibility** — Everyone builds the same version
2. **Security** — SHA256 verifies integrity
3. **Offline builds** — No network needed on subsequent builds
4. **CI determinism** — Builds are identical across machines

### Updating Lockfile

```bash
aura pkg update          # Update all to latest
aura pkg update serde    # Update only serde
```

After updating, review diffs:

```bash
git diff aura.lock       # See what changed
aura pkg audit           # Check for security issues
```

---

## Version Resolution

### Semver Constraints

Aura uses semantic versioning with constraint syntax:

| Constraint | Meaning | Examples |
| --- | --- | --- |
| `1.0` | Exactly 1.0 | `1.0` |
| `^1.0` | Compatible release (>=1.0, <2.0) | `1.5`, `1.99` ✓ / `2.0` ✗ |
| `~1.2` | Patch release (>=1.2, <1.3) | `1.2.5` ✓ / `1.3` ✗ |
| `>=1.0` | Greater than or equal | `1.0`, `1.5`, `2.0` ✓ |
| `<2.0` | Less than | `1.99` ✓ / `2.0` ✗ |
| `>=1.0,<2.0` | Range | `1.5` ✓ / `0.9` or `2.0` ✗ |
| `*` | Any version (not recommended) | `0.1`, `5.0` ✓ |

### Conflict Resolution

If two dependencies request incompatible versions, `aura pkg` fails with clear error:

```
ERROR: Version conflict

  Package: serde
    - my_app depends on serde ^1.0 (i.e., >=1.0, <2.0)
    - tokio/1.35 depends on serde >=0.9, <1.5

  Intersection: >=1.0, <1.5

  Resolution: Install serde 1.4.0 (compatible with both)
```

---

## Signature Verification

### Signing a Release

For security-critical packages (cryptography, networking):

```bash
aura pkg publish \
  --sign \
  --key-id 2024-jan \
  --private-key ~/.aura/private.key \
  my_secure_lib
```

Generates:

```toml
[[packages]]
name = "my_secure_lib"
version = "1.0.0"
url = "https://pkg.auralang.org/my_secure_lib/1.0.0.zip"
sha256 = "abc123..."
signature = "3021...7c5a"           # ed25519 signature
signature_key_id = "2024-jan"        # Public key identifier
```

### Verifying Signatures

In your project:

```bash
aura pkg add my_secure_lib \
  --verify-signatures \
  --trusted-keys ~/.aura/trusted_keys.json
```

Or in `aura.toml`:

```toml
[package]
verify_signatures = true

[trusted_keys]
"2024-jan" = "c9ae...1f2a"   # Public key for "2024-jan"
```

The package manager:
1. Downloads artifact
2. Fetches signature from registry
3. Verifies: `signature == sign(sha256, private_key)`
4. Only proceeds if valid

---

## Trusted Boundary Reports

Every package includes a **trusted boundary report** — which parts are verified vs. trusted.

### Generate a Report

```bash
aura pkg inspect serde --trusted-boundary
```

Output:

```
Package: serde/1.0.195

Trusted Code (C FFI, unsafe blocks):
  - src/lib/ffi.aura (45 LOC)
    Foreign function binding to libc
    Risk: Memory safety
    Audited: No

Verified Code (100% proof coverage):
  - src/lib/core.aura (1,203 LOC)
  - src/lib/collections.aura (892 LOC)
  Total: 2,095 LOC

Partially Verified:
  - src/lib/serialize.aura (445 LOC)
  Coverage: 89%

Summary:
  ✗ Not fully verified (trusted FFI boundary)
  ✓ Signatures: Present and valid
  ✓ Dependencies: All signed

Risk Assessment: MEDIUM (trusted FFI, but small attack surface)
```

### CI Gate: Trusted Core Policy

In CI, enforce policies:

```bash
aura pkg audit \
  --deny-untrusted \           # Fail if any untrusted code
  --require-signatures \        # All packages must be signed
  --min-coverage 90            # Minimum 90% proof coverage
```

---

## Publishing Your Package

### Prepare Your Package

1. **Update version** in `aura.toml`
2. **Write CHANGELOG**
3. **Run tests** — `aura test`
4. **Verify** — `aura verify`
5. **Review proof coverage** — `aura verify --report coverage`

### Create Registry Account

```bash
aura pkg login https://pkg.auralang.org

# Authenticates and stores token in ~/.aura/credentials.toml
```

### Publish

```bash
aura pkg publish \
  --registry https://pkg.auralang.org \
  --sign \                        # Optional
  --dry-run                       # Preview
```

### Monitor

```bash
# Check download stats
aura pkg stats my_package

# View package page
aura pkg info my_package --web   # Opens browser
```

---

## Dependency Audit

### Security Advisories

Check for known vulnerabilities:

```bash
aura pkg audit
```

Output:

```
Checking dependencies for known vulnerabilities...

WARNING: tokio/1.10 - RUSTSEC-2021-0089
  Memory safety issue in OS-level signal handler
  Affected versions: <1.11
  Recommendation: Update to 1.11 or later

CRITICAL: openssl-sys/0.9 - RUSTSEC-2021-0146
  OpenSSL vulnerability CVE-2021-1234
  Status: Can fix by updating to openssl-sys/0.9.72

Summary: 1 critical, 1 warning
Action: Run `aura pkg update` to fix
```

### Proof Coverage Audit

Check that dependencies have sufficient proof coverage:

```bash
aura pkg audit --proof-coverage

# By default: warn if <80% coverage
# Options: --min-coverage 90
```

---

## Offline Usage

### Cache Dependencies Locally

Once downloaded, packages are cached:

```
~/.aura/pkg-cache/
  ├── serde/1.0.195.zip
  ├── tokio/1.35.0.zip
  └── ...
```

### Build Without Network

```bash
# Offline build (uses cache)
aura build --offline

# Error if dependency not cached:
# ERROR: Package 'serde' not in cache. Run 'aura build --online' first.
```

### CI Workflow

In CI, commit `aura.lock` but not cached files:

```bash
# On developer machine
aura build
git add aura.lock
git commit -m "Update dependencies"

# In CI
git clone ...
aura build --offline  # Uses lock file to restore exact versions
```

---

## Troubleshooting

### "Version conflict"

```
aura pkg add my_lib
ERROR: Version conflict: my_lib^1.0 conflicts with existing dependency
```

**Solution:**

```bash
aura pkg update my_lib  # Update to compatible version
# or manually edit aura.toml to compatible constraint
```

### "Package not found"

```
ERROR: Package 'my_lib' not found in registry
```

**Solution:**

```bash
aura pkg search my_lib    # Search registry
# or publish it first
aura pkg publish my_lib
```

### "Signature verification failed"

```
ERROR: Signature verification failed for tokio/1.35
```

**Solution:**

```bash
# Check your trusted keys:
cat ~/.aura/trusted_keys.json

# Update from package registry:
aura pkg update --refresh-keys

# Or remove signature requirement:
# Edit aura.toml: verify_signatures = false
```

### "SHA256 mismatch"

```
ERROR: Downloaded tokio/1.35 has sha256 'abc...', expected 'xyz...'
```

**Solution:**

```bash
# Likely corrupted download
aura pkg update tokio --force

# Check network and cache:
rm ~/.aura/pkg-cache/tokio/1.35.0.zip
aura build  # Re-download
```

---

## Best Practices

### 1. Use Exact Versions in Libraries

If you're **publishing a library**, use exact versions:

```toml
[dependencies]
serde = "1.0.195"  # Exact, not "^1.0"
```

This prevents version skew in downstream consumers.

### 2. Use Ranges in Applications

If you're **publishing an application**, use ranges:

```toml
[dependencies]
serde = "^1.0"     # Allow 1.x versions
tokio = "^1.0"
```

This lets the resolver find compatible combinations.

### 3. Commit Your Lock File

Always commit `aura.lock`:

```bash
git add aura.lock
git commit -m "Lock dependencies"
```

This ensures reproducible builds.

### 4. Review Dependency Diffs

When updating:

```bash
aura pkg update
git diff aura.lock    # Review changes
aura pkg audit        # Check for security issues
aura build --offline  # Verify it builds
```

### 5. Minimize Trusted Code

When adding a dependency with unsafe code:

```bash
aura pkg inspect tokio --trusted-boundary

# If too much untrusted code, consider alternatives:
aura pkg search async-runtime  # Find alternatives
```

### 6. Sign Security-Critical Packages

If publishing cryptography or security code:

```bash
aura pkg publish --sign --key-id $(date +%Y-%m)
```

---

## Advanced: Custom Package Sources

### Local Registry

For internal/private packages:

```toml
[registries]
internal = { url = "file:///internal/registry" }

[dependencies]
my_internal_lib = { version = "1.0", registry = "internal" }
```

### Git Dependencies

For development / pre-release versions:

```toml
[dependencies]
serde = { git = "https://github.com/serde-rs/serde.git", branch = "master" }
```

**Caution:** Git dependencies are not included in `aura.lock` by default. Use `--locked` flag to pin them.

---

## Next Steps

- **Publishing guide** — `aura pkg publish --help`
- **Verification** — Chapter 10: Verification & Proof-Driven Development
- **Standard library** — Chapters 14–16: Core, Collections, Network, Concurrency
