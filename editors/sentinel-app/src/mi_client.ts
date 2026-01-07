/**
 * GDB/LLDB Machine Interface (MI) Protocol Client
 * 
 * Implements the GDB MI protocol for debugger communication.
 * Supports both GDB and LLDB (which exposes GDB MI interface).
 * 
 * Reference: https://sourceware.org/gdb/onlinedocs/gdb/GDB_002fMI.html
 */

export interface MICommand {
  /** Command token (for matching responses) */
  token: number;
  /** Command type (e.g., "exec-run", "exec-step") */
  command: string;
  /** Optional arguments */
  args?: Record<string, string>;
}

export interface MIResponse {
  /** Token from the original command */
  token: number;
  /** Response type: "done", "running", "connected", "error", "exit" */
  resultClass: 'done' | 'running' | 'connected' | 'error' | 'exit';
  /** Result data (varies by command) */
  result?: Record<string, any>;
  /** Error message (if resultClass === 'error') */
  message?: string;
}

export interface Breakpoint {
  /** Breakpoint number (assigned by debugger) */
  number: string;
  /** Is breakpoint enabled */
  enabled: boolean;
  /** File path */
  file: string;
  /** Line number (1-indexed) */
  line: number;
  /** Function name where breakpoint is set */
  func?: string;
  /** Number of times breakpoint was hit */
  times: number;
  /** Breakpoint condition (if any) */
  condition?: string;
}

export interface StackFrame {
  /** Frame number (0 = innermost) */
  level: number;
  /** Program counter address */
  addr: string;
  /** Function name */
  func: string;
  /** File path */
  file: string;
  /** Line number (1-indexed) */
  line: number;
  /** Column number (optional) */
  col?: number;
}

export interface Variable {
  /** Variable name */
  name: string;
  /** Variable type */
  type: string;
  /** Variable value (as string) */
  value: string;
  /** Can be expanded (has children) */
  numchild: number;
  /** Is dynamic (value changes based on runtime state) */
  dynamic?: boolean;
}

export interface DebuggerState {
  /** Is execution stopped (at breakpoint, etc.) */
  stopped: boolean;
  /** Stop reason: "breakpoint-hit", "end-stepping-range", "exited-normally", etc. */
  stopReason?: string;
  /** Thread ID that stopped (if multi-threaded) */
  threadId?: string;
  /** Current stack frames */
  frames: StackFrame[];
  /** Current thread variables */
  variables: Variable[];
  /** All known breakpoints */
  breakpoints: Breakpoint[];
}

/**
 * GDB/LLDB MI Protocol Client
 * 
 * Manages communication with a GDB or LLDB debugger instance.
 * Sends MI commands and parses MI responses.
 * Maintains debugger state and emits events.
 */
export class MIClient {
  private commandToken = 0;
  private debuggerState: DebuggerState = {
    stopped: false,
    frames: [],
    variables: [],
    breakpoints: [],
  };
  private callbacks: Map<number, (response: MIResponse) => void> = new Map();
  private eventListeners: Map<string, ((data: any) => void)[]> = new Map();

  constructor(private debuggerProcess?: NodeJS.Process) {}

  /**
   * Send a MI command to the debugger.
   */
  async sendCommand(command: MICommand): Promise<MIResponse> {
    return new Promise((resolve, reject) => {
      const token = ++this.commandToken;
      const cmdStr = this.formatCommand(token, command);

      // Register response callback
      this.callbacks.set(token, (response: MIResponse) => {
        this.callbacks.delete(token);
        if (response.resultClass === 'error') {
          reject(new Error(response.message || `MI error: ${command.command}`));
        } else {
          resolve(response);
        }
      });

      // Send command (in real implementation, write to debugger process stdin)
      console.log(`[MI] ${cmdStr}`);

      // Timeout after 10s
      setTimeout(() => {
        if (this.callbacks.has(token)) {
          this.callbacks.delete(token);
          reject(new Error(`Timeout waiting for response to: ${command.command}`));
        }
      }, 10000);
    });
  }

  /**
   * Format a MI command string.
   */
  private formatCommand(token: number, cmd: MICommand): string {
    const parts = [token.toString(), cmd.command];
    
    if (cmd.args) {
      for (const [key, value] of Object.entries(cmd.args)) {
        parts.push(`--${key} "${value}"`);
      }
    }

    return parts.join(' ');
  }

  /**
   * Parse a MI response string.
   */
  private parseResponse(line: string): MIResponse | null {
    const match = line.match(/^(\d+)\^(\w+)(.*)/);
    if (!match) return null;

    const [, tokenStr, resultClass, dataStr] = match;
    const token = parseInt(tokenStr, 10);

    return {
      token,
      resultClass: resultClass as any,
      result: dataStr ? this.parseResultData(dataStr) : undefined,
    };
  }

  /**
   * Parse MI result data (simple implementation).
   */
  private parseResultData(data: string): Record<string, any> {
    const result: Record<string, any> = {};
    // Basic parsing: "key=\"value\",key2=\"value2\""
    const pairs = data.match(/(\w+)="([^"]*)"/g) || [];
    
