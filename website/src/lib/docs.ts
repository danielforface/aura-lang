import fs from "node:fs/promises";
import path from "node:path";

import matter from "gray-matter";
import { compileMDX } from "next-mdx-remote/rsc";

import { mdxComponents } from "@/components/mdx";

export type DocFrontmatter = {
  title: string;
  description?: string;
};

function docsRoot() {
  return path.join(process.cwd(), "content", "docs");
}

function resolveDocPath(slug: string) {
  // Normalize and prevent path traversal.
  const normalized = slug.replace(/\\/g, "/").replace(/^\/+/, "");
  if (!normalized || normalized.includes("..")) {
    throw new Error("invalid doc slug");
  }

  const root = docsRoot();
  const filePath = path.join(root, `${normalized}.mdx`);
  const resolved = path.resolve(filePath);
  const resolvedRoot = path.resolve(root);
  if (!resolved.startsWith(resolvedRoot + path.sep) && resolved !== resolvedRoot) {
    throw new Error("invalid doc path");
  }
  return resolved;
}

export async function getDoc(slug: string) {
  const filePath = resolveDocPath(slug);
  const raw = await fs.readFile(filePath, "utf8");
  const { content, data } = matter(raw);

  const mdx = await compileMDX<DocFrontmatter>({
    source: content,
    options: { parseFrontmatter: false },
    components: mdxComponents,
  });

  const frontmatter: DocFrontmatter = {
    title: (data.title as string) ?? slug,
    description: data.description as string | undefined,
  };

  return { frontmatter, content: mdx.content };
}
