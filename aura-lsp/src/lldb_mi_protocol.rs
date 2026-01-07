/// LLDB Machine Interface (MI) Compatibility Layer
/// 
/// Adapts LLDB's native MI protocol to match GDB MI for unified interface.
/// Handles protocol differences and normalizes output.

use crate::gdb_mi_protocol::{MICommand, MIResponse, MIResult, MIValue, GDBMIProtocol};
use std::collections::HashMap;

/// LLDB-specific MI protocol handler
pub struct LLDBMIProtocol {
    gdb_protocol: GDBMIProtocol,
}

/// LLDB command format (slightly different from GDB)
#[derive(Debug, Clone)]
pub struct LLDBCommand {
    command: String,
    args: Vec<String>,
}

/// LLDB to GDB MI translation mapping
pub struct MITranslator {
    command_map: HashMap<String, String>,
    option_map: HashMap<String, String>,
}

impl MITranslator {
    pub fn new() -> Self {
        let mut command_map = HashMap::new();
        let mut option_map = HashMap::new();

        // Map LLDB commands to GDB MI equivalents
        command_map.insert("process launch".to_string(), "exec-run".to_string());
        command_map.insert("process continue".to_string(), "exec-continue".to_string());
        command_map.insert("process interrupt".to_string(), "exec-interrupt".to_string());
        command_map.insert("process detach".to_string(), "target-detach".to_string());
        command_map.insert("process kill".to_string(), "target-detach".to_string());
        command_map.insert("frame select".to_string(), "stack-select-frame".to_string());
        command_map.insert("frame variable".to_string(), "stack-list-variables".to_string());
        command_map.insert("breakpoint list".to_string(), "break-list".to_string());
        command_map.insert("breakpoint set".to_string(), "break-insert".to_string());
        command_map.insert("breakpoint delete".to_string(), "break-delete".to_string());

        // Map LLDB options to GDB MI options
        option_map.insert("-n".to_string(), "-f".to_string()); // function name
        option_map.insert("-p".to_string(), "--source-and-assembly-mode".to_string());
        option_map.insert("-s".to_string(), "-s".to_string()); // source file

        MITranslator {
            command_map,
            option_map,
        }
    }

    /// Translate LLDB command to GDB MI command
    pub fn translate_command(&self, lldb_cmd: &str) -> Option<String> {
        // Try direct lookup
        if let Some(gdb_cmd) = self.command_map.get(lldb_cmd) {
            return Some(gdb_cmd.clone());
        }

        // Try partial match for compound commands
        for (lldb_pattern, gdb_equiv) in &self.command_map {
            if lldb_cmd.starts_with(lldb_pattern) {
                let remainder = &lldb_cmd[lldb_pattern.len()..];
                return Some(format!("{}{}", gdb_equiv, remainder));
            }
        }

        None
    }

    /// Translate LLDB option to GDB MI option
    pub fn translate_option(&self, lldb_opt: &str) -> String {
        self.option_map
            .get(lldb_opt)
            .cloned()
            .unwrap_or_else(|| lldb_opt.to_string())
    }

    /// Parse LLDB native output format and convert to GDB MI
    pub fn normalize_response(&self, lldb_output: &str) -> String {
        // LLDB outputs are typically in format:
        // (lldb) command
        // result...

        // Remove LLDB prompt if present
        let cleaned = if lldb_output.contains("(lldb)") {
            lldb_output.replace("(lldb)", "").trim().to_string()
        } else {
            lldb_output.to_string()
        };

        // Normalize common output patterns
        self.normalize_patterns(&cleaned)
    }

    fn normalize_patterns(&self, output: &str) -> String {
        let mut result = output.to_string();

        // Normalize frame output
        result = result.replace("frame #", "frame=");
        
        // Normalize variable output
        result = result.replace("(", "");
        result = result.replace(")", "");

        // Normalize breakpoint output
        result = result.replace("Breakpoint ", "bkpt,number=");

        result
    }
}

impl LLDBMIProtocol {
    pub fn new() -> Self {
        LLDBMIProtocol {
            gdb_protocol: GDBMIProtocol::new(),
        }
    }

