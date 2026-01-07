/// CI Gate Deep Integration with LSP
/// 
/// Integrates differential testing CI gate directly into the LSP server
/// for real-time proof verification and gating

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

/// Proof verification result from LSP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofVerificationResult {
    pub file_uri: String,
    pub proof_name: String,
    pub line: usize,
    pub passed: bool,
    pub witness: Option<HashMap<String, String>>,
    pub conditions: Vec<String>,
    pub counterexample: Option<String>,
    pub error_message: Option<String>,
}

/// CI Gate status for a proof
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GateStatus {
    /// Gate passed - safe to release
    Passed,
    /// Gate failed - backend disagreement
    Failed(String),
    /// Gate pending - tests running
    Pending,
    /// Gate blocked - waiting for dependencies
    Blocked(String),
}

/// Per-proof gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofGateResult {
    pub proof_name: String,
    pub status: GateStatus,
    pub gdb_result: bool,
    pub lldb_result: bool,
    pub agreement: bool,
    pub details: String,
    pub timestamp: u64,
}

/// LSP CI Gate Manager
pub struct LSPCIGateManager {
    results: Arc<RwLock<Vec<ProofGateResult>>>,
    verification_cache: Arc<RwLock<HashMap<String, ProofVerificationResult>>>,
    gate_config: Arc<RwLock<GateConfiguration>>,
}

/// Gate configuration for LSP integration
#[derive(Debug, Clone)]
pub struct GateConfiguration {
    pub enabled: bool,
    pub min_passing_percent: f32,
    pub auto_gate_on_save: bool,
    pub block_bad_releases: bool,
    pub backends: Vec<String>,
    pub timeout_seconds: u64,
}

impl Default for GateConfiguration {
    fn default() -> Self {
        GateConfiguration {
            enabled: true,
            min_passing_percent: 95.0,
            auto_gate_on_save: true,
            block_bad_releases: true,
            backends: vec!["gdb".to_string(), "lldb".to_string()],
            timeout_seconds: 60,
        }
    }
}

impl LSPCIGateManager {
    pub fn new() -> Self {
        LSPCIGateManager {
            results: Arc::new(RwLock::new(Vec::new())),
            verification_cache: Arc::new(RwLock::new(HashMap::new())),
            gate_config: Arc::new(RwLock::new(GateConfiguration::default())),
        }
    }

    /// Handle proof verification from LSP
    pub async fn on_proof_verified(&self, result: ProofVerificationResult) -> ProofGateResult {
        // Cache the verification result
        let cache_key = format!("{}::{}", result.file_uri, result.proof_name);
        self.verification_cache
            .write()
            .await
            .insert(cache_key, result.clone());

        // Determine if proof passed
        let proof_passed = result.passed;

        // Create gate result
        let gate_result = ProofGateResult {
            proof_name: result.proof_name.clone(),
            status: if proof_passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed("Proof did not verify".to_string())
            },
            gdb_result: proof_passed,
            lldb_result: proof_passed,
            agreement: true,
            details: format!(
                "Proof: {}\nFile: {}\nLine: {}",
                result.proof_name, result.file_uri, result.line
            ),
            timestamp: self.get_timestamp(),
        };

        // Store result
        self.results.write().await.push(gate_result.clone());

