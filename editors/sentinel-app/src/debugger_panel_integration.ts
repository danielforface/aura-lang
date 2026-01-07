/// Sentinel Debugger Panel Integration
/// 
/// This module bridges debugger_integration.ts (MI protocol + AuraDebugger)
/// with the Sentinel IDE main panel, enabling interactive debugging with:
/// - Breakpoint management
/// - Step/continue/next/finish controls
/// - Stack frame inspection
/// - Variable watching
/// - Thread management

import type { AuraDebugger, DebuggerSession } from "./debugger_integration";

export interface DebuggerUIState {
  initialized: boolean;
  running: boolean;
  paused: boolean;
  currentFile?: string;
  currentLine?: number;
  currentColumn?: number;
  stopReason?: string;
  stackFrames: StackFrame[];
  variables: DebugVariable[];
  watches: WatchExpr[];
  threads: DebugThread[];
  breakpoints: DebugBreakpoint[];
}

export interface StackFrame {
  id: number;
  name: string;
  file: string;
  line: number;
  column: number;
  address: string;
}

export interface DebugVariable {
  name: string;
  type: string;
  value: string;
  variablesReference: number;
  children?: DebugVariable[];
}

export interface WatchExpr {
  id: string;
  expression: string;
  value?: string;
  error?: string;
}

export interface DebugThread {
  id: number;
  name: string;
  state: "running" | "stopped" | "waiting";
}

export interface DebugBreakpoint {
  id: number;
  verified: boolean;
  file: string;
  line: number;
  condition?: string;
  hitCount?: number;
}

export class DebuggerPanelIntegration {
  private debugger: AuraDebugger | null = null;
  private state: DebuggerUIState;
  private container: HTMLElement | null = null;
  private eventListeners: Map<string, Set<Function>> = new Map();

  constructor() {
    this.state = {
      initialized: false,
      running: false,
      paused: false,
      stackFrames: [],
      variables: [],
      watches: [],
      threads: [],
      breakpoints: [],
    };
  }

  /// Initialize debugger panel
  initialize(containerId: string, debuggerInstance: AuraDebugger): void {
    const container = document.getElementById(containerId);
    if (!container) {
      console.error(`Debugger panel container not found: ${containerId}`);
      return;
    }

    this.container = container;
    this.debugger = debuggerInstance;
    this.state.initialized = true;

    // Set up event listeners on debugger
    this.setupDebuggerListeners();

    this.render();
  }

  /// Set up listeners for debugger events
  private setupDebuggerListeners(): void {
    if (!this.debugger) return;

    // Listen to debugger events
    this.debugger.on("running", () => {
      this.state.running = true;
      this.state.paused = false;
      this.render();
    });

    this.debugger.on("stopped", (event: any) => {
      this.state.running = false;
      this.state.paused = true;
      this.state.currentFile = event.file;
      this.state.currentLine = event.line;
      this.state.currentColumn = event.column;
      this.state.stopReason = event.reason;
      this.updateStackAndVariables();
      this.render();
    });

    this.debugger.on("breakpoint-set", (event: any) => {
      this.state.breakpoints.push({
        id: event.id,
        verified: true,
        file: event.file,
        line: event.line,
        condition: event.condition,
      });
      this.render();
    });

    this.debugger.on("breakpoint-removed", (event: any) => {
      this.state.breakpoints = this.state.breakpoints.filter((bp) => bp.id !== event.id);
      this.render();
    });
  }

  /// Update stack frames and variables from debugger
  private async updateStackAndVariables(): Promise<void> {
    if (!this.debugger) return;

    try {
      const frames = await this.debugger.getStack();
      this.state.stackFrames = frames.map((f, i) => ({
        id: i,
        name: f.func || "<unknown>",
        file: f.file || "<unknown>",
        line: f.line || 0,
        column: f.col || 0,
        address: f.addr || "0x0",
      }));

      const vars = await this.debugger.getVariables(0);
      this.state.variables = vars.map((v) => ({
        name: v.name,
        type: v.type || "unknown",
        value: v.value || "<unknown>",
        variablesReference: 0,
      }));
    } catch (e) {
      console.error("Failed to update stack/variables:", e);
    }
  }

