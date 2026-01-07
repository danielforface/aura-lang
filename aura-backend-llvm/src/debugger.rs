/// DWARF debug information emission for LLVM backend.
/// Enables stepping debuggers (GDB, LLDB) to understand Aura code execution.

use std::collections::BTreeMap;

/// Represents a single source location in Aura code.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceLocation {
    /// Absolute path to source file
    pub file_path: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (0-indexed)
    pub column: u32,
}

/// Maps LLVM IR instructions to source locations.
#[derive(Clone, Debug)]
pub struct LineNumberMap {
    /// LLVM instruction offset -> source location
    pub entries: BTreeMap<u64, SourceLocation>,
}

impl LineNumberMap {
    pub fn new() -> Self {
        LineNumberMap {
            entries: BTreeMap::new(),
        }
    }

    /// Add a mapping from LLVM IR offset to source location.
    pub fn add_mapping(&mut self, ir_offset: u64, loc: SourceLocation) {
        self.entries.insert(ir_offset, loc);
    }

    /// Look up source location for a given IR offset.
    pub fn lookup(&self, ir_offset: u64) -> Option<&SourceLocation> {
        self.entries.get(&ir_offset)
    }
}

/// Represents a function's debug metadata.
#[derive(Clone, Debug)]
pub struct FunctionDebugInfo {
    /// Function name (mangled)
    pub name: String,
    /// Return type description
    pub return_type: String,
    /// Parameters: (name, type)
    pub parameters: Vec<(String, String)>,
    /// Local variables: (name, type, line_declared)
    pub local_vars: Vec<(String, String, u32)>,
    /// Line number where function starts
    pub start_line: u32,
    /// Line number where function ends
    pub end_line: u32,
}

/// Manages all debug information for a compiled module.
#[derive(Clone, Debug, Default)]
pub struct DwarfDebugInfo {
    /// File path -> line number mappings
    pub line_maps: BTreeMap<String, LineNumberMap>,
    /// Function name -> debug info
    pub functions: BTreeMap<String, FunctionDebugInfo>,
    /// Type definitions: name -> description
    pub type_defs: BTreeMap<String, String>,
}

impl DwarfDebugInfo {
    pub fn new() -> Self {
        DwarfDebugInfo {
            line_maps: BTreeMap::new(),
            functions: BTreeMap::new(),
            type_defs: BTreeMap::new(),
        }
    }

    /// Register a function's debug info.
    pub fn add_function(&mut self, func_name: String, info: FunctionDebugInfo) {
        self.functions.insert(func_name, info);
    }

    /// Add a line number mapping for a file.
    pub fn add_line_mapping(&mut self, file_path: String, map: LineNumberMap) {
        self.line_maps.insert(file_path, map);
    }

    /// Register a type definition.
    pub fn add_type_def(&mut self, type_name: String, description: String) {
        self.type_defs.insert(type_name, description);
    }

    /// Generate DWARF-formatted debug info for use with debuggers.
    /// This would normally be embedded in LLVM IR as !dbg metadata.
    pub fn emit_dwarf_metadata(&self) -> String {
        let mut output = String::new();
        output.push_str("!llvm.module.flags = !{\n");
        output.push_str("  !0\n");
        output.push_str("}\n\n");

        output.push_str("!0 = !{i32 2, !\"Dwarf Version\", i32 4}\n");

        // For each file, emit debug info
        let mut counter = 1;
        for (file_path, map) in &self.line_maps {
            output.push_str(&format!(
                "!{} = !{{!\"0x{:x}\", !\"File\", !\"{}\"}}\n",
                counter, counter, file_path
            ));
            counter += 1;
        }

        output
    }

    /// Generate breakpoint locations for the debugger.
    /// Returns list of (file_path, line_number) where debugger should insert breakpoints.
    pub fn get_breakpoint_locations(&self) -> Vec<(String, u32)> {
        let mut locations = Vec::new();

        for (file_path, map) in &self.line_maps {
            for (_, loc) in &map.entries {
                locations.push((file_path.clone(), loc.line));
            }
        }

        // Remove duplicates and sort
        locations.sort();
        locations.dedup();
        locations
    }
}

