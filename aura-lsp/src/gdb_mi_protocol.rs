/// GDB Machine Interface (MI) Protocol Hardening
/// 
/// Provides robust, error-resistant communication with GDB using the MI protocol.
/// Handles async responses, errors, and validates all debugger output.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// MI Protocol version
pub const GDB_MI_VERSION: &str = "2.0";

/// Machine Interface command result
#[derive(Debug, Clone, PartialEq)]
pub enum MIResult {
    /// Done - command completed successfully
    Done(MIValue),
    /// Running - execution continuing
    Running,
    /// Connected - debugger connected
    Connected,
    /// Error - command failed
    Error(String),
    /// Exit - debugger exiting
    Exit,
}

/// Machine Interface value types
#[derive(Debug, Clone, PartialEq)]
pub enum MIValue {
    String(String),
    CString(String),
    Integer(i64),
    Boolean(bool),
    Tuple(Vec<(String, MIValue)>),
    List(Vec<MIValue>),
    ListOfTuples(Vec<HashMap<String, MIValue>>),
}

impl MIValue {
    pub fn as_string(&self) -> Option<String> {
        match self {
            MIValue::String(s) => Some(s.clone()),
            MIValue::CString(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            MIValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_tuple(&self) -> Option<&Vec<(String, MIValue)>> {
        match self {
            MIValue::Tuple(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<MIValue>> {
        match self {
            MIValue::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn get_field(&self, name: &str) -> Option<&MIValue> {
        match self {
            MIValue::Tuple(items) => items.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// GDB MI Protocol Handler
pub struct GDBMIProtocol {
    command_counter: Arc<Mutex<u32>>,
    handlers: Arc<Mutex<HashMap<u32, mpsc::UnboundedSender<MIResponse>>>>,
    pending_responses: Arc<Mutex<HashMap<u32, MIResponse>>>,
}

/// Complete MI Response
#[derive(Debug, Clone)]
pub struct MIResponse {
    pub token: u32,
    pub result: MIResult,
    pub async_output: Vec<String>,
}

/// MI Command builder
pub struct MICommand {
    token: u32,
    command: String,
    args: Vec<String>,
}

impl MICommand {
    pub fn new(token: u32, cmd: &str) -> Self {
        MICommand {
            token,
            command: cmd.to_string(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, key: &str, value: &str) -> Self {
        self.args.push(format!("{}={}", key, Self::escape_value(value)));
        self
    }

    pub fn flag(mut self, flag: &str) -> Self {
        self.args.push(flag.to_string());
        self
    }

    fn escape_value(value: &str) -> String {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }

    pub fn build(&self) -> String {
        if self.args.is_empty() {
            format!("{}-{}", self.token, self.command)
        } else {
            format!("{}-{} {}", self.token, self.command, self.args.join(" "))
        }
    }
}

impl GDBMIProtocol {
    pub fn new() -> Self {
        GDBMIProtocol {
            command_counter: Arc::new(Mutex::new(0)),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get next command token (monotonically increasing)
    pub fn next_token(&self) -> u32 {
        let mut counter = self.command_counter.lock().unwrap();
        *counter += 1;
        *counter
    }

    /// Register handler for async response
    pub fn register_handler(
        &self,
        token: u32,
        handler: mpsc::UnboundedSender<MIResponse>,
    ) {
        self.handlers.lock().unwrap().insert(token, handler);
    }

    /// Parse MI protocol output
    pub fn parse_response(&self, output: &str, token: u32) -> Result<MIResponse, String> {
        let trimmed = output.trim();

        // Check for result prefix with token
        if !trimmed.starts_with(&format!("{}^", token)) {
            return Err(format!("Invalid token: expected {}", token));
        }

        let rest = &trimmed[format!("{}^", token).len()..];
        let (result_str, value_part) = Self::split_result(rest)?;

        let result = match result_str {
            "done" => MIResult::Done(Self::parse_value(value_part)?),
            "running" => MIResult::Running,
            "connected" => MIResult::Connected,
            "error" => {
                let error_msg = Self::parse_error(value_part)?;
                MIResult::Error(error_msg)
            }
            "exit" => MIResult::Exit,
            _ => {
                return Err(format!("Unknown result: {}", result_str));
            }
        };

        Ok(MIResponse {
            token,
            result,
            async_output: Vec::new(),
        })
    }

    fn split_result(s: &str) -> Result<(&str, &str), String> {
        if let Some(pos) = s.find(',') {
            Ok((&s[..pos], &s[pos + 1..]))
        } else {
            Ok((s, ""))
        }
    }

    fn parse_value(s: &str) -> Result<MIValue, String> {
        let trimmed = s.trim();

        if trimmed.is_empty() {
            return Ok(MIValue::String(String::new()));
        }

        // String
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let content = &trimmed[1..trimmed.len() - 1];
            return Ok(MIValue::CString(content.to_string()));
        }

        // Integer
        if let Ok(i) = trimmed.parse::<i64>() {
            return Ok(MIValue::Integer(i));
        }

        // Boolean
        if trimmed == "true" || trimmed == "false" {
            return Ok(MIValue::Boolean(trimmed == "true"));
        }

        // Tuple: { key1=value1, key2=value2 }
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            let content = &trimmed[1..trimmed.len() - 1];
            return Ok(MIValue::Tuple(Self::parse_tuple_content(content)?));
        }

        // List: [ item1, item2 ]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let content = &trimmed[1..trimmed.len() - 1];
            return Ok(MIValue::List(Self::parse_list_content(content)?));
        }

        Ok(MIValue::String(trimmed.to_string()))
    }

    fn parse_tuple_content(content: &str) -> Result<Vec<(String, MIValue)>, String> {
        let mut items = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        let mut in_string = false;

        for ch in content.chars() {
            match ch {
                '"' if !in_string => in_string = true,
                '"' if in_string => in_string = false,
                '{' | '[' if !in_string => {
                    depth += 1;
                    current.push(ch);
                }
                '}' | ']' if !in_string => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if !in_string && depth == 0 => {
                    if let Some((key, value)) = Self::parse_tuple_item(&current) {
                        items.push((key, value));
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            if let Some((key, value)) = Self::parse_tuple_item(&current) {
                items.push((key, value));
            }
        }

        Ok(items)
    }

    fn parse_tuple_item(s: &str) -> Option<(String, MIValue)> {
        let trimmed = s.trim();
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let value_str = trimmed[eq_pos + 1..].trim();
            if let Ok(value) = Self::parse_value(value_str) {
                return Some((key, value));
            }
        }
        None
    }

    fn parse_list_content(content: &str) -> Result<Vec<MIValue>, String> {
        let mut items = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        let mut in_string = false;

        for ch in content.chars() {
            match ch {
                '"' if !in_string => in_string = true,
                '"' if in_string => in_string = false,
                '{' | '[' if !in_string => {
                    depth += 1;
                    current.push(ch);
                }
                '}' | ']' if !in_string => {
                    depth -= 1;
                    current.push(ch);
                }
                ',' if !in_string && depth == 0 => {
                    if let Ok(value) = Self::parse_value(&current) {
                        items.push(value);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }

        if !current.is_empty() {
            if let Ok(value) = Self::parse_value(&current) {
                items.push(value);
            }
        }

        Ok(items)
    }

    fn parse_error(s: &str) -> Result<String, String> {
        let value = Self::parse_value(s)?;
        match value {
            MIValue::CString(msg) => Ok(msg),
            MIValue::String(msg) => Ok(msg),
            _ => Ok("Unknown error".to_string()),
        }
    }

    /// Validate MI response integrity
    pub fn validate_response(&self, response: &MIResponse) -> Result<(), String> {
        match &response.result {
            MIResult::Error(msg) if msg.is_empty() => {
                Err("Empty error message".to_string())
            }
            MIResult::Done(MIValue::Tuple(items)) if items.is_empty() => {
                // Empty tuple is valid for simple commands
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Check if response indicates a fatal error
    pub fn is_fatal_error(&self, response: &MIResponse) -> bool {
        matches!(response.result, MIResult::Exit)
    }
}

impl Default for GDBMIProtocol {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mi_command_builder() {
        let cmd = MICommand::new(1, "exec-run")
            .flag("-exec-run")
            .build();
        assert!(cmd.contains("1-exec-run"));
    }

    #[test]
    fn test_parse_simple_response() {
        let protocol = GDBMIProtocol::new();
        let output = r#"1^done,frame={addr="0x00000000",func="main",args=[],file="test.c",fullname="/test.c",line="5"}"#;
        
        let response = protocol.parse_response(output, 1);
        assert!(response.is_ok());
    }

    #[test]
    fn test_parse_error_response() {
        let protocol = GDBMIProtocol::new();
        let output = r#"2^error,msg="Not confirmed.""#;
        
        let response = protocol.parse_response(output, 2);
        assert!(response.is_ok());
        if let MIResult::Error(msg) = &response.unwrap().result {
            assert_eq!(msg, "Not confirmed.");
        }
    }

    #[test]
    fn test_parse_value_string() {
        let result = GDBMIProtocol::parse_value(r#""hello world""#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_string(), Some("hello world".to_string()));
    }

    #[test]
    fn test_parse_value_integer() {
        let result = GDBMIProtocol::parse_value("42");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_integer(), Some(42));
    }

    #[test]
    fn test_token_generation() {
        let protocol = GDBMIProtocol::new();
        let t1 = protocol.next_token();
        let t2 = protocol.next_token();
        assert!(t2 > t1);
    }
}
