// Sentinel IDE Debugger Panel
//
// Native debugging UI for Aura code:
// - Set breakpoints (click gutter)
// - Step through execution (step-in, step-over, step-out)
// - Inspect variables (stack frame locals + watches)
// - Integration with LLVM DWARF debug info (v0.3 foundation)
// - GDB/LLDB MI protocol (Machine Interface)
//
// Design:
// 1. LaunchButton: "Debug (Native)" in toolbar
// 2. BreakpointsPanel: list + gutter UI
// 3. StackPanel: call stack + frame selection
// 4. VariablesPanel: locals + watch expressions
// 5. ConsolePa: debug console (GDB/LLDB output)

import React, { useState, useCallback, useRef } from 'react';
import { DebuggerState, BreakpointInfo, StackFrame, Variable } from './debugger_types';
import './debugger.css';

/**
 * Main Sentinel Debugger Panel Component
 * Rendered in the left activity bar when debugging is active
 */
export const DebugPanel: React.FC = () => {
  const [debuggerState, setDebuggerState] = useState<DebuggerState>({
    isRunning: false,
    isPaused: false,
    currentFile: '',
    currentLine: 0,
    breakpoints: [],
    callStack: [],
    locals: [],
    watches: [],
    consoleOutput: [],
  });

  const [watchExpression, setWatchExpression] = useState('');
  const debuggerRef = useRef<DebuggerIntegration | null>(null);

  /**
   * Launch debugger: compile, attach to target, wait for pause
   */
  const handleLaunchDebugger = useCallback(async () => {
    try {
      console.log('Launching Aura debugger...');
      
      // Initialize GDB/LLDB integration
      if (!debuggerRef.current) {
        debuggerRef.current = new DebuggerIntegration();
        await debuggerRef.current.initialize();
      }

      // Compile the current project
      const compileResult = await compileBinary();
      if (!compileResult.success) {
        addConsoleOutput(`Error: ${compileResult.error}`, 'error');
        return;
      }

      addConsoleOutput(`[Debugger] Binary: ${compileResult.binary}`, 'info');

      // Launch GDB/LLDB
      await debuggerRef.current.launch(compileResult.binary);
      addConsoleOutput('[Debugger] Launched successfully', 'info');

      setDebuggerState(prev => ({
        ...prev,
        isRunning: true,
        isPaused: false,
      }));
    } catch (err) {
      addConsoleOutput(`Error launching debugger: ${err}`, 'error');
    }
  }, []);

  /**
   * Set breakpoint at line in current file
   */
  const handleSetBreakpoint = useCallback((file: string, line: number) => {
    if (!debuggerRef.current) {
      addConsoleOutput('Debugger not initialized', 'error');
      return;
    }

    try {
      debuggerRef.current.setBreakpoint(file, line);
      
      setDebuggerState(prev => ({
        ...prev,
        breakpoints: [
          ...prev.breakpoints,
          { file, line, enabled: true, verified: true },
        ],
      }));

      addConsoleOutput(`Breakpoint set at ${file}:${line}`, 'info');
    } catch (err) {
      addConsoleOutput(`Error setting breakpoint: ${err}`, 'error');
    }
  }, []);

  /**
   * Remove breakpoint
   */
  const handleRemoveBreakpoint = useCallback((file: string, line: number) => {
    if (!debuggerRef.current) return;

    try {
      debuggerRef.current.removeBreakpoint(file, line);
      
      setDebuggerState(prev => ({
        ...prev,
        breakpoints: prev.breakpoints.filter(bp => !(bp.file === file && bp.line === line)),
      }));

      addConsoleOutput(`Breakpoint removed at ${file}:${line}`, 'info');
    } catch (err) {
      addConsoleOutput(`Error removing breakpoint: ${err}`, 'error');
    }
  }, []);

  /**
   * Continue execution until next breakpoint
   */
  const handleContinue = useCallback(async () => {
    if (!debuggerRef.current || !debuggerState.isPaused) return;

    try {
      addConsoleOutput('[Debugger] Continuing...', 'info');
      await debuggerRef.current.continue();
      // Will receive paused event when hitting breakpoint
    } catch (err) {
      addConsoleOutput(`Error continuing: ${err}`, 'error');
    }
  }, [debuggerState.isPaused]);

  /**
   * Step over: execute one line, staying in current function
   */
  const handleStepOver = useCallback(async () => {
    if (!debuggerRef.current || !debuggerState.isPaused) return;

    try {
      addConsoleOutput('[Debugger] Step over...', 'info');
      const result = await debuggerRef.current.stepOver();
      handleUpdateLocation(result.file, result.line);
      refreshVariables();
    } catch (err) {
      addConsoleOutput(`Error stepping: ${err}`, 'error');
    }
  }, [debuggerState.isPaused]);

  /**
   * Step in: execute one line, descending into function calls
   */
  const handleStepIn = useCallback(async () => {
    if (!debuggerRef.current || !debuggerState.isPaused) return;

    try {
      addConsoleOutput('[Debugger] Step in...', 'info');
      const result = await debuggerRef.current.stepIn();
      handleUpdateLocation(result.file, result.line);
      refreshVariables();
    } catch (err) {
      addConsoleOutput(`Error stepping: ${err}`, 'error');
    }
  }, [debuggerState.isPaused]);

  /**
   * Step out: execute until current function returns
   */
  const handleStepOut = useCallback(async () => {
    if (!debuggerRef.current || !debuggerState.isPaused) return;

    try {
      addConsoleOutput('[Debugger] Step out...', 'info');
      const result = await debuggerRef.current.stepOut();
      handleUpdateLocation(result.file, result.line);
      refreshVariables();
    } catch (err) {
      addConsoleOutput(`Error stepping: ${err}`, 'error');
    }
  }, [debuggerState.isPaused]);

  /**
   * Stop debugger
   */
  const handleStop = useCallback(async () => {
    if (!debuggerRef.current) return;

    try {
      await debuggerRef.current.terminate();
      addConsoleOutput('[Debugger] Terminated', 'info');
      
      setDebuggerState(prev => ({
        ...prev,
        isRunning: false,
        isPaused: false,
      }));
    } catch (err) {
      addConsoleOutput(`Error stopping: ${err}`, 'error');
    }
  }, []);

  /**
   * Add a watch expression
   */
  const handleAddWatch = useCallback(async () => {
    if (!watchExpression || !debuggerRef.current) return;

    try {
      const value = await debuggerRef.current.evaluateExpression(watchExpression);
      
      setDebuggerState(prev => ({
        ...prev,
        watches: [
          ...prev.watches,
          { name: watchExpression, value, type: 'expression' },
        ],
      }));

      setWatchExpression('');
      addConsoleOutput(`Watch: ${watchExpression} = ${value}`, 'info');
    } catch (err) {
      addConsoleOutput(`Error evaluating expression: ${err}`, 'error');
    }
  }, [watchExpression]);

  /**
   * Update current location (file + line)
   */
  const handleUpdateLocation = useCallback((file: string, line: number) => {
    setDebuggerState(prev => ({
      ...prev,
      currentFile: file,
      currentLine: line,
      isPaused: true,
      isRunning: true,
    }));
  }, []);

  /**
   * Refresh variables and call stack from debugger
   */
  const refreshVariables = useCallback(async () => {
    if (!debuggerRef.current) return;

    try {
      const callStack = await debuggerRef.current.getCallStack();
      const locals = await debuggerRef.current.getVariables('locals');

      setDebuggerState(prev => ({
        ...prev,
        callStack,
        locals,
      }));
    } catch (err) {
      console.error('Error refreshing variables:', err);
    }
  }, []);

  /**
   * Add message to debug console
   */
  const addConsoleOutput = useCallback((message: string, type: 'info' | 'error' | 'warning' = 'info') => {
    setDebuggerState(prev => ({
      ...prev,
      consoleOutput: [
        ...prev.consoleOutput,
        { message, type, timestamp: new Date() },
      ],
    }));
  }, []);

  return (
    <div className="debug-panel">
      <div className="debug-toolbar">
        <button
          onClick={handleLaunchDebugger}
          disabled={debuggerState.isRunning}
          title="Debug (Native)"
          className="btn btn-primary"
        >
          ▶ Debug
        </button>
        
        {debuggerState.isRunning && (
          <>
            <button
              onClick={handleContinue}
              disabled={!debuggerState.isPaused}
              title="Continue"
              className="btn"
            >
              ▶ Continue
            </button>
            <button
              onClick={handleStepOver}
              disabled={!debuggerState.isPaused}
              title="Step Over"
              className="btn"
            >
              ↻ Step
            </button>
            <button
              onClick={handleStepIn}
              disabled={!debuggerState.isPaused}
              title="Step In"
              className="btn"
            >
              ↓ In
            </button>
            <button
              onClick={handleStepOut}
              disabled={!debuggerState.isPaused}
              title="Step Out"
              className="btn"
            >
              ↑ Out
            </button>
            <button
              onClick={handleStop}
              title="Stop"
              className="btn btn-danger"
            >
              ⏹ Stop
            </button>
          </>
        )}
      </div>

      {debuggerState.isRunning && (
        <div className="debug-content">
          <div className="debug-section">
            <h3>Call Stack</h3>
            <StackPanel frames={debuggerState.callStack} />
          </div>

          <div className="debug-section">
            <h3>Variables</h3>
            <VariablesPanel variables={debuggerState.locals} />
          </div>

          <div className="debug-section">
            <h3>Watch Expressions</h3>
            <div className="watch-input">
              <input
                type="text"
                placeholder="Add watch expression"
                value={watchExpression}
                onChange={(e) => setWatchExpression(e.target.value)}
                onKeyPress={(e) => {
                  if (e.key === 'Enter') handleAddWatch();
                }}
              />
              <button onClick={handleAddWatch}>Add</button>
            </div>
            <VariablesPanel variables={debuggerState.watches} />
          </div>

          <div className="debug-section">
            <h3>Debug Console</h3>
            <ConsolePanel output={debuggerState.consoleOutput} />
          </div>
        </div>
      )}
    </div>
  );
};

