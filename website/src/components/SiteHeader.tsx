"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState } from "react";

import { BrandMark } from "./BrandMark";
import { Icon } from "./Icon";
import { GITHUB_URL } from "@/lib/siteData";

const nav = [
  { href: "/language", label: "Language" },
  { href: "/verification", label: "Verification" },
  { href: "/toolchain", label: "Toolchain" },
  { href: "/ecosystem", label: "Ecosystem" },
  { href: "/downloads", label: "Downloads" },
  { href: "/docs/getting-started", label: "Docs" },
];

export function SiteHeader() {
  const [open, setOpen] = useState(false);
  const pathname = usePathname();

  return (
    <header className="site-header">
      <div className="site-header-inner">
        <BrandMark />

        <nav className="desktop-nav" aria-label="Primary navigation">
          {nav.map((item) => {
            const active = item.href.startsWith("/docs")
              ? pathname.startsWith("/docs")
              : pathname === item.href || pathname.startsWith(`${item.href}/`);
            return (
              <Link key={item.href} href={item.href} className={active ? "nav-link is-active" : "nav-link"}>
                {item.label}
              </Link>
            );
          })}
        </nav>

        <div className="header-actions">
          <Link href="/status" className="status-link">
            <span className="status-dot" />
            Pre-stable
          </Link>
          <a className="icon-button desktop-only" href={GITHUB_URL} target="_blank" rel="noreferrer" aria-label="Aura on GitHub">
            <Icon name="github" size={19} />
          </a>
          <button className="icon-button mobile-menu-button" type="button" onClick={() => setOpen((value) => !value)} aria-expanded={open} aria-label="Toggle navigation">
            <Icon name={open ? "x" : "menu"} size={20} />
          </button>
        </div>
      </div>

      {open ? (
        <div className="mobile-nav-wrap">
          <nav className="mobile-nav" aria-label="Mobile navigation">
            {[...nav, { href: "/status", label: "Project status" }].map((item) => (
              <Link key={item.href} href={item.href} onClick={() => setOpen(false)}>
                {item.label}
                <Icon name="arrow" size={17} />
              </Link>
            ))}
            <a href={GITHUB_URL} target="_blank" rel="noreferrer" onClick={() => setOpen(false)}>
              GitHub
              <Icon name="external" size={17} />
            </a>
          </nav>
        </div>
      ) : null}
    </header>
  );
}