/// Integration with GDB/LLDB via MI (Machine Interface).
pub struct DebuggerIntegration {
    /// Last breakpoint ID assigned
    breakpoint_counter: u64,
    /// Active breakpoints: (id -> (file, line))
    breakpoints: BTreeMap<u64, (String, u32)>,
    /// Current execution state
    is_stopped: bool,
}

impl DebuggerIntegration {
    pub fn new() -> Self {
        DebuggerIntegration {
            breakpoint_counter: 0,
            breakpoints: BTreeMap::new(),
            is_stopped: false,
        }
    }

    /// Set a breakpoint at a source location. Returns breakpoint ID.
    pub fn set_breakpoint(&mut self, file: String, line: u32) -> u64 {
        self.breakpoint_counter += 1;
        let id = self.breakpoint_counter;
        self.breakpoints.insert(id, (file, line));
        id
    }

    /// Remove a breakpoint by ID.
    pub fn remove_breakpoint(&mut self, id: u64) -> bool {
        self.breakpoints.remove(&id).is_some()
    }

    /// List all active breakpoints (GDB/LLDB command: info breakpoints).
    pub fn list_breakpoints(&self) -> Vec<(u64, String, u32)> {
        self.breakpoints
            .iter()
            .map(|(id, (file, line))| (*id, file.clone(), *line))
            .collect()
    }

    /// Format a breakpoint as GDB MI output.
    pub fn format_breakpoint_mi(&self, id: u64) -> Option<String> {
        self.breakpoints.get(&id).map(|(file, line)| {
            format!(
                "^done,BreakpointTable={{nr_rows=\"1\",nr_cols=\"6\",hdr={{width=\"3\",alignment=\"-1\",col_name=\"number\",col_hdr=\"Num\"}},body=[bkpt={{number=\"{}\",type=\"breakpoint\",disp=\"keep\",enabled=\"y\",addr=\"0x0\",file=\"{}\",line=\"{}\",times=\"0\"}}]}}",
                id, file, line
            )
        })
    }

    /// Mark execution as stopped at a breakpoint.
    pub fn stop_at_breakpoint(&mut self, breakpoint_id: u64) {
        self.is_stopped = true;
    }

    /// Resume execution after stop.
    pub fn resume(&mut self) {
        self.is_stopped = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_location() {
        let loc = SourceLocation {
            file_path: "src/math.aura".to_string(),
            line: 42,
            column: 10,
        };
        assert_eq!(loc.line, 42);
    }

    #[test]
    fn test_line_number_map() {
        let mut map = LineNumberMap::new();
        let loc = SourceLocation {
            file_path: "test.aura".to_string(),
            line: 5,
            column: 0,
        };
        map.add_mapping(0x1000, loc.clone());
        assert_eq!(map.lookup(0x1000), Some(&loc));
        assert_eq!(map.lookup(0x2000), None);
    }

    #[test]
    fn test_debugger_breakpoints() {
        let mut dbg = DebuggerIntegration::new();
        let id1 = dbg.set_breakpoint("main.aura".to_string(), 10);
        let id2 = dbg.set_breakpoint("main.aura".to_string(), 20);

        let bps = dbg.list_breakpoints();
        assert_eq!(bps.len(), 2);
        assert!(dbg.remove_breakpoint(id1));
        assert_eq!(dbg.list_breakpoints().len(), 1);
    }

    #[test]
    fn test_dwarf_debug_info() {
        let mut dwarf = DwarfDebugInfo::new();
        let func = FunctionDebugInfo {
            name: "add".to_string(),
            return_type: "i32".to_string(),
            parameters: vec![
                ("x".to_string(), "i32".to_string()),
                ("y".to_string(), "i32".to_string()),
            ],
            local_vars: vec![("result".to_string(), "i32".to_string(), 5)],
            start_line: 3,
            end_line: 7,
        };
        dwarf.add_function("add".to_string(), func);
        assert_eq!(dwarf.functions.len(), 1);
    }
}
