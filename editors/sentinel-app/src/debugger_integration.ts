/**
 * GDB/LLDB Debugger Integration for Sentinel
 * 
 * Bridges the MIClient with the Sentinel IDE debugger panel.
 * Manages debugger lifecycle, handles events, and updates UI state.
 */

import MIClient, { Breakpoint, StackFrame, Variable, DebuggerState } from './mi_client';

export interface DebuggerSession {
  /** Debugger executable path (gdb or lldb) */
  debuggerPath: string;
  /** Target executable path */
  targetPath: string;
  /** Working directory */
  workDir?: string;
  /** Arguments to pass to target */
  targetArgs?: string[];
  /** Environment variables */
  env?: Record<string, string>;
}

/**
 * Aura Debugger Integration
 * Connects GDB/LLDB MI protocol to Sentinel IDE.
 */
export class AuraDebugger {
  private miClient: MIClient | null = null;
  private session: DebuggerSession | null = null;
  private listeners: Map<string, ((data: any) => void)[]> = new Map();
  private initialized = false;

  /**
   * Initialize debugger with a target executable.
   */
  async init(session: DebuggerSession): Promise<void> {
    this.session = session;

    // Create MI client (in real implementation, spawn debugger process)
    this.miClient = new MIClient();

    // Register MI event listeners
    this.miClient.on('stopped', (data) => this.onDebuggerStopped(data));
    this.miClient.on('running', () => this.onDebuggerRunning());
    this.miClient.on('breakpoint-set', (bp) => this.onBreakpointSet(bp));
    this.miClient.on('breakpoint-removed', (number) => this.onBreakpointRemoved(number));

    this.initialized = true;
    this.emit('initialized', {});
  }

  /**
   * Start execution of the target.
   */
  async run(): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.executeTarget();
  }

  /**
   * Continue execution from a breakpoint.
   */
  async continue(): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.continue();
  }

  /**
   * Step over the next instruction.
   */
  async next(): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.stepOver();
  }

  /**
   * Step into the next function.
   */
  async step(): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.stepInto();
  }

  /**
   * Step out of the current function.
   */
  async stepOut(): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.stepOut();
  }

  /**
   * Set a breakpoint at a specific location.
   */
  async setBreakpoint(file: string, line: number): Promise<Breakpoint> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    return await this.miClient.setBreakpoint(file, line);
  }

  /**
   * Remove a breakpoint.
   */
  async removeBreakpoint(number: string): Promise<void> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    await this.miClient.removeBreakpoint(number);
  }

  /**
   * Get current call stack.
   */
  async getStack(): Promise<StackFrame[]> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    return await this.miClient.stackTrace();
  }

  /**
   * Get variables in current frame.
   */
  async getVariables(): Promise<Variable[]> {
    if (!this.miClient) throw new Error('Debugger not initialized');

    return await this.miClient.frameVariables();
  }

  /**
   * Get current debugger state.
   */
  getState(): DebuggerState | null {
    if (!this.miClient) return null;

    return this.miClient.getState();
  }

  /**
   * Terminate the debugger session.
   */
  async terminate(): Promise<void> {
    if (!this.miClient) return;

    // In real implementation, kill debugger process
    this.initialized = false;
    this.emit('terminated', {});
  }

  /**
   * Handle debugger stopping event.
   */
  private async onDebuggerStopped(data: { stopReason?: string; threadId?: string }): Promise<void> {
    if (!this.miClient) return;

    this.miClient.updateStateOnStop(data.stopReason || 'unknown', data.threadId);

    // Fetch stack and variables on stop
    const frames = await this.miClient.stackTrace();
    const variables = await this.miClient.frameVariables();

    this.emit('stopped', {
      stopReason: data.stopReason || 'unknown',
      frames,
      variables,
    });
  }

  /**
   * Handle debugger running event.
   */
  private onDebuggerRunning(): void {
    if (!this.miClient) return;

    this.miClient.updateStateOnRunning();
    this.emit('running', {});
  }

  /**
   * Handle breakpoint set event.
   */
  private onBreakpointSet(breakpoint: Breakpoint): void {
    this.emit('breakpoint-set', breakpoint);
  }

  /**
   * Handle breakpoint removed event.
   */
  private onBreakpointRemoved(number: string): void {
    this.emit('breakpoint-removed', number);
  }

  /**
   * Emit an event.
   */
  private emit(event: string, data: any): void {
    const listeners = this.listeners.get(event) || [];
    for (const listener of listeners) {
      listener(data);
    }
  }

  /**
   * Register an event listener.
   */
  on(event: string, listener: (data: any) => void): void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, []);
    }
    this.listeners.get(event)!.push(listener);
  }

  /**
   * Check if debugger is initialized.
   */
  isInitialized(): boolean {
    return this.initialized;
  }
}

/**
 * Debugger Integration Example
 */
export async function createDebuggerSession(
  targetPath: string,
  workDir?: string
): Promise<AuraDebugger> {
  const debugger_ = new AuraDebugger();

  // Auto-detect GDB vs LLDB based on platform
  const debuggerPath =
    process.platform === 'darwin' ? 'lldb' : 'gdb';

  await debugger_.init({
    debuggerPath,
    targetPath,
    workDir,
  });

  return debugger_;
}

export default AuraDebugger;
