import Link from "next/link";

import { CodePanel } from "@/components/CodePanel";
import { Icon } from "@/components/Icon";
import { PipelineExplorer } from "@/components/PipelineExplorer";
import { ProofExplorer } from "@/components/ProofExplorer";
import { SectionHeading } from "@/components/SectionHeading";
import { StatusBadge } from "@/components/StatusBadge";
import {
  CLAIMS,
  ECOSYSTEM_SURFACES,
  GITHUB_URL,
  LANGUAGE_FEATURES,
  STATUS,
  TOOLCHAIN_SURFACES,
} from "@/lib/siteData";

const heroCode = `cell transfer(balance: u32[0..10000], amount: u32) -> u32:
    requires amount <= balance
    ensures result <= balance

    val next: u32 = balance - amount
    assert next <= balance
    next`;

const principles = [
  ["01", "Proof belongs in the development loop", "Contracts, proof obligations and counterexamples are designed to return to source-level tooling instead of living in a separate formal-methods silo."],
  ["02", "Fast iteration and native execution stay distinct", "Aura keeps an AVM development path alongside C-oriented and evolving LLVM IR paths rather than hiding backend maturity behind one marketing label."],
  ["03", "Trust is a first-class engineering surface", "FFI, unsafe calls, solver assumptions, backends and external toolchains remain visible parts of the trusted computing base."],
] as const;