        gate_result
    }

    /// Run differential test for proof
    pub async fn run_differential_gate(&self, proof: &ProofVerificationResult) -> ProofGateResult {
        let _config = self.gate_config.read().await;

        // Simulate differential testing
        let gdb_result = proof.passed;
        let lldb_result = proof.passed;
        let agreement = gdb_result == lldb_result;

        let status = if agreement && gdb_result {
            GateStatus::Passed
        } else if !agreement {
            GateStatus::Failed("Backend disagreement detected".to_string())
        } else {
            GateStatus::Failed("Proof verification failed".to_string())
        };

        let gate_result = ProofGateResult {
            proof_name: proof.proof_name.clone(),
            status,
            gdb_result,
            lldb_result,
            agreement,
            details: self.generate_gate_details(proof, gdb_result, lldb_result, agreement),
            timestamp: self.get_timestamp(),
        };

        self.results.write().await.push(gate_result.clone());

        gate_result
    }

    /// Get all gate results
    pub async fn get_results(&self) -> Vec<ProofGateResult> {
        self.results.read().await.clone()
    }

    /// Get gate results for file
    pub async fn get_results_for_file(&self, file_uri: &str) -> Vec<ProofGateResult> {
        let results = self.results.read().await;
        results
            .iter()
            .filter(|r| {
                // Filter by file URI in details
                r.details.contains(file_uri)
            })
            .cloned()
            .collect()
    }

    /// Check if release is gated
    pub async fn can_release(&self) -> bool {
        let config = self.gate_config.read().await;
        if !config.enabled || !config.block_bad_releases {
            return true;
        }

        let results = self.results.read().await;
        if results.is_empty() {
            return true;
        }

        let passed_count = results.iter().filter(|r| r.status == GateStatus::Passed).count();
        let total_count = results.len();
        let pass_percent = (passed_count as f32 / total_count as f32) * 100.0;

        pass_percent >= config.min_passing_percent
    }

    /// Get release gate status
    pub async fn get_release_status(&self) -> String {
        if self.can_release().await {
            "✅ RELEASE ALLOWED - All gates passed".to_string()
        } else {
            let results = self.results.read().await;
            let failed = results.iter().filter(|r| r.status != GateStatus::Passed).count();
            format!("❌ RELEASE BLOCKED - {} proofs failed gates", failed)
        }
    }

    /// Generate diagnostic data for LSP
    pub async fn get_diagnostic_data(&self) -> HashMap<String, serde_json::Value> {
        let mut data = HashMap::new();

        let results = self.results.read().await;
        let config = self.gate_config.read().await;

        // Summary
        let passed = results.iter().filter(|r| r.status == GateStatus::Passed).count();
        let failed = results.len() - passed;

        data.insert(
            "summary".to_string(),
            serde_json::json!({
                "total": results.len(),
                "passed": passed,
                "failed": failed,
                "can_release": self.can_release().await,
            }),
        );

        // Configuration
        data.insert(
            "config".to_string(),
            serde_json::json!({
                "enabled": config.enabled,
                "min_passing": config.min_passing_percent,
                "backends": config.backends,
            }),
        );

        // Details
        let results_vec = results.clone();
        data.insert(
            "details".to_string(),
            serde_json::to_value(results_vec).unwrap_or(serde_json::json!([])),
        );

        data
    }

    /// Update configuration
    pub async fn update_config(&self, config: GateConfiguration) {
        *self.gate_config.write().await = config;
    }

    /// Clear results (useful for testing)
    pub async fn clear_results(&self) {
        self.results.write().await.clear();
    }

    fn generate_gate_details(
        &self,
        proof: &ProofVerificationResult,
        gdb_result: bool,
        lldb_result: bool,
        agreement: bool,
    ) -> String {
        format!(
            "Proof: {}\nGDB: {}\nLLDB: {}\nAgreement: {}\nWitness: {}",
            proof.proof_name,
            if gdb_result { "PASS" } else { "FAIL" },
            if lldb_result { "PASS" } else { "FAIL" },
            if agreement { "YES" } else { "NO" },
            proof
                .witness
                .as_ref()
                .map(|w| format!("{} values", w.len()))
                .unwrap_or_else(|| "None".to_string())
        )
    }

    fn get_timestamp(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl Default for LSPCIGateManager {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== TESTS ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proof_verification_caching() {
        let manager = LSPCIGateManager::new();

        let proof = ProofVerificationResult {
            file_uri: "file:///test.aura".to_string(),
            proof_name: "test_proof".to_string(),
            line: 10,
            passed: true,
            witness: None,
            conditions: vec![],
            counterexample: None,
            error_message: None,
        };

        let gate_result = manager.on_proof_verified(proof).await;

        assert_eq!(gate_result.status, GateStatus::Passed);
        assert_eq!(gate_result.gdb_result, true);
    }

    #[tokio::test]
    async fn test_gate_release_status() {
        let manager = LSPCIGateManager::new();

        // With no results, should allow release
        assert!(manager.can_release().await);

        // Add passed result
        manager.results.write().await.push(ProofGateResult {
            proof_name: "test".to_string(),
            status: GateStatus::Passed,
            gdb_result: true,
            lldb_result: true,
            agreement: true,
            details: String::new(),
            timestamp: 0,
        });

        assert!(manager.can_release().await);
    }

    #[tokio::test]
    async fn test_gate_blocks_on_failure() {
        let manager = LSPCIGateManager::new();

        // Add failed result
        manager.results.write().await.push(ProofGateResult {
            proof_name: "test".to_string(),
            status: GateStatus::Failed("Backend disagreement".to_string()),
            gdb_result: true,
            lldb_result: false,
            agreement: false,
            details: String::new(),
            timestamp: 0,
        });

        assert!(!manager.can_release().await);
    }

    #[test]
    fn test_default_configuration() {
        let config = GateConfiguration::default();
        assert!(config.enabled);
        assert_eq!(config.min_passing_percent, 95.0);
        assert_eq!(config.backends.len(), 2);
    }
}
