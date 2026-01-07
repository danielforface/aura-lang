#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

use aura_ast::{Expr, Span};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugBreakpoint {
    pub line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugWatch {
    pub expr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum DebugCommand {
    Enable {
        #[serde(default)]
        start_paused: bool,
        #[serde(default)]
        perf: bool,
    },
    Pause,
    Continue,
    Step,
    /// Request termination of any running program (native child or Dev-VM).
    Terminate,
    SetBreakpoints { breakpoints: Vec<DebugBreakpoint> },
    SetWatches { watches: Vec<DebugWatch> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugWatchValue {
    pub expr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfTimelineEvent {
    pub line: u32,
    pub col: u32,
    pub kind: String,
    pub dur_ns: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfReport {
    pub timeline: Vec<PerfTimelineEvent>,
    pub flame_folded: String,
    pub memory: HashMap<String, u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum DebugEvent {
    Hello {
        protocol: u32,
        capabilities: Vec<String>,
    },
    Stopped {
        reason: String,
        file: String,
        line: u32,
        col: u32,
        env: HashMap<String, String>,
        watches: Vec<DebugWatchValue>,
    },
    PerfReport { report: PerfReport },
    NativeLaunch { exe: String },
    NativeExit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<i32>,
    },
    Terminated {
        /// Termination target, e.g. "devvm" or "native".
        target: String,
    },
}

pub type DebugEmitFn = Arc<dyn Fn(DebugEvent) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct DebugHandle {
    inner: Arc<DebugSessionInner>,
}

#[derive(Clone)]
pub struct DebugSession {
    inner: Arc<DebugSessionInner>,
}

impl std::fmt::Debug for DebugHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DebugHandle(..)")
    }
}

impl std::fmt::Debug for DebugSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DebugSession(..)")
    }
}

impl DebugSession {
    pub const PROTOCOL_VERSION: u32 = 1;

    pub fn new(emit: DebugEmitFn) -> (Self, DebugHandle) {
        let inner = Arc::new(DebugSessionInner::new(emit));
        (
            Self {
                inner: Arc::clone(&inner),
            },
            DebugHandle { inner },
        )
    }

    pub fn set_source(&self, file: String, text: &str) {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        if file.starts_with('<') {
            if st.file.is_empty() {
                st.file = file;
            }
        } else {
            st.file = file;
        }
        st.line_starts.clear();
        st.line_starts.push(0);
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                st.line_starts.push(i + 1);
            }
        }
    }

    pub fn emit(&self, ev: DebugEvent) {
        (self.inner.emit)(ev)
    }

    pub fn line_col(&self, span: Span) -> (u32, u32, String) {
        let st = self.inner.state.lock().expect("debug state poisoned");
        let off: usize = span.offset().into();

        let line_idx = match st.line_starts.binary_search(&off) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };

        let line_start = st.line_starts.get(line_idx).copied().unwrap_or(0);
        let col0 = off.saturating_sub(line_start);
        (
            (line_idx as u32) + 1,
            (col0 as u32) + 1,
            st.file.clone(),
        )
    }

    pub fn apply_command(&self, cmd: DebugCommand) {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        match cmd {
            DebugCommand::Enable { start_paused, perf } => {
                st.enabled = true;
                if start_paused {
                    st.paused = true;
                }
                st.perf_enabled = perf;
            }
            DebugCommand::Pause => {
                st.enabled = true;
                st.paused = true;
            }
            DebugCommand::Continue => {
                st.enabled = true;
                st.paused = false;
                st.step_mode = false;
                st.step_budget = 0;
                self.inner.cv.notify_all();
            }
            DebugCommand::Step => {
                st.enabled = true;
                st.paused = false;
                st.step_mode = true;
                st.step_budget = 1;
                self.inner.cv.notify_all();
            }
            DebugCommand::Terminate => {
                st.enabled = true;
                st.terminate_requested = true;
                self.inner.cv.notify_all();
            }
            DebugCommand::SetBreakpoints { breakpoints } => {
                st.enabled = true;
                st.breakpoints = breakpoints
                    .into_iter()
                    .map(|b| InternalBreakpoint::from_wire(b))
                    .collect();
            }
            DebugCommand::SetWatches { watches } => {
                st.enabled = true;
                st.watches = watches
                    .into_iter()
                    .map(|w| InternalWatch::from_wire(w))
                    .collect();
            }
        }
    }

    pub fn take_terminate_requested(&self) -> bool {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        let out = st.terminate_requested;
        st.terminate_requested = false;
        out
    }

    pub fn take_pending_commands(&self) -> Vec<DebugCommand> {
        let mut q = self.inner.queue.lock().expect("debug queue poisoned");
        q.drain(..).collect()
    }

    pub fn should_stop_before_stmt<F>(&self, span: Span, mut eval_bool: F) -> Option<String>
    where
        F: FnMut(&Expr) -> Result<bool, String>,
    {
        let (line, _col, _file) = self.line_col(span);

        // Apply any queued commands first.
        for c in self.take_pending_commands() {
            self.apply_command(c);
        }

        let mut st = self.inner.state.lock().expect("debug state poisoned");
        if !st.enabled {
            return None;
        }

        // Step mode: after we have executed one statement, we stop before the next.
        if st.step_mode && st.step_budget == 0 && !st.paused {
            st.paused = true;
            return Some("step".to_string());
        }

        if st.paused {
            return Some("pause".to_string());
        }

        // Breakpoints.
        for bp in &mut st.breakpoints {
            if bp.line != line {
                continue;
            }

            match &bp.condition {
                None => {
                    st.paused = true;
                    return Some("breakpoint".to_string());
                }
                Some(cond) => match cond {
                    ParsedExpr::Ok(e) => match eval_bool(e) {
                        Ok(true) => {
                            st.paused = true;
                            return Some("breakpoint".to_string());
                        }
                        Ok(false) => continue,
                        Err(err) => {
                            bp.last_error = Some(err);
                            st.paused = true;
                            return Some("breakpoint".to_string());
                        }
                    },
                    ParsedExpr::Err(_e) => {
                        st.paused = true;
                        return Some("breakpoint".to_string());
                    }
                },
            }
        }

        None
    }

    pub fn note_stmt_executed(&self) {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        if st.step_mode && st.step_budget > 0 {
            st.step_budget = st.step_budget.saturating_sub(1);
        }
    }

    pub fn wait_while_paused(&self) {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        while st.paused && !st.terminate_requested {
            st = self.inner.cv.wait(st).expect("debug state poisoned");
        }

        // If termination was requested while paused, unpause so the VM can unwind.
        if st.terminate_requested {
            st.paused = false;
        }
    }

    pub fn watches(&self) -> Vec<InternalWatch> {
        let st = self.inner.state.lock().expect("debug state poisoned");
        st.watches.clone()
    }

    pub fn perf_enabled(&self) -> bool {
        let st = self.inner.state.lock().expect("debug state poisoned");
        st.perf_enabled
    }

    pub fn perf_push(&self, ev: PerfTimelineEvent) {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        if st.perf_enabled {
            st.perf_timeline.push(ev);
        }
    }

    pub fn perf_take(&self) -> Vec<PerfTimelineEvent> {
        let mut st = self.inner.state.lock().expect("debug state poisoned");
        let out = st.perf_timeline.clone();
        st.perf_timeline.clear();
        out
    }

    pub fn breakpoint_errors(&self) -> Vec<(u32, String)> {
        let st = self.inner.state.lock().expect("debug state poisoned");
        st.breakpoints
            .iter()
            .filter_map(|b| b.last_error.clone().map(|e| (b.line, e)))
            .collect()
    }
}

