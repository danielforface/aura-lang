import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

function workspaceRoot() {
  // Resolve relative to this script so it works regardless of cwd.
  // website/scripts -> website -> repo root
  const here = path.dirname(fileURLToPath(import.meta.url));
  return path.resolve(here, "..", "..");
}

function uniqueSorted(xs) {
  return Array.from(new Set(xs)).sort();
}

async function main() {
  const root = workspaceRoot();
  const websiteDir = path.join(root, "website");
  const lexerPath = path.join(root, "aura-lex", "src", "lexer.rs");
  const outPath = path.join(websiteDir, "src", "generated", "aura-grammar.json");

  const src = await fs.readFile(lexerPath, "utf8");

  // Extract #[token("...")] literals from RawToken.
  const tokenRe = /#\[token\("([^"]+)"\)\]\s*\n\s*([A-Za-z0-9_]+)/g;

  const keywords = [];
  const operators = [];

  let m;
  while ((m = tokenRe.exec(src)) !== null) {
    const lexeme = m[1];
    const variant = m[2];

    // Heuristic: treat Kw* as keywords; everything else as operator/punctuation.
    if (variant.startsWith("Kw")) {
      keywords.push(lexeme);
    } else {
      operators.push(lexeme);
    }
  }

  const payload = {
    source: "aura-lex/src/lexer.rs",
    extractedAt: new Date().toISOString(),
    keywords: uniqueSorted(keywords),
    operators: uniqueSorted(operators),
  };

  await fs.mkdir(path.dirname(outPath), { recursive: true });
  await fs.writeFile(outPath, JSON.stringify(payload, null, 2) + "\n", "utf8");
  console.log(`Wrote ${path.relative(process.cwd(), outPath)} from ${path.relative(process.cwd(), lexerPath)}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