  /// Launch debugger
  async launch(targetPath: string, args?: string[]): Promise<void> {
    if (!this.debugger) return;

    try {
      await this.debugger.init();
      await this.debugger.run(targetPath, args);
      this.state.running = true;
      this.render();
    } catch (e) {
      console.error("Failed to launch debugger:", e);
      this.addError(`Launch failed: ${e}`);
    }
  }

  /// Step over next line
  async stepOver(): Promise<void> {
    if (!this.debugger || !this.state.paused) return;
    try {
      await this.debugger.next();
    } catch (e) {
      console.error("Step over failed:", e);
    }
  }

  /// Step into function
  async stepInto(): Promise<void> {
    if (!this.debugger || !this.state.paused) return;
    try {
      await this.debugger.step();
    } catch (e) {
      console.error("Step into failed:", e);
    }
  }

  /// Step out of function
  async stepOut(): Promise<void> {
    if (!this.debugger || !this.state.paused) return;
    try {
      await this.debugger.stepOut();
    } catch (e) {
      console.error("Step out failed:", e);
    }
  }

  /// Continue execution
  async continue(): Promise<void> {
    if (!this.debugger || !this.state.paused) return;
    try {
      await this.debugger.continue();
      this.state.running = true;
      this.state.paused = false;
      this.render();
    } catch (e) {
      console.error("Continue failed:", e);
    }
  }

  /// Pause execution
  async pause(): Promise<void> {
    if (!this.debugger || !this.state.running) return;
    try {
      // Most debuggers pause on signal
      console.log("Pause requested (implementation depends on debugger)");
    } catch (e) {
      console.error("Pause failed:", e);
    }
  }

  /// Terminate debugger
  async terminate(): Promise<void> {
    if (!this.debugger) return;
    try {
      await this.debugger.terminate();
      this.state.running = false;
      this.state.paused = false;
      this.state.stackFrames = [];
      this.state.variables = [];
      this.render();
    } catch (e) {
      console.error("Terminate failed:", e);
    }
  }

  /// Set breakpoint
  async setBreakpoint(file: string, line: number, condition?: string): Promise<void> {
    if (!this.debugger) return;
    try {
      await this.debugger.setBreakpoint(file, line, condition);
    } catch (e) {
      console.error("Set breakpoint failed:", e);
    }
  }

  /// Remove breakpoint
  async removeBreakpoint(id: number): Promise<void> {
    if (!this.debugger) return;
    try {
      await this.debugger.removeBreakpoint(id);
    } catch (e) {
      console.error("Remove breakpoint failed:", e);
    }
  }

  /// Add watch expression
  addWatch(expression: string): void {
    const id = `watch-${Date.now()}`;
    this.state.watches.push({
      id,
      expression,
    });
    this.render();
  }

  /// Remove watch expression
  removeWatch(id: string): void {
    this.state.watches = this.state.watches.filter((w) => w.id !== id);
    this.render();
  }

  /// Add error message
  private addError(message: string): void {
    console.error(`Debugger: ${message}`);
  }

  /// Render debugger panel
  private render(): void {
    if (!this.container) return;

    const html = `
      <div class="debugger-panel">
        ${this.renderToolbar()}
        <div class="debugger-content">
          ${this.renderStatus()}
          ${this.renderStackTrace()}
          ${this.renderVariables()}
          ${this.renderWatches()}
          ${this.renderBreakpoints()}
        </div>
      </div>
    `;

    this.container.innerHTML = html;
    this.attachEventListeners();
  }

