# Security Policy

Aura includes compiler, verifier, package, FFI, runtime and tooling surfaces. Security reports can therefore affect both ordinary implementation safety and the integrity of claims made by the proof system.

## Report privately

Please use GitHub's **Private vulnerability reporting / Security Advisory** flow for this repository when available.

Do not open a public issue for an unpatched vulnerability.

## High-priority classes

Examples include:

- verifier unsoundness that marks an invalid obligation as proved,
- proof-cache invalidation errors that reuse stale results,
- trusted-boundary bypass,
- `unsafe`/FFI enforcement bypass,
- ownership/move/capability enforcement bypass,
- package signature or hash-verification bypass,
- registry/path traversal or archive extraction vulnerabilities,
- code execution through package/install tooling,
- LSP/Sentinel command injection,
- malformed source causing exploitable native behavior,
- release signing/provenance issues.

## Include

A useful report contains:

- affected commit/version,
- affected platform,
- minimal reproduction,
- expected vs actual behavior,
- impact,
- whether proof/safety status is misreported,
- suggested mitigation if known.

## Proof-system severity

A verifier crash is important.

A verifier that returns **proved** for an invalid property is potentially more severe because it undermines the trust model. Please call out false-proof behavior explicitly.

## Disclosure

Please allow time for diagnosis and a coordinated fix before public disclosure. The project should publish a clear advisory when a fix is available and identify which releases/commits are affected.