export default function HomePage() {
  return (
    <>
      <section className="hero">
        <div className="page-shell hero-grid">
          <div className="hero-copy">
            <div className="eyebrow">Aura · 2026 Edition</div>
            <h1>Write systems code with <span className="accent-word">proof in the loop.</span></h1>
            <p className="hero-lead">
              Aura is a proof-driven systems programming language and developer platform. It connects source semantics,
              Z3-backed verification, development execution, native-oriented backends and editor feedback around one
              language pipeline.
            </p>
            <div className="hero-actions">
              <Link className="button-primary" href="/downloads">Get Aura <Icon name="arrow" size={16} /></Link>
              <Link className="button-secondary" href="/docs/getting-started">Start with Aura</Link>
              <Link className="button-secondary" href="/language">Explore the language</Link>
              <a className="button-text" href={GITHUB_URL} target="_blank" rel="noreferrer">Source on GitHub <Icon name="external" size={14} /></a>
            </div>
            <div className="hero-status">
              <span>{STATUS.stability}</span>
              <span>{STATUS.workspaceMembers} workspace crates</span>
              <span>{STATUS.verification}</span>
              <span>{STATUS.protocolVersion}</span>
            </div>
          </div>

          <div className="hero-visual">
            <CodePanel label="bounded_transfer.aura" meta="source → proof → execution">
              <pre className="hero-code"><code>{heroCode}</code></pre>
              <div className="code-trace">
                <div><strong>parse</strong><span>indentation + cells</span></div>
                <div><strong>sema</strong><span>refinement + contract</span></div>
                <div className="is-proof"><strong>verify</strong><span>proof obligation</span></div>
                <div><strong>execute</strong><span>AVM · C · LLVM*</span></div>
              </div>
            </CodePanel>
          </div>
        </div>
      </section>

      <section className="page-section compact">
        <div className="page-shell">
          <div className="fact-strip">
            <div className="fact"><strong>22</strong><span>Rust workspace members</span></div>
            <div className="fact"><strong>3</strong><span>SMT profiles · fast / ci / thorough</span></div>
            <div className="fact"><strong>v1</strong><span>Aura-specific LSP protocol</span></div>
            <div className="fact"><strong>3</strong><span>Execution surfaces · AVM / C / LLVM*</span></div>
            <div className="fact"><strong>2026</strong><span>Edition namespace · pre-stable</span></div>
          </div>
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Design position"
            title="A language platform, not a compiler demo."
            body="Aura’s repository already spans frontend, semantic analysis, IR, verification, development execution, backends, runtime, packages, FFI, language-server tooling, Sentinel, plugins, release engineering and Android integration. The website treats those as one architecture — while keeping maturity boundaries explicit."
          />
          <div className="principle-grid">
            {principles.map(([index, title, body]) => (
              <article className="principle-card" key={index}>
                <span className="principle-index">{index}</span>
                <h3>{title}</h3>
                <p>{body}</p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Compiler architecture"
            title="One semantic center. Multiple proof and execution surfaces."
            body="The same language pipeline feeds verification, the development VM and native-oriented backends. Backend equivalence is an engineering concern of its own; a source proof is never silently promoted into a claim that every downstream compiler stage is formally correct."
            aside={<Link href="/toolchain" className="button-secondary">Toolchain details <Icon name="arrow" size={15} /></Link>}
          />
          <PipelineExplorer />
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Language surface"
            title="Correctness intent is syntax, not a comment."
            body="Aura’s current implemented/reference-backed surface is intentionally smaller than a stable 1.0 specification. The language already exposes contracts, range refinements, loop reasoning, resource-sensitive rules and explicit foreign-code trust boundaries."
          />
          <div className="language-grid">
            {LANGUAGE_FEATURES.map((feature) => (
              <article className="language-card" key={feature.label}>
                <h3>{feature.label}</h3>
                <span className="language-code">{feature.code}</span>
                <p>{feature.detail}</p>
              </article>
            ))}
          </div>
          <div style={{ marginTop: 24 }}><Link href="/language" className="button-secondary">Read the language overview <Icon name="arrow" size={15} /></Link></div>
        </div>
      </section>

      <section className="page-section dark-section">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Verification"
            title="Proof results become developer feedback."
            body="The verifier is only half the product. Aura’s LSP protocol carries proof phases, structured diagnostics and counterexample metadata back toward the editor so a failing obligation can become something a developer can inspect and repair."
            aside={<Link href="/verification" className="button-secondary">Verification model <Icon name="arrow" size={15} /></Link>}
          />
          <ProofExplorer />
        </div>
      </section>

      <section className="page-section" id="toolchain-home">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Toolchain"
            title="Build, verify, execute, inspect."
            body="Aura exposes a real command surface today. The site distinguishes code-backed commands from experimental or evolving paths instead of presenting roadmap vocabulary as executable fact."
          />
          <div className="toolchain-grid">
            {TOOLCHAIN_SURFACES.map((tool) => (
              <article className="tool-card" key={tool.title}>
                <div className="tool-card-head"><h3>{tool.title}</h3><StatusBadge tone={tool.status === "evolving" ? "evolving" : "proof"}>{tool.status}</StatusBadge></div>
                <span className="tool-command">{tool.command}</span>
                <p>{tool.body}</p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell sentinel-grid">
          <div className="sentinel-copy">
            <div className="eyebrow">Aura Sentinel</div>
            <h3>The proof UI is part of the language experience.</h3>
            <p>Sentinel is a Tauri 2 + CodeMirror desktop environment built around Aura-specific language-server capabilities rather than a generic syntax skin.</p>
            <div className="feature-list">
              <div><span className="feature-icon"><Icon name="proof" size={14} /></span><div><strong>Proof streaming</strong><span>Non-blocking start / phase / done / error / cancelled states.</span></div></div>
              <div><span className="feature-icon"><Icon name="code" size={14} /></span><div><strong>Counterexample detail</strong><span>Structured bindings, type metadata, relevance and source mapping where available.</span></div></div>
              <div><span className="feature-icon"><Icon name="terminal" size={14} /></span><div><strong>Debugger + telemetry infrastructure</strong><span>Development VM and native-debug protocol layers live beside proof UX.</span></div></div>
            </div>
            <div style={{ marginTop: 26 }}><Link className="button-secondary" href="/ecosystem#sentinel">Explore Sentinel</Link></div>
          </div>
          <div className="sentinel-frame" aria-label="Conceptual Aura Sentinel interface">
            <div className="sentinel-titlebar"><span>sentinel · bounded_transfer.aura</span><span>proof stream / illustrative UI</span></div>
            <div className="sentinel-workspace">
              <div className="sentinel-editor">
                <pre>{`cell transfer(balance: u32[0..10000], amount: u32) -> u32:\n    requires amount <= balance\n    ensures result <= balance\n\n    val next: u32 = balance - amount\n`}<span className="line-good">{`    assert next <= balance`}</span>{`\n    next`}</pre>
              </div>
              <div className="sentinel-panel">
                <h4>PROOFS</h4>
                <div className="proof-result"><div className="proof-result-head"><span>requires</span><span>obligation</span></div><p>amount ≤ balance enters the function contract.</p></div>
                <div className="proof-result"><div className="proof-result-head"><span>assert</span><span>mapped</span></div><p>Source-level assertion remains linked to verification feedback.</p></div>
                <div className="proof-result"><div className="proof-result-head"><span>protocol</span><span>v1</span></div><p>Aura-specific extensions are versioned separately from ordinary LSP compatibility.</p></div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell">
          <SectionHeading
            eyebrow="Ecosystem"
            title="A language needs more than syntax."
            body="Packages, FFI, plugins, an SDK, an IDE, application UI work, Android tooling and deterministic release engineering already exist in the monorepo. They are presented as integration surfaces with their real maturity — not as blanket stability guarantees."
          />
          <div className="ecosystem-grid">
            {ECOSYSTEM_SURFACES.map((surface) => (
              <Link className="ecosystem-card" href={surface.href} key={surface.title}>
                <span className="card-eyebrow">{surface.eyebrow}</span>
                <h3>{surface.title}</h3>
                <p>{surface.body}</p>
                <span className="card-arrow"><Icon name="arrow" size={17} /></span>
              </Link>
            ))}
          </div>
        </div>
      </section>

      <section className="page-section">
        <div className="page-shell">
          <SectionHeading eyebrow="Evidence discipline" title="Impressive claims are useful only when they stay true." body="Aura is strongest when the website states exactly what the repository establishes. The public status layer deliberately separates implementation, feature gates, evolving backends, targets and claims that are not yet justified." />
          <div className="claims-grid">
            <div className="claim-panel is-supported">
              <div className="claim-panel-head"><Icon name="check" size={18} /><strong>Supported public claims</strong></div>
              <ul className="claim-list">{CLAIMS.supported.map((claim) => <li key={claim}>{claim}</li>)}</ul>
            </div>
            <div className="claim-panel is-boundary">
              <div className="claim-panel-head"><Icon name="shield" size={18} /><strong>Boundaries we keep explicit</strong></div>
              <ul className="claim-list">{CLAIMS.notClaimed.map((claim) => <li key={claim}>{claim}</li>)}</ul>
            </div>
          </div>
        </div>
      </section>

      <section className="page-section compact">
        <div className="page-shell">
          <div className="closing-panel">
            <div><h2>Read the language. Inspect the proof model. Then inspect the source.</h2><p>Aura is public as a serious pre-stable language platform. The website is an orientation layer; the repository remains the implementation source of truth.</p></div>
            <div className="hero-actions"><Link className="button-secondary" href="/docs/getting-started">Documentation</Link><a className="button-secondary" href={GITHUB_URL} target="_blank" rel="noreferrer">GitHub <Icon name="external" size={14} /></a></div>
          </div>
        </div>
      </section>
    </>
  );
}