  /// Render toolbar with controls
  private renderToolbar(): string {
    const controls = [
      { id: "dbg-launch", label: "▶ Launch", disabled: this.state.running },
      { id: "dbg-continue", label: "▶ Continue", disabled: !this.state.paused },
      { id: "dbg-pause", label: "⏸ Pause", disabled: !this.state.running },
      { id: "dbg-step-over", label: "↓ Step Over", disabled: !this.state.paused },
      { id: "dbg-step-into", label: "⬇ Step Into", disabled: !this.state.paused },
      { id: "dbg-step-out", label: "⬆ Step Out", disabled: !this.state.paused },
      { id: "dbg-terminate", label: "⏹ Stop", disabled: !this.state.running && !this.state.paused },
    ];

    return `
      <div class="debugger-toolbar">
        ${controls
          .map(
            (ctrl) => `
          <button class="dbg-btn" id="${ctrl.id}" ${ctrl.disabled ? "disabled" : ""}>
            ${ctrl.label}
          </button>
        `
          )
          .join("")}
      </div>
    `;
  }

  /// Render execution status
  private renderStatus(): string {
    const status = this.state.running
      ? "🟢 Running"
      : this.state.paused
        ? `🔴 Paused (${this.state.stopReason || "unknown"})`
        : "⚪ Stopped";

    const location =
      this.state.currentFile && this.state.currentLine
        ? `${this.state.currentFile}:${this.state.currentLine}:${this.state.currentColumn || 0}`
        : "(none)";

    return `
      <div class="dbg-status">
        <div class="status-indicator">${status}</div>
        <div class="status-location">${escapeHtml(location)}</div>
      </div>
    `;
  }

  /// Render stack trace
  private renderStackTrace(): string {
    if (this.state.stackFrames.length === 0) {
      return "";
    }

    return `
      <div class="dbg-stack">
        <details open>
          <summary>Call Stack (${this.state.stackFrames.length})</summary>
          ${this.state.stackFrames
            .map(
              (frame) => `
            <div class="dbg-frame" data-frame-id="${frame.id}">
              <div class="dbg-frame-name">${escapeHtml(frame.name)}</div>
              <div class="dbg-frame-file">${escapeHtml(frame.file)}:${frame.line}</div>
              <div class="dbg-frame-addr"><code>${frame.address}</code></div>
            </div>
          `
            )
            .join("")}
        </details>
      </div>
    `;
  }

  /// Render variables
  private renderVariables(): string {
    if (this.state.variables.length === 0) {
      return "";
    }

    return `
      <div class="dbg-variables">
        <details open>
          <summary>Variables (${this.state.variables.length})</summary>
          ${this.state.variables
            .map(
              (v) => `
            <div class="dbg-variable">
              <span class="dbg-var-name">${escapeHtml(v.name)}</span>
              <span class="dbg-var-type">${escapeHtml(v.type)}</span>
              <span class="dbg-var-value"><code>${escapeHtml(v.value)}</code></span>
            </div>
          `
            )
            .join("")}
        </details>
      </div>
    `;
  }

  /// Render watch expressions
  private renderWatches(): string {
    return `
      <div class="dbg-watches">
        <details open>
          <summary>Watches (${this.state.watches.length})</summary>
          ${this.state.watches
            .map(
              (w) => `
            <div class="dbg-watch">
              <input class="dbg-watch-expr" value="${escapeHtml(w.expression)}" readonly>
              <span class="dbg-watch-value">${escapeHtml(w.value || "?")}</span>
              <button class="dbg-watch-delete" data-watch-id="${w.id}">✕</button>
            </div>
          `
            )
            .join("")}
          <div class="dbg-watch-add">
            <input class="dbg-watch-input" placeholder="Add watch expression...">
            <button class="dbg-watch-btn-add">Add</button>
          </div>
        </details>
      </div>
    `;
  }

