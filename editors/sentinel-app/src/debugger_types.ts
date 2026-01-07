// Type definitions for Sentinel Debugger

export interface DebuggerState {
  isRunning: boolean;
  isPaused: boolean;
  currentFile: string;
  currentLine: number;
  breakpoints: BreakpointInfo[];
  callStack: StackFrame[];
  locals: Variable[];
  watches: Variable[];
  consoleOutput: ConsoleOutput[];
}

export interface BreakpointInfo {
  file: string;
  line: number;
  enabled: boolean;
  verified: boolean;
  condition?: string;
  hitCount?: number;
}

export interface StackFrame {
  id: number;
  functionName: string;
  file: string;
  line: number;
  column?: number;
}

export interface Variable {
  name: string;
  value: string;
  type: string;
  presentationHint?: {
    kind?: 'class' | 'method' | 'property' | 'unit' | 'variable' | 'constant' | 'string' | 'number' | 'boolean' | 'array' | 'object';
    attributes?: ('static' | 'constant' | 'readOnly' | 'rawString' | 'hasObjectId' | 'canHaveObjectId' | 'hasSideEffects' | 'hasDataBreakpoint')[];
    visibility?: 'public' | 'private' | 'protected' | 'internal';
    indexed?: boolean;
    memoryReference?: string;
  };
}

export interface ConsoleOutput {
  message: string;
  type: 'info' | 'error' | 'warning' | 'stdout' | 'stderr';
  timestamp: Date;
}

export interface DebugLaunchConfig {
  name: string;
  type: 'aura';
  request: 'launch' | 'attach';
  program: string;
  args: string[];
  cwd: string;
  stopOnEntry?: boolean;
  console?: 'integratedTerminal' | 'externalTerminal' | 'internalConsole';
}

export interface DebugEvent {
  event: 'stopped' | 'continued' | 'thread' | 'output' | 'breakpoint' | 'module' | 'loaded' | 'process' | 'capabilities' | 'memory' | 'invalidated';
  body: any;
}

export interface StoppedEvent extends DebugEvent {
  event: 'stopped';
  body: {
    reason: 'step' | 'breakpoint' | 'breakpoint' | 'exception' | 'pause' | 'entry' | 'goto' | 'function breakpoint' | 'instruction breakpoint';
    threadId?: number;
    allThreadsStopped?: boolean;
    text?: string;
    description?: string;
    preserveFocusWhenOpen?: boolean;
    hitBreakpointIds?: number[];
  };
}
