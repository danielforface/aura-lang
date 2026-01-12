# Aura Package Manager v1.0 - Complete Guide

## Overview

Aura is a modern Rust package manager designed for safety, performance, and ease of use. Built with cryptographic verification, advanced dependency resolution, and comprehensive caching.

## Quick Start

### Installation

```bash
cargo install aura-pkg
```

### First Package

```bash
# Create a new package
aura init my-awesome-lib

# Add dependencies
aura add serde@^1.0.0
aura add tokio@^1.0.0

# Build and verify
aura verify

# Publish to registry
aura publish --dry-run
aura publish
```

## Features

### 1. Type-Safe Package Management
- Built entirely in Rust
- Zero-copy dependency resolution
- Compile-time verification where possible

### 2. Cryptographic Verification
- Ed25519 digital signatures on all packages
- Manifest integrity checking
- Published package verification

### 3. Advanced Dependency Resolution
- Semantic versioning (semver) support
- Caret ranges (`^1.2.3`)
- Tilde ranges (`~1.2.3`)
- Wildcard ranges (`1.2.*`)
- Complex multi-package dependency graphs
- Automatic conflict detection

### 4. High-Performance Caching
- LRU cache with TTL-based expiration (5 minute default)
- Resolution result caching
- Thread-safe parallel operations
- Lazy-evaluated computations

### 5. Registry Integration
- HTTP registry client with optional authentication
- Local file-based registry for development
- Bearer token authentication support
- Package search and discovery

### 6. Configuration Management
- Per-user configuration in `~/.aura/config.toml`
- Multiple registry support
- Customizable timeouts and verification options
- TOML-based configuration format

## Command Reference

### init
Initialize a new package

```bash
aura init <name>
  --edition 2024       # Edition (default: 2024)
  --registry <url>     # Custom registry URL
  --license <id>       # License (default: MIT)
```

### add
Add a dependency

```bash
aura add <package>[@version]
  --dev               # Add as dev dependency
  --optional          # Optional dependency
```

### remove
Remove a dependency

```bash
aura remove <package>
  --dev              # From dev dependencies
```

### list
List all dependencies

```bash
aura list
  --locked           # Show locked versions
  --tree             # Show dependency tree
```

### verify
Verify package integrity

```bash
aura verify
  --offline          # Skip registry checks
  --check-sigs       # Verify signatures
```

### publish
Publish package to registry

```bash
aura publish
  --registry <url>   # Custom registry
  --token <token>    # Authentication token
  --dry-run          # Show what would be published
```

## Configuration

### Basic Configuration

Create `~/.aura/config.toml`:

```toml
default_registry = "https://registry.aura.dev"

[registries.official]
url = "https://registry.aura.dev"
token = "aur_XXXXXXXXXXXX"

[settings]
check_signatures = true
require_license = true
offline = false
verbose = false
http_timeout = 30
```

### Registry Configuration

```toml
[registries.custom]
url = "https://private-registry.company.com"
token = "private_token_here"
index = "https://index.company.com"

[registries.local]
url = "file:///home/user/.aura/local-registry"
```

### Global Settings

```toml
[settings]
# Require cryptographic verification of all packages
check_signatures = true

# Require license information in Package.toml
require_license = true

# Work offline (use cached data only)
offline = false

# Enable verbose logging
verbose = true

# HTTP request timeout in seconds
http_timeout = 30
```

## Package Format

### Package.toml

```toml
[package]
name = "my-awesome-lib"
version = "1.0.0"
edition = "2024"
authors = ["John Doe <john@example.com>"]
description = "A description of what my library does"
license = "MIT"

[dependencies]
serde = "^1.0.0"
tokio = { version = "^1.0", features = ["full"] }

[dev-dependencies]
criterion = "^0.5"
```

### Manifest Fields

- `name`: Package name (required, must match `[a-zA-Z0-9_\-\.]{1,64}`)
- `version`: Semantic version (required, format: `MAJOR.MINOR.PATCH`)
- `edition`: Edition year (required, default: 2024)
- `authors`: Author list with optional email (optional)
- `description`: Package description (optional, max 500 chars)
- `license`: SPDX license identifier (optional)
- `repository`: Git repository URL (optional)
- `homepage`: Project homepage (optional)

## Version Resolution

### Semantic Versioning

Aura strictly follows semantic versioning:

```
MAJOR.MINOR.PATCH(-PRERELEASE)(+BUILD)

Examples:
1.0.0           # Stable release
1.0.0-alpha.1   # Pre-release
1.0.0+build.1   # Build metadata
```

### Version Requirements

```
1.2.3           # Exact version (caret range: ^1.2.3)
^1.2.3          # Caret range (allow changes that don't break left-most non-zero)
~1.2.3          # Tilde range (allow patch-level changes)
1.2.*           # Wildcard (allow any patch version)
1.*             # Major wildcard (allow any minor/patch)
*               # Any version
>=1.2.0         # Greater than or equal
<2.0.0          # Less than
```

### Caret Range Rules

```
^1.2.3  => >=1.2.3, <2.0.0  (allow minor/patch updates)
^0.2.3  => >=0.2.3, <0.3.0  (allow patch updates only)
^0.0.3  => >=0.0.3, <0.0.4  (no updates allowed)
```

### Tilde Range Rules

