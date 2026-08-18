import Link from "next/link";
import { ProofExplorer } from "@/components/ProofExplorer";
import { SectionHeading } from "@/components/SectionHeading";
import { StatusBadge } from "@/components/StatusBadge";

export const metadata = { title: "Verification", description: "Aura contracts, Z3-backed proof flow, structured counterexamples and trusted-computing-base model." };

const stages = [
  ["01", "Contract / assertion", "Source intent becomes an explicit proof obligation."],
  ["02", "Normalize", "Semantic state is lowered into verifier-consumable constraints."],
  ["03", "Solver", "Feature-gated Z3 runs under fast, ci or thorough profiles."],
  ["04", "Result", "Proof, counterexample, timeout or unknown remain distinct outcomes."],
  ["05", "Map", "Counterexample data can be mapped toward Aura types and source ranges."],
  ["06", "Editor", "aura-lsp streams phases and structured diagnostics to Sentinel/editor clients."],
] as const;

export default function VerificationPage(){return <>
  <section className="subpage-hero"><div className="page-shell subpage-hero-grid"><div><div className="eyebrow">Verification</div><h1>Formal reasoning as developer feedback.</h1><p>Aura integrates contracts and Z3-backed verification into the ordinary toolchain. The differentiator is not merely “there is an SMT solver”; it is the path from source obligation to structured, source-oriented feedback.</p></div><aside className="subpage-summary"><dl><div><dt>Solver path</dt><dd>Z3 · feature-gated</dd></div><div><dt>Profiles</dt><dd>fast / ci / thorough</dd></div><div><dt>Counterexample schema</dt><dd>v2</dd></div><div><dt>LSP extension</dt><dd>Aura protocol v1</dd></div></dl></aside></div></section>

  <section className="page-section dark-section"><div className="page-shell"><SectionHeading eyebrow="Proof explorer" title="See how source intent travels through the verification model." body="This is a precise explanatory visualization of the repository’s proof architecture — not a simulated claim that the browser is executing Z3."/><ProofExplorer/></div></section>

  <section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Lifecycle" title="A proof is a pipeline with failure semantics." body="Timeout and solver unknown are not proofs. Counterexamples are not just red badges. Each state needs to survive the trip back to the developer without being flattened into marketing-friendly success/failure."/><div className="numbered-list">{stages.map(([n,t,b])=><div className="numbered-row" key={n}><span>{n}</span><h3>{t}</h3><p>{b}</p></div>)}</div></div></section>

  <section className="page-section"><div className="page-shell"><div className="three-col"><article className="content-card"><div className="tool-card-head"><h3>Interactive profile</h3><StatusBadge tone="info">fast</StatusBadge></div><p>Low-latency-oriented solver profile for the edit/verify loop.</p></article><article className="content-card"><div className="tool-card-head"><h3>CI profile</h3><StatusBadge tone="proof">ci</StatusBadge></div><p>CI-oriented defaults intended to balance depth with repeatable automation.</p></article><article className="content-card"><div className="tool-card-head"><h3>Deep profile</h3><StatusBadge tone="evolving">thorough</StatusBadge></div><p>Deeper verification mode; the repository’s verification example notes quantifier acceptance under thorough.</p></article></div><pre className="command-block"><code>{`cargo run -p aura --features z3 -- verify main.aura --smt-profile fast\n\n# optional warm solver state inside a run\nAURA_Z3_INCREMENTAL=1 cargo run -p aura --features z3 -- verify main.aura`}</code></pre></div></section>

  <section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Counterexamples" title="“SAT” is not a useful developer message." body="The documented aura.counterexample.v2 shape can carry bindings, values, value kinds, Aura type information, relevance, source ranges and source-anchored injections when available."/><div className="two-col"><article className="content-card"><h3>Solver view</h3><pre className="command-block"><code>{`sat\n(model\n  (define-fun p () Int 180)\n)`}</code></pre></article><article className="content-card"><h3>Developer-facing direction</h3><pre className="command-block"><code>{`assert p <= 100\n       ^^^^^^^^\n\np: u32 = 180\nrelevant: true\nsource: this assertion`}</code></pre></article></div></div></section>

  <section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Trusted core" title="Verification shrinks uncertainty. It does not erase trust."/><div className="claims-grid"><article className="claim-panel is-supported"><div className="claim-panel-head"><strong>Inside the proof story</strong></div><ul className="claim-list"><li>source contracts and assertions</li><li>proof obligations and modeled semantics</li><li>structured proof/counterexample results</li><li>trusted-core reports and audits</li></ul></article><article className="claim-panel is-boundary"><div className="claim-panel-head"><strong>Still trusted / independently validated</strong></div><ul className="claim-list"><li>solver implementation and invocation assumptions</li><li>compiler/backend correctness not formally established end-to-end</li><li>runtime, OS and hardware</li><li>explicitly trusted FFI and external toolchains</li></ul></article></div></div></section>

  <section className="page-section compact"><div className="page-shell"><div className="closing-panel"><div><h2>Proof-driven does not mean proof-washed.</h2><p>Aura’s website deliberately refuses “formally verified compiler” and universal latency claims that the repository does not establish.</p></div><Link href="/status" className="button-secondary">Read project status</Link></div></div></section>
</>}