    for (const pair of pairs) {
      const [key, value] = pair.split('=');
      result[key] = value?.slice(1, -1);  // Remove quotes
    }

    return result;
  }

  /**
   * Execute target (run the program).
   */
  async executeTarget(): Promise<void> {
    const response = await this.sendCommand({
      token: 0,
      command: 'exec-run',
    });

    if (response.resultClass === 'running') {
      this.emit('running', {});
    }
  }

  /**
   * Continue execution.
   */
  async continue(): Promise<void> {
    const response = await this.sendCommand({
      token: 0,
      command: 'exec-continue',
    });

    if (response.resultClass === 'running') {
      this.emit('running', {});
    }
  }

  /**
   * Step over (next instruction).
   */
  async stepOver(): Promise<void> {
    const response = await this.sendCommand({
      token: 0,
      command: 'exec-next',
    });

    if (response.resultClass === 'running') {
      this.emit('running', {});
    }
  }

  /**
   * Step into (descend into function).
   */
  async stepInto(): Promise<void> {
    const response = await this.sendCommand({
      token: 0,
      command: 'exec-step',
    });

    if (response.resultClass === 'running') {
      this.emit('running', {});
    }
  }

  /**
   * Step out (exit current function).
   */
  async stepOut(): Promise<void> {
    const response = await this.sendCommand({
      token: 0,
      command: 'exec-finish',
    });

    if (response.resultClass === 'running') {
      this.emit('running', {});
    }
  }

  /**
   * Set a breakpoint at file:line.
   */
  async setBreakpoint(file: string, line: number): Promise<Breakpoint> {
    const response = await this.sendCommand({
      token: 0,
      command: 'break-insert',
      args: { file, line: line.toString() },
    });

    const bkpt = response.result?.bkpt || {};
    const breakpoint: Breakpoint = {
      number: bkpt.number || '0',
      enabled: bkpt.enabled !== 'n',
      file: bkpt.file || file,
      line: parseInt(bkpt.line || line.toString(), 10),
      func: bkpt.func,
      times: 0,
    };

    this.debuggerState.breakpoints.push(breakpoint);
    this.emit('breakpoint-set', breakpoint);
    return breakpoint;
  }

  /**
   * Remove a breakpoint by number.
   */
  async removeBreakpoint(number: string): Promise<void> {
    await this.sendCommand({
      token: 0,
      command: 'break-delete',
      args: { number },
    });

    this.debuggerState.breakpoints = this.debuggerState.breakpoints.filter(
      (b) => b.number !== number
    );
    this.emit('breakpoint-removed', number);
  }

  /**
   * Get current stack trace.
   */
  async stackTrace(): Promise<StackFrame[]> {
    const response = await this.sendCommand({
      token: 0,
      command: 'stack-list-frames',
    });

    const frames: StackFrame[] = [];
    const frameList = response.result?.frame || [];

    for (const f of frameList) {
      frames.push({
        level: parseInt(f.level || '0', 10),
        addr: f.addr || '0x0',
        func: f.func || '<unknown>',
        file: f.file || '<unknown>',
        line: parseInt(f.line || '0', 10),
        col: f.col ? parseInt(f.col, 10) : undefined,
      });
    }

    this.debuggerState.frames = frames;
    return frames;
  }

  /**
   * Get variables in current frame.
   */
  async frameVariables(): Promise<Variable[]> {
    const response = await this.sendCommand({
      token: 0,
      command: 'stack-list-variables',
      args: { 'no-frame-filters': 'false' },
    });

    const variables: Variable[] = [];
    const varList = response.result?.variables || [];

    for (const v of varList) {
      variables.push({
        name: v.name || '<unknown>',
        type: v.type || 'unknown',
        value: v.value || '<uninitialized>',
        numchild: parseInt(v.numchild || '0', 10),
        dynamic: v.dynamic === '1',
      });
    }

    this.debuggerState.variables = variables;
    return variables;
  }

  /**
   * Emit an event to listeners.
   */
  private emit(event: string, data: any): void {
    const listeners = this.eventListeners.get(event) || [];
    for (const listener of listeners) {
      listener(data);
    }
  }

  /**
   * Register an event listener.
   */
  on(event: string, listener: (data: any) => void): void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event)!.push(listener);
  }

  /**
   * Get current debugger state.
   */
  getState(): DebuggerState {
    return this.debuggerState;
  }

  /**
   * Update debugger state based on stop event.
   */
  updateStateOnStop(stopReason: string, threadId?: string): void {
    this.debuggerState.stopped = true;
    this.debuggerState.stopReason = stopReason;
    this.debuggerState.threadId = threadId;
    this.emit('stopped', { stopReason, threadId });
  }

  /**
   * Update debugger state based on resumed execution.
   */
  updateStateOnRunning(): void {
    this.debuggerState.stopped = false;
    this.debuggerState.frames = [];
    this.debuggerState.variables = [];
    this.emit('running', {});
  }
}

export default MIClient;