```
~1.2.3  => >=1.2.3, <1.3.0  (allow patch-level changes only)
~1.2    => >=1.2.0, <1.3.0
~1      => >=1.0.0, <2.0.0
```

## Security

### Package Verification

All published packages go through security verification:

```bash
# Automatic verification
aura verify

# Manual signature check
aura verify --check-sigs

# Offline verification (no registry check)
aura verify --offline
```

### Security Checks

1. **Package Integrity**: SHA256 hashing of all files
2. **Signature Verification**: Ed25519 digital signatures
3. **Name Validation**: Prevent confusing/reserved names
4. **Size Limits**: Maximum 100MB per package, 50MB per file
5. **Executable Prevention**: No binary files in packages
6. **Path Traversal Prevention**: Reject `../` in paths

## Performance

### Caching Strategy

Aura implements intelligent multi-level caching:

```
1. In-Memory Cache (LRU)
   - Dependency resolution results
   - Version lookups
   - Registry responses
   - TTL: 5 minutes (configurable)

2. On-Disk Cache (Lockfile)
   - Locked dependency versions
   - Installation order
   - Persistent across sessions

3. Remote Cache (Registry)
   - Package metadata
   - Available versions
   - Package signatures
```

### Performance Tips

1. Use lockfile for reproducible builds
2. Enable caching for offline operation
3. Use tilde ranges for stable dependencies
4. Minimize transitive dependencies
5. Pre-fetch registry metadata

## Examples

### Creating a Library

```bash
# Initialize
aura init my-lib

# Add dependencies
aura add serde --features derive
aura add tokio --features rt

# Create source files
mkdir -p src
echo 'pub fn hello() { println!("Hello!"); }' > src/lib.rs

# Verify
aura verify

# Publish
aura publish --dry-run
aura publish --token $AURA_TOKEN
```

### Using Private Registry

```bash
# Configure registry
aura init my-app

# Add to config
cat >> ~/.aura/config.toml << EOF
[registries.internal]
url = "https://registry.internal.company.com"
token = "internal_token"
EOF

# Update default registry
# Edit ~/.aura/config.toml: default_registry = "https://registry.internal.company.com"

# Add from private registry
aura add internal-lib
```

### Dependency Management

```bash
# Add with version constraint
aura add async-trait@^0.1.0

# Add development dependency
aura add tokio-test --dev

# List all dependencies
aura list

# View dependency tree
aura list --tree

# Remove dependency
aura remove tokio-test
```

## Troubleshooting

### Package Not Found

```
Error: Package 'unknown-pkg' not found in registry
Solution: Check package name spelling and registry configuration
```

### Version Conflict

```
Error: No version of 'lib' matches requirement '^2.0'
Solution: Check available versions with `aura list --locked`
         or adjust version requirement
```

### Signature Verification Failed

```
Error: Package signature verification failed
Solution: Check package integrity with `aura verify --check-sigs`
         Update registry and retry
```

### Registry Unreachable

```
Error: Failed to connect to registry
Solution: Check network connectivity and registry URL
         Use `aura verify --offline` to work with cached data
```

## Advanced Topics

### Custom Registry Setup

1. Create registry server supporting HTTP API
2. Implement required endpoints:
   - `GET /api/packages/<name>` - Package info
   - `GET /api/packages/<name>/versions` - Available versions
   - `PUT /api/packages/<name>/<version>` - Upload package
3. Configure Aura to use your registry

### Performance Tuning

```toml
[settings]
# Increase cache TTL for stable environments
cache_ttl = 3600  # 1 hour

# Adjust HTTP timeout for slow networks
http_timeout = 60  # seconds

# Disable signature checking for internal packages
# (not recommended for production)
check_signatures = false
```

### Git Integration

```bash
# Add git repository as dependency (future feature)
aura add my-lib --git https://github.com/user/repo.git
```

## Best Practices

1. **Always use version constraints**: Don't use bare version numbers
2. **Lock dependencies**: Commit Aura.lock to version control
3. **Verify packages**: Always run `aura verify` before publishing
4. **Use HTTPS registries**: Never use unencrypted registries
5. **Update regularly**: Keep dependencies current
6. **Document dependencies**: Explain why each dependency exists
7. **Minimize transitives**: Reduce indirect dependency count
8. **Test with lockfile**: Build with locked versions before release

## API Reference

### DependencyResolver

```rust
use aura_pkg::resolver::{DependencyResolver, PackageRegistry};

let mut registry = PackageRegistry::new();
// ... register packages ...

let resolver = DependencyResolver::new(registry);
let resolved = resolver.resolve(
    "my-app",
    &Version::parse("1.0.0").unwrap(),
    &dependencies,
)?;
```

### SecurityValidator

```rust
use aura_pkg::security::SecurityValidator;

let validator = SecurityValidator::new();
validator.validate_package_name("my-lib")?;
validator.validate_file_path("src/main.rs")?;
```

### DependencyCache

```rust
use aura_pkg::cache::DependencyCache;
use std::time::Duration;

let cache = DependencyCache::new(Duration::from_secs(300));
cache.set_versions("serde".to_string(), versions);
if let Some(v) = cache.get_versions("serde") {
    println!("Cached versions: {:?}", v);
}
```

## Contributing

For security issues, please email security@aura.dev

For other contributions:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## License

Licensed under the MIT License. See LICENSE file for details.