    /// Convert LLDB command to GDB MI format
    pub fn lldb_to_gdb_mi(&self, lldb_cmd: &str) -> Result<String, String> {
        let translator = MITranslator::new();
        let token = self.gdb_protocol.next_token();

        if let Some(gdb_cmd) = translator.translate_command(lldb_cmd) {
            let mi_cmd = MICommand::new(token, &gdb_cmd);
            Ok(mi_cmd.build())
        } else {
            Err(format!("Unable to translate LLDB command: {}", lldb_cmd))
        }
    }

    /// Parse LLDB output and convert to GDB MI response
    pub fn parse_lldb_output(&self, output: &str, token: u32) -> Result<MIResponse, String> {
        let translator = MITranslator::new();
        let normalized = translator.normalize_response(output);

        // For now, create a simple response
        // In production, would parse LLDB output more thoroughly
        let response = MIResponse {
            token,
            result: MIResult::Done(MIValue::String(normalized)),
            async_output: Vec::new(),
        };

        Ok(response)
    }

    /// Check if LLDB supports a feature
    pub fn supports_feature(&self, feature: &str) -> bool {
        match feature {
            "mi" => true, // LLDB supports MI protocol
            "python" => true, // LLDB has Python scripting
            "variables" => true,
            "breakpoints" => true,
            "frames" => true,
            "threads" => true,
            "async-execution" => true,
            "non-stop-mode" => false, // LLDB doesn't support non-stop
            _ => false,
        }
    }

    /// Get LLDB version capabilities
    pub fn get_capabilities(&self) -> HashMap<String, String> {
        let mut caps = HashMap::new();
        caps.insert("name".to_string(), "lldb".to_string());
        caps.insert("version".to_string(), "13+".to_string());
        caps.insert("mi_version".to_string(), "2.0".to_string());
        caps.insert("supports_var_objects".to_string(), "true".to_string());
        caps.insert("supports_data_formatters".to_string(), "true".to_string());
        caps.insert("supports_python_scripting".to_string(), "true".to_string());
        caps
    }

    /// Create unified interface for both GDB and LLDB
    pub fn create_unified_command(&self, gdb_cmd: &str) -> MICommand {
        let token = self.gdb_protocol.next_token();
        MICommand::new(token, gdb_cmd)
    }
}

impl Default for LLDBMIProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_translation() {
        let translator = MITranslator::new();
        
        let gdb_cmd = translator.translate_command("process launch");
        assert_eq!(gdb_cmd, Some("exec-run".to_string()));
    }

    #[test]
    fn test_option_translation() {
        let translator = MITranslator::new();
        
        let gdb_opt = translator.translate_option("-n");
        assert_eq!(gdb_opt, "-f");
    }

    #[test]
    fn test_response_normalization() {
        let translator = MITranslator::new();
        let lldb_output = "(lldb) frame variable\n(int) x = 42";
        
        let normalized = translator.normalize_response(lldb_output);
        assert!(!normalized.contains("(lldb)"));
    }

    #[test]
    fn test_lldb_mi_protocol_creation() {
        let protocol = LLDBMIProtocol::new();
        assert!(protocol.supports_feature("mi"));
    }

    #[test]
    fn test_capabilities() {
        let protocol = LLDBMIProtocol::new();
        let caps = protocol.get_capabilities();
        
        assert_eq!(caps.get("name"), Some(&"lldb".to_string()));
        assert_eq!(caps.get("mi_version"), Some(&"2.0".to_string()));
    }

    #[test]
    fn test_lldb_to_gdb_mi() {
        let protocol = LLDBMIProtocol::new();
        let result = protocol.lldb_to_gdb_mi("process launch");
        
        assert!(result.is_ok());
        let mi_cmd = result.unwrap();
        assert!(mi_cmd.contains("exec-run"));
    }

    #[test]
    fn test_unified_command_interface() {
        let protocol = LLDBMIProtocol::new();
        let cmd = protocol.create_unified_command("exec-run");
        
        let built = cmd.build();
        assert!(built.contains("exec-run"));
    }
}
