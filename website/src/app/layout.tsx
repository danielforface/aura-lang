import type { Metadata, Viewport } from "next";

import "./globals.css";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/SiteFooter";

export const metadata: Metadata = {
  metadataBase: new URL("https://aura.geniuses.team"),
  title: {
    default: "Aura — Proof-driven systems programming",
    template: "%s · Aura",
  },
  description:
    "Aura is a proof-driven systems programming language and developer platform with Z3-backed verification, an AVM, native-oriented backends, Aura Sentinel, package tooling, plugins and Android integration.",
  keywords: [
    "Aura programming language",
    "systems programming",
    "program verification",
    "Z3",
    "formal methods",
    "compiler",
    "Rust",
    "LLVM",
    "language server",
    "refinement types",
  ],
  authors: [{ name: "Daniel Cohen" }],
  creator: "Daniel Cohen",
  publisher: "Aura Project",
  category: "technology",
  alternates: { canonical: "/" },
  openGraph: {
    title: "Aura — Proof-driven systems programming",
    description: "Contracts, proofs, execution and developer feedback in one language platform.",
    url: "https://aura.geniuses.team/",
    siteName: "Aura",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Aura — Proof-driven systems programming",
    description: "Contracts, proofs, execution and developer feedback in one language platform.",
  },
  robots: { index: true, follow: true },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#f5f3ee",
  colorScheme: "light",
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <SiteHeader />
        <main className="site-main">{children}</main>
        <SiteFooter />
      </body>
    </html>
  );
}
