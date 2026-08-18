import { SectionHeading } from "@/components/SectionHeading";
import { StatusBadge } from "@/components/StatusBadge";
import { CLAIMS, STATUS } from "@/lib/siteData";

export const metadata = { title: "Project status", description: "Implementation-backed status and public claim boundaries for the active pre-stable Aura language platform." };

const rows = [
 ["Language frontend", "Code-backed", "lexer, parser, AST, semantic core, Aura IR", "proof"],
 ["Z3 verification", "Feature-gated", "verify path, solver profiles, counterexample mapping", "info"],
 ["AVM", "Code-backed", "development VM / interpreter", "proof"],
 ["C backend", "Code-backed", "default C-oriented backend surface", "proof"],
 ["LLVM IR backend", "Evolving", "implemented + feature-gated; not finished optimizer", "evolving"],
 ["Hybrid selector", "Code-backed", "automatic AVM→LLVM promotion not implemented", "warning"],
 ["Package manager", "Code-backed", "resolver/cache/lockfile/security/signing", "proof"],
 ["Sentinel + LSP", "Code-backed", "proof protocol, diagnostics, IDE integrations", "proof"],
 ["Lumina", "Evolving", "substantial UI/application plugin", "evolving"],
 ["Android", "Integrated path", "SDK/NDK, runtime cross-build, sample APK tooling", "info"],
] as const;

export default function StatusPage(){return <>
<section className="subpage-hero"><div className="page-shell subpage-hero-grid"><div><div className="eyebrow">Project status</div><h1>Implementation state is not the same thing as release maturity.</h1><p>Aura is an active, pre-stable language platform. The repository contains several version namespaces and historical milestone labels, so the public site reconciles workspace version, edition, component versions and roadmap status rather than collapsing them into one misleading number.</p></div><aside className="subpage-summary"><dl><div><dt>Public label</dt><dd>Aura 2026 Edition</dd></div><div><dt>Workspace</dt><dd>{STATUS.workspaceVersion}</dd></div><div><dt>Stability</dt><dd>active · pre-stable</dd></div><div><dt>Workspace members</dt><dd>{STATUS.workspaceMembers}</dd></div></dl></aside></div></section>
<section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Version reconciliation" title="Four different things can all carry a version."/><div className="four-col three-col"><article className="content-card"><h3>Workspace</h3><p>`0.2.0` is the primary Rust workspace/compiler package version.</p></article><article className="content-card"><h3>Edition</h3><p>`2026` is a language syntax/feature compatibility namespace.</p></article><article className="content-card"><h3>Components</h3><p>Individual tools can carry independent versions such as `aura-pkg 1.0.0`.</p></article><article className="content-card"><h3>Milestones</h3><p>Historical “v0.3” / “v1.0” documents describe planning or subsystem progress, not one stable GA release.</p></article></div></div></section>
<section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Capability matrix" title="What exists, what evolves, what is gated."/><div className="status-matrix"><div className="status-matrix-row is-head"><strong>Surface</strong><strong>Status</strong><strong>Meaning</strong></div>{rows.map(([surface,status,meaning,tone])=><div className="status-matrix-row" key={surface}><strong>{surface}</strong><span><StatusBadge tone={tone}>{status}</StatusBadge></span><span>{meaning}</span></div>)}</div></div></section>
<section className="page-section"><div className="page-shell"><SectionHeading eyebrow="Public claims" title="Precision is part of the product."/><div className="claims-grid"><div className="claim-panel is-supported"><div className="claim-panel-head"><strong>Supported</strong></div><ul className="claim-list">{CLAIMS.supported.map(x=><li key={x}>{x}</li>)}</ul></div><div className="claim-panel is-boundary"><div className="claim-panel-head"><strong>Not claimed today</strong></div><ul className="claim-list">{CLAIMS.notClaimed.map(x=><li key={x}>{x}</li>)}</ul></div></div></div></section>
<section className="page-section"><div className="page-shell"><div className="content-card"><div className="eyebrow">Performance</div><h3>&lt;200ms is an engineering target until a benchmark artifact says otherwise.</h3><p>The repository contains caching, profiling and tuning infrastructure, and historical planning documents define interactive proof-latency ambitions. The website does not convert those implementation efforts into universal measured performance. A headline benchmark should include commit, hardware/software environment, workload, cache state, solver version/config, sample count and percentile methodology.</p></div></div></section>
</>}
