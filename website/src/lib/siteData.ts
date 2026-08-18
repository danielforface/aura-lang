export const GITHUB_URL = "https://github.com/danielforface/aura-lang";

export const STATUS = {
  edition: "2026 Edition",
  stability: "Active · pre-stable",
  workspaceVersion: "0.2.0",
  workspaceMembers: 22,
  protocolVersion: "Aura protocol v1",
  verification: "Z3-backed · feature-gated",
} as const;

export const WORKSPACE_MEMBERS = [
  ["aura", "CLI / orchestration"],
  ["aura-lex", "Indentation-aware lexer"],
  ["aura-parse", "Parser + edition/feature config"],
  ["aura-ast", "Source syntax representation"],
  ["aura-core", "Semantic core, lowering, diagnostics, safety analysis"],
  ["aura-ir", "Validated intermediate representation"],
  ["aura-verify", "Proof engine, Z3 integration, counterexamples"],
  ["aura-interpret", "AVM / development VM"],
  ["aura-backend-c", "C23-oriented backend"],
  ["aura-backend-llvm", "Feature-gated LLVM IR backend"],
  ["aura-rt", "Runtime"],
  ["aura-rt-native", "Native runtime support"],
  ["aura-stdlib", "Standard library"],
  ["aura-pkg", "Package manager"],
  ["aura-lsp", "Language server + proof protocol"],
  ["aura-sdk", "SDK crate"],
  ["aura-bridge", "FFI / native bridge"],
  ["aura-nexus", "Plugin host / integration layer"],
  ["aura-plugin-lumina", "Lumina UI/application plugin"],
  ["aura-plugin-ai", "Optional AI plugin"],
  ["aura-plugin-iot", "Optional IoT plugin"],
  ["aura-ai-opt", "AI-oriented optimization tooling"],
] as const;

export const PIPELINE = [
  {
    id: "source",
    label: "Source",
    crate: ".aura",
    detail: "Significant indentation, cells, contracts, refinements, explicit trust boundaries.",
  },
  {
    id: "frontend",
    label: "Frontend",
    crate: "lex → parse → AST",
    detail: "Indentation-aware tokenization and source structures remain backend-independent.",
  },
  {
    id: "semantics",
    label: "Semantics",
    crate: "aura-core",
    detail: "Lowering, control flow, diagnostics, ownership/move/capability infrastructure.",
  },
  {
    id: "ir",
    label: "Aura IR",
    crate: "aura-ir",
    detail: "Validation and optimization boundary shared by verification and execution paths.",
  },
  {
    id: "proof",
    label: "Proof",
    crate: "aura-verify",
    detail: "Proof obligations, Z3 profiles, counterexamples, summaries, region/linear analysis.",
  },
  {
    id: "execution",
    label: "Execution",
    crate: "AVM · C · LLVM",
    detail: "Development VM plus native-oriented C and evolving LLVM IR paths.",
  },
  {
    id: "feedback",
    label: "Feedback",
    crate: "aura-lsp · Sentinel",
    detail: "Structured diagnostics, proof streaming, source mapping and developer tooling.",
  },
] as const;

export const LANGUAGE_FEATURES = [
  {
    label: "Contracts",
    code: "requires · ensures · assert · assume",
    detail: "Correctness intent is visible at boundaries and inside functions.",
  },
  {
    label: "Refinements",
    code: "u32[0..100]",
    detail: "Range information can participate directly in proof obligations.",
  },
  {
    label: "Loop reasoning",
    code: "invariant · decreases",
    detail: "Loops can expose inductive invariants and termination-oriented hints.",
  },
  {
    label: "Explicit trust",
    code: "unsafe: · trusted extern cell",
    detail: "Foreign code is placed inside a visible trust boundary rather than hidden.",
  },
  {
    label: "Flow",
    code: "-> · ~>",
    detail: "The current reference distinguishes synchronous and asynchronous flow operators.",
  },
  {
    label: "Resource semantics",
    code: "Tensor · Model · Style",
    detail: "Current MVP rules describe move behavior for resource-like values.",
  },
] as const;

export const PROOF_STATES = ["start", "phase(parse)", "phase(sema)", "phase(normalize)", "phase(z3)", "done"] as const;

