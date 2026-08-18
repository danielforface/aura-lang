"use client";

import { useState } from "react";

import { PIPELINE } from "@/lib/siteData";

export function PipelineExplorer() {
  const [active, setActive] = useState("proof");
  const selected = PIPELINE.find((item) => item.id === active) ?? PIPELINE[0];

  return (
    <div className="pipeline-explorer">
      <div className="pipeline-track" role="tablist" aria-label="Aura compiler pipeline">
        {PIPELINE.map((item, index) => (
          <button
            key={item.id}
            type="button"
            className={item.id === active ? "pipeline-node is-active" : "pipeline-node"}
            onClick={() => setActive(item.id)}
          >
            <span className="pipeline-number">{String(index + 1).padStart(2, "0")}</span>
            <strong>{item.label}</strong>
            <span>{item.crate}</span>
          </button>
        ))}
      </div>
      <div className="pipeline-detail">
        <div className="eyebrow">{selected.label}</div>
        <h3>{selected.crate}</h3>
        <p>{selected.detail}</p>
        <div className="pipeline-rule">
          <span />
          <span>{selected.id === "proof" ? "verification and execution stay adjacent — not conflated" : "shared semantics, explicit boundaries"}</span>
        </div>
      </div>
    </div>
  );
}
