import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// scripts/ is inside website/
const WEBSITE = path.resolve(__dirname, "..");
const OUT_DIR = path.join(WEBSITE, "content", "docs", "plugins");

/**
 * Minimal registry of built-in Nexus plugins.
 * Keep this as the single source of truth for website docs generation.
 */
const PLUGINS = [
  {
    id: "aura-iot",
    title: "Aura IoT Plugin",
    description: "Hardware/MMIO safety checks (capabilities, offsets, bitmasks).",
    enableSnippet: `plugins = [\n  { name = \"aura-iot\", trusted = true },\n]`,
    example: `import aura::hw\n\ncell main() ->:\n    val cap = hw.open(\"SPI_CTRL\")\n    val _ = hw.write_u32(cap, 0, 0x3FF)`,
    notes: [
      "Emits informational proof diagnostics for verified bitmask/offset facts.",
      "Uses the nearest aura.toml [hardware] manifest to learn register masks.",
    ],
  },
  {
    id: "aura-ai",
    title: "Aura AI Plugin",
    description: "Tensor and ONNX shape checks (model contracts, safe inference).",
    enableSnippet: `plugins = [\n  { name = \"aura-ai\", trusted = true },\n]`,
    example: `import aura::tensor\nimport onnxruntime\n\ncell main() ->:\n    val model: Model = ai.load_model(\"identity_u32_2x2x3.onnx\")\n    val input: Tensor<u32, [2, 2, 3]> = tensor::new<u32>(12)\n    val out: Tensor<u32, [2, 2, 3]> = model.infer(input)`,
    notes: [
      "Emits informational proof diagnostics when shapes are proven compatible.",
      "In VS Code, these appear as plugin-attributed verified overlays.",
    ],
  },
];

function mdxForPlugin(p) {
  const notes = p.notes.map((n) => `- ${n}`).join("\n");
  return `---\ntitle: ${p.title}\ndescription: ${p.description}\n---\n\n## Enable\n\nAdd this to your \`aura.toml\`:\n\n\`\`\`toml\n${p.enableSnippet}\n\`\`\`\n\n## What it verifies\n\n${notes}\n\n## Example\n\n\`\`\`aura\n${p.example}\n\`\`\`\n\n## Editor feedback\n\nWhen verification succeeds, the LSP publishes informational diagnostics tagged with the plugin id (\`${p.id}\`). Aura Sentinel uses these to render gutter icons, hovers, and inlay hints.\n`;
}

function main() {
  fs.mkdirSync(OUT_DIR, { recursive: true });

  for (const p of PLUGINS) {
    const outPath = path.join(OUT_DIR, `${p.id}.mdx`);
    fs.writeFileSync(outPath, mdxForPlugin(p), "utf8");
  }

  // Keep directory tidy by removing stale plugin pages for plugins we no longer generate.
  const expected = new Set(PLUGINS.map((p) => `${p.id}.mdx`));
  for (const name of fs.readdirSync(OUT_DIR)) {
    if (name.endsWith(".mdx") && !expected.has(name)) {
      fs.unlinkSync(path.join(OUT_DIR, name));
    }
  }
}

main();
