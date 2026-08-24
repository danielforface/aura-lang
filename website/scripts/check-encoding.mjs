import fs from "node:fs";
import path from "node:path";

const BAD_PATTERNS = [
  { name: "Mojibake â€ (quotes/dashes)", regex: /\u00E2\u20AC/ },
  { name: "Mojibake â† (arrows)", regex: /\u00E2\u2020/ },
  { name: "Mojibake â” (box drawing)", regex: /\u00E2\u201D/ },
  { name: "Mojibake â‰ (math symbols)", regex: /\u00E2\u2030/ },
  { name: "Mojibake â€“ (en-dash)", regex: /\u00E2\u2013/ },
  { name: "UTF-8 BOM artifact ï»¿", regex: /\u00EF\u00BB\u00BF/ },
  { name: "Unicode Replacement Character \uFFFD", regex: /\uFFFD/ },
];

const TARGET_DIRS = ["content", "src"];
const TEXT_EXTENSIONS = new Set([".mdx", ".md", ".ts", ".tsx", ".json", ".mjs", ".js", ".css"]);

let errors = 0;

function scanDir(dir) {
  if (!fs.existsSync(dir)) return;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "node_modules" && entry.name !== ".next" && entry.name !== "out") {
        scanDir(fullPath);
      }
    } else if (entry.isFile()) {
      const ext = path.extname(entry.name).toLowerCase();
      if (TEXT_EXTENSIONS.has(ext)) {
        checkFile(fullPath);
      }
    }
  }
}

function checkFile(filePath) {
  const content = fs.readFileSync(filePath, "utf-8");
  const lines = content.split("\n");
  lines.forEach((line, index) => {
    for (const pattern of BAD_PATTERNS) {
      if (pattern.regex.test(line)) {
        console.error(`[ENCODING ERROR] ${filePath}:${index + 1} - Found ${pattern.name}`);
        console.error(`  Line: ${line.trim()}`);
        errors++;
      }
    }
  });
}

for (const dir of TARGET_DIRS) {
  scanDir(path.resolve(process.cwd(), dir));
}

if (errors > 0) {
  console.error(`\nFound ${errors} encoding error(s). Please fix corrupted characters.`);
  process.exit(1);
} else {
  console.log("PASS: All website content and source files have clean UTF-8 encoding.");
}