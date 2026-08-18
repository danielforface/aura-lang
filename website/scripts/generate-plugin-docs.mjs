import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const WEBSITE = path.resolve(__dirname, "..");
const OUT_DIR = path.join(WEBSITE, "content", "docs", "plugins");

// Public website registry for built-in Nexus plugin documentation.
// Wording is deliberately implementation-scoped: a plugin integration is not
// a universal guarantee for every device/model/program in that domain.
const PLUGINS = [
  {
    id: "aura-iot",
    title: "Aura IoT Plugin",
    description: "IoT/MMIO-oriented Nexus integration with verification-aware capability and register checks.",
    enableSnippet: `plugins = [\n  { name = "aura-iot", trusted = true },\n]`,
    example: `import aura::hw\n\ncell main() ->:\n    val cap = hw.open("SPI_CTRL")\n    val _ = hw.write_u32(cap, 0, 0x3FF)`,
    evidence: [
      "The repository contains a dedicated aura-plugin-iot workspace component.",
      "Plugin diagnostics can carry verification-oriented offset/bitmask information where the configured model supports it.",
      "Hardware behavior and trusted native/platform boundaries remain outside a universal proof claim.",
    ],
  },
  {
    id: "aura-ai",
    title: "Aura AI Plugin",
    description: "AI/tensor Nexus integration for shape-aware model and inference checks in supported paths.",
    enableSnippet: `plugins = [\n  { name = "aura-ai", trusted = true },\n]`,
    example: `import aura::tensor\n\ncell main() ->:\n    val model: Model = ai.load_model("model.onnx")\n    val input: Tensor<u32, [2, 2, 3]> = tensor::new<u32>(12)\n    val out = model.infer(input)`,
    evidence: [
      "The repository contains aura-plugin-ai, aura-ai-opt and ONNX Runtime bridge/example assets.",
      "Tensor/model shape information can participate in plugin/verifier logic where the implementation supports it.",
      "This is not a claim that every ONNX operator, model or native runtime path is formally verified.",
    ],
  },
];

function mdxForPlugin(p) {
  const bullets = p.evidence.map((n) => `- ${n}`).join("\n");
  return `---\ntitle: ${p.title}\ndescription: ${p.description}\n---\n\n## Scope\n\n${p.description}\n\n${bullets}\n\n## Enable\n\n\`\`\`toml\n${p.enableSnippet}\n\`\`\`\n\nMarking a plugin as \`trusted\` places its trusted behavior inside the project trust boundary; it does not automatically prove external hardware or native libraries.\n\n## Example\n\n\`\`\`aura\n${p.example}\n\`\`\`\n\n## Editor feedback\n\nWhere verification/plugin diagnostics are produced, \`aura-lsp\` can expose plugin-attributed information to Aura-aware clients such as Sentinel. Exact diagnostics depend on the current plugin implementation and configured project model.\n`;
}

fs.mkdirSync(OUT_DIR, { recursive: true });
const expected = new Set();
for (const plugin of PLUGINS) {
  const name = `${plugin.id}.mdx`;
  expected.add(name);
  fs.writeFileSync(path.join(OUT_DIR, name), mdxForPlugin(plugin), "utf8");
}
for (const name of fs.readdirSync(OUT_DIR)) {
  if (name.endsWith(".mdx") && !expected.has(name)) fs.unlinkSync(path.join(OUT_DIR, name));
}
