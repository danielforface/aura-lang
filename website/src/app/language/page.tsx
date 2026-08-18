import Link from "next/link";
import { Icon } from "@/components/Icon";
import { SectionHeading } from "@/components/SectionHeading";
import { LANGUAGE_FEATURES } from "@/lib/siteData";

export const metadata = { title: "Language", description: "Current Aura syntax, contracts, refinements, resource semantics and explicit FFI trust boundaries." };

const constructs = [
  ["Bindings", "val · val mut", "Immutable and mutable value bindings with optional/explicit type information where supported."],
  ["Cells", "cell · extern cell · trusted extern cell", "Functions plus explicit foreign-code boundaries and deliberate trust annotations."],
  ["Control flow", "if · match · while", "Core control-flow forms with proof-oriented loop annotations."],
  ["Current built-ins", "u32 · bool · String · Unit · Tensor · Model · Style", "A current snapshot, not a frozen 1.0 type-system promise."],
  ["Refinements", "u32[0..100]", "Range constraints are available as verifier-visible type information."],
  ["UI syntax", "layout: · render:", "Current reference-backed application syntax used by the evolving Lumina layer."],
] as const;

export default function LanguagePage() {
  return <>
    <section className="subpage-hero"><div className="page-shell subpage-hero-grid"><div><div className="eyebrow">Language</div><h1>Systems syntax designed to carry correctness intent.</h1><p>Aura’s current language surface combines significant indentation, `cell` declarations, contracts, refinements, explicit trust boundaries and resource-sensitive rules. This page describes what is implemented/reference-backed today — not a hypothetical frozen 1.0 grammar.</p></div><aside className="subpage-summary"><dl><div><dt>Edition namespace</dt><dd>2026</dd></div><div><dt>Stability</dt><dd>pre-stable</dd></div><div><dt>Reference precedence</dt><dd>implementation first</dd></div><div><dt>Indentation</dt><dd>significant</dd></div></dl></aside></div></section>

    <section className="page-section compact"><div className="page-shell"><div className="two-col"><article className="content-card"><div className="eyebrow">Small example</div><h3>Contracts live beside code.</h3><pre className="syntax-code" style={{background:"var(--code)",padding:22,borderRadius:14,marginTop:20}}><code>{`cell bounded_inc(x: u32[0..99]) -> u32:\n    requires x < 100\n    ensures result <= 100\n\n    val next: u32 = x + 1\n    assert next <= 100\n    next`}</code></pre></article><article className="content-card"><div className="eyebrow">Precedence</div><h3>When documents disagree, code wins.</h3><p>Aura already has historical design and milestone documents with syntax that can run ahead of the compiler. Public language documentation therefore follows a strict order: current implementation → compact SDK reference → focused current verifier/protocol docs → historical design material.</p><Link href="/docs/language-reference" className="button-text">Normative snapshot <Icon name="arrow" size={14}/></Link></article></div></div></section>

    <section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Current surface" title="The language you can reason about today." body="The compact public surface is intentionally conservative. That is a feature for a pre-stable language: examples should teach syntax the repository can defend."/><div className="three-col">{constructs.map(([title,code,body])=><article className="content-card" key={title}><span className="language-code">{code}</span><h3>{title}</h3><p>{body}</p></article>)}</div></div></section>

    <section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Proof vocabulary" title="Correctness constructs stay close to the statement they constrain."/><div className="language-grid">{LANGUAGE_FEATURES.map(f=><article className="language-card" key={f.label}><h3>{f.label}</h3><span className="language-code">{f.code}</span><p>{f.detail}</p></article>)}</div></div></section>

    <section className="page-section"><div className="page-shell"><div className="two-col"><article className="content-card"><div className="eyebrow">Resources</div><h3>Move-oriented rules exist without pretending Aura is Rust.</h3><p>The current reference describes move behavior for resource-like values such as Tensor, Model and Style. The codebase also contains broader ownership, move-tracking, capability and linear-type infrastructure. Until those converge into a stable normative spec, the website separates implemented analysis from stable language guarantees.</p></article><article className="content-card"><div className="eyebrow">Foreign code</div><h3>Trust must be visible.</h3><pre className="command-block"><code>{`extern cell native_read(fd: u32) -> u32\ntrusted extern cell audited_clock() -> u32\n\nunsafe:\n    native_read(fd)`}</code></pre><p>An untrusted foreign call requires an explicit `unsafe:` boundary in the current model. A trusted extern means the project accepts that implementation into its trusted base — it does not mean Aura proved it.</p></article></div></div></section>

    <section className="page-section compact"><div className="page-shell"><div className="closing-panel"><div><h2>The language guide is conservative on purpose.</h2><p>Pre-stable languages earn trust by distinguishing what exists from what is merely designed.</p></div><Link className="button-secondary" href="/docs/language-reference">Read the reference</Link></div></div></section>
  </>;
}
