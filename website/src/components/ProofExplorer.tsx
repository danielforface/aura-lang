"use client";

import { useState } from "react";

import { Icon } from "./Icon";

const examples = {
  contract: {
    tab: "Contract",
    title: "Make the boundary provable",
    code: [
      ["kw", "cell"], ["plain", " bounded_inc(x: "], ["type", "u32[0..99]"], ["plain", ") -> "], ["type", "u32"], ["plain", ":\n"],
      ["kw", "    requires"], ["plain", " x < 100\n"],
      ["kw", "    ensures"], ["plain", " result <= 100\n\n"],
      ["plain", "    val next: u32 = x + 1\n"],
      ["kw", "    assert"], ["plain", " next <= 100\n"],
      ["plain", "    next"],
    ],
    steps: [
      ["parse", "Source shape accepted"],
      ["sema", "Refinement + contract visible"],
      ["normalize", "Proof obligation produced"],
      ["z3", "Solver-backed verification path"],
    ],
    note: "This panel explains the repository’s proof flow; it is not a browser-hosted compiler session.",
  },
  counterexample: {
    tab: "Counterexample",
    title: "Turn a solver model into source feedback",
    code: [
      ["kw", "cell"], ["plain", " percentage(p: "], ["type", "u32"], ["plain", ") -> "], ["type", "u32[0..100]"], ["plain", ":\n"],
      ["kw", "    requires"], ["plain", " p <= 180\n"],
      ["kw", "    assert"], ["plain", " p <= 100\n"],
      ["plain", "    p"],
    ],
    steps: [
      ["model", "SAT model can be attached"],
      ["map", "Bindings map toward Aura types"],
      ["range", "Source ranges can be included"],
      ["editor", "Sentinel can render structured detail"],
    ],
    note: "The documented aura.counterexample.v2 protocol supports bindings, type metadata, relevance and source-anchored injections when available.",
  },
  trust: {
    tab: "Trust boundary",
    title: "Unsafe code stays visible",
    code: [
      ["kw", "extern cell"], ["plain", " native_read(fd: u32) -> u32\n"],
      ["kw", "trusted extern cell"], ["plain", " audited_clock() -> u32\n\n"],
      ["kw", "unsafe"], ["plain", ":\n"],
      ["plain", "    native_read(fd)"],
    ],
    steps: [
      ["extern", "Foreign implementation is outside Aura"],
      ["unsafe", "Untrusted call requires explicit boundary"],
      ["trusted", "Trusted extern moves code into the TCB"],
      ["claim", "Trust metadata is not an automatic proof"],
    ],
    note: "Aura’s public verification model treats the trusted computing base as something to expose and shrink, not something to pretend has disappeared.",
  },
} as const;

type ExampleKey = keyof typeof examples;

export function ProofExplorer() {
  const [active, setActive] = useState<ExampleKey>("contract");
  const example = examples[active];

  return (
    <div className="proof-explorer">
      <div className="proof-tabs" role="tablist" aria-label="Proof explorer examples">
        {(Object.keys(examples) as ExampleKey[]).map((key) => (
          <button
            key={key}
            type="button"
            className={active === key ? "proof-tab is-active" : "proof-tab"}
            onClick={() => setActive(key)}
          >
            {examples[key].tab}
          </button>
        ))}
      </div>

      <div className="proof-explorer-grid">
        <div className="proof-source">
          <div className="proof-panel-label">Aura source</div>
          <h3>{example.title}</h3>
          <pre className="syntax-code"><code>{example.code.map(([kind, text], index) => <span key={`${kind}-${index}`} className={`tok-${kind}`}>{text}</span>)}</code></pre>
        </div>

        <div className="proof-trace">
          <div className="proof-panel-label">Proof / trust flow</div>
          <div className="trace-list">
            {example.steps.map(([key, text], index) => (
              <div className="trace-row" key={key}>
                <span className="trace-index">{String(index + 1).padStart(2, "0")}</span>
                <span className="trace-line" aria-hidden="true" />
                <div>
                  <strong>{key}</strong>
                  <span>{text}</span>
                </div>
                <Icon name="check" size={17} />
              </div>
            ))}
          </div>
          <p className="proof-note">{example.note}</p>
        </div>
      </div>
    </div>
  );
}
