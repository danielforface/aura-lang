export type DocsNavItem = {
  slug: string;
  title: string;
};

export const docsNav: DocsNavItem[] = [
  { slug: "getting-started", title: "Getting Started" },
  { slug: "toolchain", title: "Toolchain & Project Layout" },
  { slug: "language-reference", title: "Language Reference" },
  { slug: "stdlib-and-modules", title: "Stdlib, Modules, and Imports" },
  { slug: "proof-system", title: "The Proof System" },
  { slug: "nexus", title: "Nexus Plugin Architecture" },
  { slug: "lumina-ui", title: "Lumina UI (Grid, Image, Layout)" },
  { slug: "lumina-media", title: "Lumina Media (Audio, Video, Files)" },
  { slug: "cookbook-lumina-ui", title: "Cookbook: Lumina UI Patterns" },
  { slug: "lumina-sentinel", title: "Lumina Sentinel (Proof → Pixels)" },
  { slug: "lsp-and-sentinel", title: "LSP + Aura Sentinel (VS Code)" },
  { slug: "ai-and-tensors", title: "AI & Tensors" },
  { slug: "plugins/aura-ai", title: "Plugin: aura-ai" },
  { slug: "plugins/aura-iot", title: "Plugin: aura-iot" },
  { slug: "universal-bridge", title: "The Universal Bridge" },
  { slug: "repl-and-avm", title: "REPL + AVM Interpreter" },
  { slug: "demos", title: "Demos" },
  { slug: "why-aura", title: "Why Aura?" },
];
