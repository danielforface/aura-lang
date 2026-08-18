import Link from "next/link";

import { AuraGlyph } from "./BrandMark";
import { Icon } from "./Icon";
import { GITHUB_URL } from "@/lib/siteData";

const columns = [
  {
    title: "Language",
    links: [
      ["/language", "Language overview"],
      ["/verification", "Verification model"],
      ["/toolchain", "Compiler & toolchain"],
      ["/status", "Project status"],
    ],
  },
  {
    title: "Develop",
    links: [
      ["/docs/getting-started", "Getting started"],
      ["/playground", "Language explorer"],
      ["/downloads", "Downloads"],
      ["/gallery", "Examples"],
    ],
  },
  {
    title: "Platform",
    links: [
      ["/ecosystem#sentinel", "Aura Sentinel"],
      ["/ecosystem#packages", "Packages"],
      ["/ecosystem#lumina", "Lumina"],
      ["/ecosystem#android", "Android"],
    ],
  },
] as const;

export function SiteFooter() {
  return (
    <footer className="site-footer">
      <div className="footer-grid">
        <div className="footer-brand">
          <AuraGlyph className="footer-glyph" />
          <div>
            <strong>Aura</strong>
            <p>Proof-driven systems programming language and developer platform.</p>
          </div>
        </div>
        {columns.map((column) => (
          <div className="footer-column" key={column.title}>
            <div className="footer-label">{column.title}</div>
            {column.links.map(([href, label]) => (
              <Link key={href} href={href}>{label}</Link>
            ))}
          </div>
        ))}
        <div className="footer-column">
          <div className="footer-label">Source</div>
          <a href={GITHUB_URL} target="_blank" rel="noreferrer" className="footer-external">
            GitHub <Icon name="external" size={14} />
          </a>
          <a href={`${GITHUB_URL}/blob/main/LICENSE`} target="_blank" rel="noreferrer">MIT License</a>
          <a href={`${GITHUB_URL}/blob/main/SECURITY.md`} target="_blank" rel="noreferrer">Security</a>
        </div>
      </div>
      <div className="footer-bottom">
        <span>Aura 2026 Edition</span>
        <span>Active · pre-stable</span>
        <span>Claims follow evidence.</span>
      </div>
    </footer>
  );
}