  /// Render breakpoints
  private renderBreakpoints(): string {
    if (this.state.breakpoints.length === 0) {
      return "";
    }

    return `
      <div class="dbg-breakpoints">
        <details>
          <summary>Breakpoints (${this.state.breakpoints.length})</summary>
          ${this.state.breakpoints
            .map(
              (bp) => `
            <div class="dbg-breakpoint ${bp.verified ? "verified" : "unverified"}">
              <span class="dbg-bp-line">${escapeHtml(bp.file)}:${bp.line}</span>
              ${bp.condition ? `<span class="dbg-bp-cond">if ${escapeHtml(bp.condition)}</span>` : ""}
              ${bp.hitCount !== undefined ? `<span class="dbg-bp-hits">${bp.hitCount}</span>` : ""}
              <button class="dbg-bp-delete" data-bp-id="${bp.id}">✕</button>
            </div>
          `
            )
            .join("")}
        </details>
      </div>
    `;
  }

  /// Attach event listeners
  private attachEventListeners(): void {
    if (!this.container) return;

    // Toolbar buttons
    this.container.querySelector("#dbg-launch")?.addEventListener("click", () => this.launch("./a.out"));
    this.container.querySelector("#dbg-continue")?.addEventListener("click", () => this.continue());
    this.container.querySelector("#dbg-pause")?.addEventListener("click", () => this.pause());
    this.container.querySelector("#dbg-step-over")?.addEventListener("click", () => this.stepOver());
    this.container.querySelector("#dbg-step-into")?.addEventListener("click", () => this.stepInto());
    this.container.querySelector("#dbg-step-out")?.addEventListener("click", () => this.stepOut());
    this.container.querySelector("#dbg-terminate")?.addEventListener("click", () => this.terminate());

    // Stack frame selection
    this.container.querySelectorAll(".dbg-frame").forEach((elem) => {
      elem.addEventListener("click", () => {
        const frameId = parseInt(elem.getAttribute("data-frame-id") || "0");
        console.log(`Selected frame ${frameId}`);
      });
    });

    // Watch management
    const addWatchBtn = this.container.querySelector(".dbg-watch-btn-add");
    const watchInput = this.container.querySelector(".dbg-watch-input") as HTMLInputElement;
    if (addWatchBtn && watchInput) {
      addWatchBtn.addEventListener("click", () => {
        if (watchInput.value) {
          this.addWatch(watchInput.value);
        }
      });
    }

    this.container.querySelectorAll(".dbg-watch-delete").forEach((btn) => {
      btn.addEventListener("click", () => {
        const watchId = btn.getAttribute("data-watch-id");
        if (watchId) this.removeWatch(watchId);
      });
    });

    // Breakpoint deletion
    this.container.querySelectorAll(".dbg-bp-delete").forEach((btn) => {
      btn.addEventListener("click", () => {
        const bpId = parseInt(btn.getAttribute("data-bp-id") || "0");
        this.removeBreakpoint(bpId);
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
let debuggerPanelInstance: DebuggerPanelIntegration | null = null;

/// Initialize debugger panel globally
export function initializeDebuggerPanel(debuggerInstance: AuraDebugger): void {
  debuggerPanelInstance = new DebuggerPanelIntegration();
  debuggerPanelInstance.initialize("debugger-panel-container", debuggerInstance);
}

/// Public API for debugger control
export const debuggerAPI = {
  launch: (target: string, args?: string[]) => debuggerPanelInstance?.launch(target, args),
  continue: () => debuggerPanelInstance?.continue(),
  pause: () => debuggerPanelInstance?.pause(),
  stepOver: () => debuggerPanelInstance?.stepOver(),
  stepInto: () => debuggerPanelInstance?.stepInto(),
  stepOut: () => debuggerPanelInstance?.stepOut(),
  terminate: () => debuggerPanelInstance?.terminate(),
  setBreakpoint: (file: string, line: number, cond?: string) =>
    debuggerPanelInstance?.setBreakpoint(file, line, cond),
  removeBreakpoint: (id: number) => debuggerPanelInstance?.removeBreakpoint(id),
  addWatch: (expr: string) => debuggerPanelInstance?.addWatch(expr),
  removeWatch: (id: string) => debuggerPanelInstance?.removeWatch(id),
};
