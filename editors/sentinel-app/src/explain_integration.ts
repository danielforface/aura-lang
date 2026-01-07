/// Sentinel Explain Panel Integration
/// 
/// This module bridges the explainPanel.tsx React component with the main.ts
/// Sentinel IDE, enabling interactive proof failure explanation.
///
/// Integration points:
/// - Diagnostic data (counterexample + variable traces)
/// - Proof failure detection
/// - Panel state management
/// - User interactions (expand, repair suggestions)

import { CounterexamplePayload, VariableTrace, RepairHint, SourceLocation } from "./typed_value";

export interface ExplainPanelState {
  visible: boolean;
  diagnostic: {
    message: string;
    line: number;
    column: number;
    file: string;
  };
  counterexample?: {
    inputs: Record<string, any>;
    outputs: Record<string, any>;
    failedAssertion: string;
    failureTrace: string[];
  };
  variableTraces?: VariableTrace[];
  repairHints?: RepairHint[];
  selectedVariable?: string;
}

export class ExplainPanelIntegration {
  private state: ExplainPanelState;
  private container: HTMLElement | null = null;

  constructor() {
    this.state = {
      visible: false,
      diagnostic: {
        message: "",
        line: 0,
        column: 0,
        file: "",
      },
    };
  }

  /// Initialize panel in DOM
  initialize(containerId: string): void {
    const container = document.getElementById(containerId);
    if (!container) {
      console.error(`Explain panel container not found: ${containerId}`);
      return;
    }
    this.container = container;
    this.render();
  }

  /// Update panel with diagnostic data
  setDiagnostic(payload: {
    message: string;
    line: number;
    column: number;
    file: string;
    data?: any;
  }): void {
    this.state.diagnostic = {
      message: payload.message,
      line: payload.line,
      column: payload.column,
      file: payload.file,
    };

    // Parse counterexample from diagnostic.data
    if (payload.data?.counterexample) {
      const cx = payload.data.counterexample;
      this.state.counterexample = {
        inputs: cx.inputs || {},
        outputs: cx.outputs || {},
        failedAssertion: cx.assertion || "Unknown assertion",
        failureTrace: cx.trace || [],
      };
    }

    // Parse variable traces
    if (payload.data?.variableTraces) {
      this.state.variableTraces = payload.data.variableTraces;
    }

    // Parse repair hints
    if (payload.data?.repairHints) {
      this.state.repairHints = payload.data.repairHints;
    }

    this.show();
    this.render();
  }

  /// Show the explain panel
  show(): void {
    this.state.visible = true;
    this.render();
  }

  /// Hide the explain panel
  hide(): void {
    this.state.visible = false;
    this.render();
  }

  /// Select a variable to inspect
  selectVariable(name: string): void {
    this.state.selectedVariable = name;
    this.render();
  }

  /// Apply a repair hint
  applyRepairHint(hint: RepairHint): void {
    console.log(`Applying repair hint: ${hint.type}`);
    // In full implementation, would emit event to main.ts to apply edit
    // Emits: "explainPanel:applyRepair" with hint details
  }

  /// Navigate to source location
  navigateToLocation(loc: SourceLocation): void {
    console.log(`Navigate to: ${loc.file}:${loc.line}:${loc.column}`);
    // Emits: "explainPanel:navigate" with location
  }

  /// Render panel content
  private render(): void {
    if (!this.container) return;

    if (!this.state.visible) {
      this.container.style.display = "none";
      return;
    }

    this.container.style.display = "block";

    // Build HTML content
    const html = this.buildContent();
    this.container.innerHTML = html;

    // Attach event listeners
    this.attachEventListeners();
  }