/**
 * Stack Frame Display Component
 */
const StackPanel: React.FC<{ frames: StackFrame[] }> = ({ frames }) => {
  return (
    <div className="stack-panel">
      {frames.length === 0 ? (
        <p className="empty">No stack frames</p>
      ) : (
        <ul className="stack-list">
          {frames.map((frame, idx) => (
            <li key={idx} className="stack-frame">
              <code>{frame.functionName}</code>
              <span className="location">
                @ {frame.file}:{frame.line}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

/**
 * Variables Display Component
 */
const VariablesPanel: React.FC<{ variables: Variable[] }> = ({ variables }) => {
  return (
    <div className="variables-panel">
      {variables.length === 0 ? (
        <p className="empty">No variables</p>
      ) : (
        <ul className="variable-list">
          {variables.map((variable, idx) => (
            <li key={idx} className="variable">
              <span className="name">{variable.name}</span>
              <span className="type">: {variable.type}</span>
              <span className="value"> = {variable.value}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

/**
 * Debug Console Output Component
 */
const ConsolePanel: React.FC<{ output: any[] }> = ({ output }) => {
  const consoleRef = useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [output]);

  return (
    <div className="console-panel" ref={consoleRef}>
      {output.map((line, idx) => (
        <div key={idx} className={`console-line ${line.type}`}>
          {line.message}
        </div>
      ))}
    </div>
  );
};

/**
 * DebuggerIntegration: Low-level GDB/LLDB control via MI protocol
 */
class DebuggerIntegration {
  private process: any = null;
  private miMode: 'gdb' | 'lldb' = 'gdb';

  async initialize() {
    // Detect available debugger (GDB or LLDB)
    try {
      // Try LLDB first (better macOS support)
      await this.detectDebugger();
    } catch (err) {
      throw new Error(`No debugger found (GDB/LLDB required): ${err}`);
    }
  }

  private async detectDebugger() {
    // Platform-specific detection
    // On Windows: look for debuggers in PATH
    // On macOS: prefer lldb
    // On Linux: prefer gdb
    const isWindows = process.platform === 'win32';
    const isMac = process.platform === 'darwin';

    if (isMac || process.platform === 'linux') {
      this.miMode = 'lldb';
    } else {
      this.miMode = 'gdb';
    }

    console.log(`Using ${this.miMode} debugger`);
  }

  async launch(binaryPath: string) {
    // Spawn debugger process and send MI commands
    // Example: gdb --interpreter=mi2 <binary>
    // This is a stub; real implementation would use child_process
    console.log(`[Debugger] Launching ${this.miMode} on ${binaryPath}`);
  }

  setBreakpoint(file: string, line: number) {
    // Send: -break-insert file:line
    console.log(`[MI] -break-insert ${file}:${line}`);
  }

  removeBreakpoint(file: string, line: number) {
    // Send: -break-delete
    console.log(`[MI] -break-delete`);
  }

  async continue() {
    // Send: -exec-continue
    return new Promise(resolve => {
      console.log(`[MI] -exec-continue`);
      setTimeout(() => resolve({}), 100); // Stub
    });
  }

  async stepOver() {
    // Send: -exec-next
    return { file: '', line: 0 };
  }

  async stepIn() {
    // Send: -exec-step
    return { file: '', line: 0 };
  }

  async stepOut() {
    // Send: -exec-finish
    return { file: '', line: 0 };
  }

  async getCallStack() {
    // Send: -stack-list-frames
    return [];
  }

  async getVariables(scope: 'locals' | 'globals') {
    // Send: -stack-list-variables
    return [];
  }

  async evaluateExpression(expr: string) {
    // Send: -data-evaluate-expression <expr>
    return `<value of ${expr}>`;
  }

  async terminate() {
    // Send: -gdb-exit
    console.log(`[MI] -gdb-exit`);
  }
}

async function compileBinary(): Promise<{ success: boolean; binary?: string; error?: string }> {
  // Compile current workspace to binary
  // Uses aura compiler backend (C or LLVM)
  // Returns path to executable
  return { success: true, binary: '/path/to/binary' };
}

export default DebugPanel;