export const TOOLCHAIN_SURFACES = [
  {
    title: "Build & run",
    command: "aura build · aura run",
    body: "Profiles dev/release/verify; modes avm/llvm/hybrid; backend selection and native link inputs.",
    status: "code-backed",
  },
  {
    title: "Verify",
    command: "aura verify",
    body: "Feature-gated Z3 path with fast, ci and thorough SMT profiles plus trusted-core report output.",
    status: "code-backed",
  },
  {
    title: "Developer loop",
    command: "test · lint · fmt · init",
    body: "The primary CLI owns project initialization, Aura test discovery, linting and canonical formatting.",
    status: "code-backed",
  },
  {
    title: "Native bridge",
    command: "aura bindgen",
    body: "Bootstrap C/C++ header bridging with link inputs and best-effort refined type mapping.",
    status: "code-backed",
  },
  {
    title: "Language server",
    command: "aura-lsp",
    body: "Proof streaming, diagnostics, counterexample transport, cache and debugger/performance integration.",
    status: "code-backed",
  },
  {
    title: "LLVM path",
    command: "--mode llvm / --backend llvm",
    body: "Implemented and feature-gated, but still evolving. Current source should not be marketed as a finished optimizing backend.",
    status: "evolving",
  },
] as const;

export const ECOSYSTEM_SURFACES = [
  {
    title: "Aura Sentinel",
    eyebrow: "IDE",
    body: "Tauri 2 + CodeMirror desktop environment designed for proof-stream UX, diagnostics, counterexamples and debugger integration.",
    href: "/ecosystem#sentinel",
  },
  {
    title: "aura-pkg",
    eyebrow: "Packages",
    body: "Registry/resolver/cache/lockfile/security/signing modules with code-backed package command surfaces.",
    href: "/ecosystem#packages",
  },
  {
    title: "Nexus",
    eyebrow: "Plugins",
    body: "Plugin manifests and verification-aware extension paths keep domain integrations outside the core language.",
    href: "/ecosystem#nexus",
  },
  {
    title: "Lumina",
    eyebrow: "Applications",
    body: "Evolving UI/application layer with Raylib support, layout, input, grid, image/media and application examples.",
    href: "/ecosystem#lumina",
  },
  {
    title: "Android",
    eyebrow: "Platform",
    body: "SDK/NDK setup, runtime cross-build paths, sample APK workflow and emulator/build helper tooling.",
    href: "/ecosystem#android",
  },
  {
    title: "Release engineering",
    eyebrow: "Provenance",
    body: "Deterministic SDK ZIPs, SHA-256, attestation JSON, optional Windows signing and Sentinel sidecar staging.",
    href: "/downloads",
  },
] as const;

export const CLAIMS = {
  supported: [
    "Proof-driven systems programming language",
    "Z3-backed verification",
    "Structured counterexample mapping",
    "AVM + C-oriented execution paths",
    "Feature-gated evolving LLVM IR backend",
    "22-member Rust language-platform workspace",
    "Sentinel / LSP proof tooling",
    "Package, FFI, plugin and Android integration work",
  ],
  notClaimed: [
    "Formally verified compiler",
    "Frozen stable v1.0 language semantics",
    "Universal memory-safety guarantee",
    "Production JIT / automatic AVM → LLVM promotion",
    "Guaranteed <200ms proof latency across workloads",
    "Universal production-ready Android support",
  ],
} as const;

export const DOC_GROUPS = [
  {
    label: "Start",
    items: [
      ["getting-started", "Getting Started"],
      ["why-aura", "Why Aura?"],
    ],
  },
  {
    label: "Language",
    items: [
      ["language-reference", "Language Reference"],
      ["stdlib-and-modules", "Stdlib, Modules & Imports"],
      ["repl-and-avm", "REPL & AVM"],
    ],
  },
  {
    label: "Verification",
    items: [
      ["proof-system", "Proof System"],
      ["lsp-and-sentinel", "LSP & Sentinel"],
      ["sentinel-protocol", "Sentinel Protocol"],
    ],
  },
  {
    label: "Platform",
    items: [
      ["toolchain", "Toolchain & Project Layout"],
      ["universal-bridge", "Universal Bridge"],
      ["nexus", "Nexus Plugin Architecture"],
      ["ai-and-tensors", "AI & Tensors"],
    ],
  },
  {
    label: "Lumina",
    items: [
      ["lumina-ui", "Lumina UI"],
      ["lumina-media", "Lumina Media"],
      ["cookbook-lumina-ui", "Lumina Cookbook"],
      ["lumina-sentinel", "Lumina Sentinel"],
    ],
  },
  {
    label: "Plugins & examples",
    items: [
      ["plugins/aura-ai", "Plugin: aura-ai"],
      ["plugins/aura-iot", "Plugin: aura-iot"],
      ["demos", "Demos"],
    ],
  },
] as const;