  /// Build panel HTML
  private buildContent(): string {
    const { diagnostic, counterexample, variableTraces, repairHints, selectedVariable } =
      this.state;

    const assertionHtml = `
      <div class="explain-assertion">
        <div class="explain-header">❌ Assertion Failed</div>
        <div class="explain-details">
          <code>${escapeHtml(diagnostic.message)}</code>
          <span class="explain-location">${escapeHtml(diagnostic.file)}:${diagnostic.line}:${diagnostic.column}</span>
        </div>
      </div>
    `;

    const counterexampleHtml = counterexample
      ? `
        <div class="explain-counterexample">
          <div class="explain-subheader">Counterexample</div>
          <div class="explain-inputs">
            <details>
              <summary>Inputs</summary>
              ${this.renderValueTree(counterexample.inputs)}
            </details>
          </div>
          <div class="explain-outputs">
            <details>
              <summary>Outputs at Failure</summary>
              ${this.renderValueTree(counterexample.outputs)}
            </details>
          </div>
        </div>
      `
      : "";

    const variableTracesHtml = variableTraces && variableTraces.length > 0
      ? `
        <div class="explain-traces">
          <div class="explain-subheader">Variable Timeline</div>
          ${variableTraces.map((trace) => this.renderVariableTrace(trace, selectedVariable)).join("")}
        </div>
      `
      : "";

    const repairHintsHtml = repairHints && repairHints.length > 0
      ? `
        <div class="explain-repairs">
          <div class="explain-subheader">Suggested Repairs</div>
          ${repairHints.map((hint, i) => this.renderRepairHint(hint, i)).join("")}
        </div>
      `
      : "";

    return `
      ${assertionHtml}
      ${counterexampleHtml}
      ${variableTracesHtml}
      ${repairHintsHtml}
      <div class="explain-footer">
        <button class="explain-btn-close">Close</button>
        <button class="explain-btn-copy">Copy Counterexample</button>
      </div>
    `;
  }

  /// Render variable value as tree
  private renderValueTree(obj: any, depth: number = 0): string {
    if (depth > 5) return "<span class='explain-ellipsis'>...</span>";

    if (obj === null) return "<span class='explain-null'>null</span>";
    if (obj === undefined) return "<span class='explain-undef'>undefined</span>";
    if (typeof obj === "boolean") return `<span class='explain-bool'>${obj}</span>`;
    if (typeof obj === "number") return `<span class='explain-num'>${obj}</span>`;
    if (typeof obj === "string") return `<span class='explain-str'>"${escapeHtml(obj)}"</span>`;

    if (Array.isArray(obj)) {
      if (obj.length === 0) return "<span class='explain-array'>[]</span>";
      return `
        <details class='explain-array'>
          <summary>[${obj.length} items]</summary>
          <div class='explain-array-items'>
            ${obj.map((v, i) => `<div>[${i}]: ${this.renderValueTree(v, depth + 1)}</div>`).join("")}
          </div>
        </details>
      `;
    }

    if (typeof obj === "object") {
      const keys = Object.keys(obj);
      if (keys.length === 0) return "<span class='explain-obj'>{}</span>";
      return `
        <details class='explain-object'>
          <summary>{${keys.length} fields}</summary>
          <div class='explain-object-fields'>
            ${keys
              .map((k) => `<div><span class='explain-key'>${escapeHtml(k)}</span>: ${this.renderValueTree(obj[k], depth + 1)}</div>`)
              .join("")}
          </div>
        </details>
      `;
    }

    return `<span class='explain-unknown'>${typeof obj}</span>`;
  }

  /// Render variable trace timeline
  private renderVariableTrace(trace: VariableTrace, selected?: string): string {
    const isSelected = trace.name === selected;
    return `
      <div class='explain-trace ${isSelected ? "selected" : ""}' data-var='${escapeHtml(trace.name)}'>
        <div class='explain-trace-name'>${escapeHtml(trace.name)}: ${trace.type}</div>
        <div class='explain-trace-timeline'>
          ${trace.assignments
            ?.map(
              (a) => `
            <div class='explain-trace-event' data-line='${a.line}' data-col='${a.column}'>
              <span class='explain-trace-line'>${a.line}:${a.column}</span>
              <span class='explain-trace-value'>${this.renderValueTree(a.value)}</span>
            </div>
          `
            )
            .join("")}
          <div class='explain-trace-final'>
            <span class='explain-trace-label'>→ Final:</span>
            <span class='explain-trace-value'>${this.renderValueTree(trace.final_value)}</span>
          </div>
        </div>
      </div>
    `;
  }