impl DebugHandle {
    pub fn send(&self, cmd: DebugCommand) {
        let mut q = self.inner.queue.lock().expect("debug queue poisoned");
        q.push_back(cmd);
        drop(q);
        self.inner.cv.notify_all();
    }
}

struct DebugSessionInner {
    emit: DebugEmitFn,
    queue: Mutex<VecDeque<DebugCommand>>,
    state: Mutex<DebugState>,
    cv: Condvar,
}

impl DebugSessionInner {
    fn new(emit: DebugEmitFn) -> Self {
        Self {
            emit,
            queue: Mutex::new(VecDeque::new()),
            state: Mutex::new(DebugState::default()),
            cv: Condvar::new(),
        }
    }
}

#[derive(Clone)]
pub struct InternalWatch {
    pub src: String,
    pub expr: ParsedExpr,
}

impl InternalWatch {
    fn from_wire(w: DebugWatch) -> Self {
        let parsed = match aura_parse::parse_expr(&w.expr) {
            Ok(e) => ParsedExpr::Ok(e),
            Err(e) => ParsedExpr::Err(e.to_string()),
        };
        Self { src: w.expr, expr: parsed }
    }
}

#[derive(Clone)]
struct InternalBreakpoint {
    line: u32,
    condition: Option<ParsedExpr>,
    last_error: Option<String>,
}

impl InternalBreakpoint {
    fn from_wire(b: DebugBreakpoint) -> Self {
        let condition = b.condition.map(|s| match aura_parse::parse_expr(&s) {
            Ok(e) => ParsedExpr::Ok(e),
            Err(e) => ParsedExpr::Err(e.to_string()),
        });
        Self {
            line: b.line,
            condition,
            last_error: None,
        }
    }
}

#[derive(Clone)]
pub enum ParsedExpr {
    Ok(Expr),
    Err(String),
}

#[derive(Default)]
struct DebugState {
    enabled: bool,
    paused: bool,
    step_mode: bool,
    step_budget: u32,
    terminate_requested: bool,
    perf_enabled: bool,
    file: String,
    line_starts: Vec<usize>,
    breakpoints: Vec<InternalBreakpoint>,
    watches: Vec<InternalWatch>,
    perf_timeline: Vec<PerfTimelineEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_command_serde_roundtrip_terminate() {
        let json = r#"{"cmd":"terminate"}"#;
        let cmd: DebugCommand = serde_json::from_str(json).expect("parse DebugCommand");
        match cmd {
            DebugCommand::Terminate => {}
            other => panic!("expected Terminate, got {other:?}"),
        }
    }

    #[test]
    fn debug_event_serde_hello() {
        let ev = DebugEvent::Hello {
            protocol: DebugSession::PROTOCOL_VERSION,
            capabilities: vec!["native.launch".to_string(), "native.terminate".to_string()],
        };
        let s = serde_json::to_string(&ev).expect("serialize DebugEvent");
        assert!(s.contains("\"event\":\"hello\""));
        assert!(s.contains("\"protocol\""));
    }
}
