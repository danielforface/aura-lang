# Aura Support

## Usage questions

For reproducible bugs, use GitHub Issues and select the most specific template.

Before filing:

```bash
git rev-parse HEAD
cargo run -p aura -- --version
cargo run -p aura -- --help
```

Include the Aura source file or a minimal reproduction when possible.

## Where to file

- compiler/parser/type errors → **Compiler bug**
- incorrect proof / counterexample / solver behavior → **Verifier bug**
- LSP/Sentinel/package/build/platform issue → **Tooling bug**
- syntax/type/semantic proposal → **Language design**

## Security

Security vulnerabilities do not belong in public issues. See [SECURITY.md](SECURITY.md).

## Historical documentation

If a command from an older roadmap/completion document does not exist, first check:

- `aura --help`,
- `aura pkg --help`,
- standalone `aura-pkg --help`,
- `sdk/docs/reference.md`,
- [PROJECT_STATUS.md](PROJECT_STATUS.md).

The repository contains historical and generated documentation copies, so a search result is not automatically the current executable contract.