  /// Render repair hint
  private renderRepairHint(hint: RepairHint, index: number): string {
    const types: Record<string, string> = {
      strengthen_precond: "Strengthen precondition",
      weaken_postcond: "Weaken postcondition",
      add_invariant: "Add loop invariant",
      relax_assertion: "Relax assertion",
    };

    const icon: Record<string, string> = {
      strengthen_precond: "🔒",
      weaken_postcond: "📉",
      add_invariant: "🔄",
      relax_assertion: "⚡",
    };

    return `
      <div class='explain-repair'>
        <div class='explain-repair-title'>
          ${icon[hint.type] || "💡"} ${types[hint.type] || hint.type}
        </div>
        <div class='explain-repair-description'>${escapeHtml(hint.description)}</div>
        ${hint.suggested_code ? `<code class='explain-repair-code'>${escapeHtml(hint.suggested_code)}</code>` : ""}
        <button class='explain-repair-apply' data-hint='${index}'>Apply</button>
      </div>
    `;
  }

  /// Attach event listeners
  private attachEventListeners(): void {
    if (!this.container) return;

    // Close button
    const closeBtn = this.container.querySelector(".explain-btn-close");
    if (closeBtn) {
      closeBtn.addEventListener("click", () => this.hide());
    }

    // Copy button
    const copyBtn = this.container.querySelector(".explain-btn-copy");
    if (copyBtn) {
      copyBtn.addEventListener("click", () => {
        const json = JSON.stringify(this.state.counterexample, null, 2);
        navigator.clipboard.writeText(json);
      });
    }

    // Variable selection
    const varTraces = this.container.querySelectorAll(".explain-trace");
    varTraces.forEach((elem) => {
      elem.addEventListener("click", () => {
        const varName = elem.getAttribute("data-var");
        if (varName) this.selectVariable(varName);
      });
    });

    // Repair hint application
    const repairBtns = this.container.querySelectorAll(".explain-repair-apply");
    repairBtns.forEach((btn) => {
      btn.addEventListener("click", () => {
        const idx = parseInt(btn.getAttribute("data-hint") || "0");
        const hint = this.state.repairHints?.[idx];
        if (hint) this.applyRepairHint(hint);
      });
    });

    // Navigation from trace events
    const traceEvents = this.container.querySelectorAll(".explain-trace-event");
    traceEvents.forEach((elem) => {
      elem.addEventListener("click", () => {
        const line = parseInt(elem.getAttribute("data-line") || "0");
        const col = parseInt(elem.getAttribute("data-col") || "0");
        // Emit navigation event
        console.log(`Navigate to line ${line}, col ${col}`);
      });
    });
  }
}

/// Utility: escape HTML
function escapeHtml(text: string): string {
  const map: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#039;",
  };
  return text.replace(/[&<>"']/g, (m) => map[m]);
}

/// Global instance
let explainPanelInstance: ExplainPanelIntegration | null = null;

/// Initialize explain panel globally
export function initializeExplainPanel(): void {
  explainPanelInstance = new ExplainPanelIntegration();
  explainPanelInstance.initialize("explain-panel-container");
}

/// Update explain panel with diagnostic
export function updateExplainPanel(payload: {
  message: string;
  line: number;
  column: number;
  file: string;
  data?: any;
}): void {
  if (!explainPanelInstance) {
    initializeExplainPanel();
  }
  explainPanelInstance?.setDiagnostic(payload);
}

/// Show explain panel
export function showExplainPanel(): void {
  explainPanelInstance?.show();
}

/// Hide explain panel
export function hideExplainPanel(): void {
  explainPanelInstance?.hide();
}
