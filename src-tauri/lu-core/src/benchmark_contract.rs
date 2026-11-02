//! Reproducible, model-independent benchmark result contract.
//! Run collection lives in the trusted benchmark host, not in the model.
//! This module neither loads models nor modifies the optional Cognitum CE.
use serde::{Deserialize, Serialize};

pub const BENCHMARK_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Sub1b,
    Approx1b,
    From7bTo14b,
    Approx16b,
    From25bTo35b,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkCondition {
    Baseline,
    WithCognitum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureLayer {
    LuToolRouting,
    Model,
    NativeTool,
    CognitumBackend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkRun {
    pub schema_version: u16,
    pub run_id: String,
    pub pair_id: String,
    pub suite_id: String,
    pub task_id: String,
    pub model_id: String,
    pub model_family: String,
    pub model_sha256: String,
    pub tier: ModelTier,
    pub quantization: String,
    pub kv_quantization: String,
    pub backend_id: String,
    pub hardware_profile: String,
    pub seed: u64,
    pub context_tokens: u32,
    pub output_token_limit: u32,
    pub condition: BenchmarkCondition,
    pub task_success: bool,
    pub tool_calls_total: u32,
    pub tool_calls_correct: u32,
    pub latency_ms: u64,
    /// Optional: never silently invent a missing resource or energy sample.
    pub peak_memory_bytes: Option<u64>,
    pub energy_millijoules: Option<u64>,
    pub failure_layer: Option<FailureLayer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchmarkError {
    InvalidRecord,
    UnsupportedSchema,
    InvalidFingerprint,
    InvalidToolAccuracy,
    InvalidFailureAttribution,
    WrongPairConditions,
    UnmatchedControls,
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128 && !id.contains('\0')
}

impl BenchmarkRun {
    pub fn validate(&self) -> Result<(), BenchmarkError> {
        if self.schema_version != BENCHMARK_SCHEMA_VERSION {
            return Err(BenchmarkError::UnsupportedSchema);
        }
        if ![
            &self.run_id,
            &self.pair_id,
            &self.suite_id,
            &self.task_id,
            &self.model_id,
            &self.model_family,
            &self.quantization,
            &self.kv_quantization,
            &self.backend_id,
            &self.hardware_profile,
        ]
        .into_iter()
        .all(|id| valid_id(id))
            || self.context_tokens == 0
            || self.output_token_limit == 0
            || self.latency_ms == 0
        {
            return Err(BenchmarkError::InvalidRecord);
        }
        if self.model_sha256.len() != 64
            || !self
                .model_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(BenchmarkError::InvalidFingerprint);
        }
        if self.tool_calls_correct > self.tool_calls_total {
            return Err(BenchmarkError::InvalidToolAccuracy);
        }
        if self.condition == BenchmarkCondition::Baseline
            && self.failure_layer == Some(FailureLayer::CognitumBackend)
        {
            return Err(BenchmarkError::InvalidFailureAttribution);
        }
        Ok(())
    }
}

/// Two records can be compared ONLY if all controlled model, task and runtime
/// settings match. Distinct quantization or KV cache controls are not pairs.
/// The metric values may differ; missing energy stays missing, never zero.
pub fn validate_matched_pair(
    baseline: &BenchmarkRun,
    augmented: &BenchmarkRun,
) -> Result<(), BenchmarkError> {
    baseline.validate()?;
    augmented.validate()?;
    if baseline.condition != BenchmarkCondition::Baseline
        || augmented.condition != BenchmarkCondition::WithCognitum
        || baseline.run_id == augmented.run_id
    {
        return Err(BenchmarkError::WrongPairConditions);
    }
    if baseline.pair_id != augmented.pair_id
        || baseline.suite_id != augmented.suite_id
        || baseline.task_id != augmented.task_id
        || baseline.model_id != augmented.model_id
        || baseline.model_family != augmented.model_family
        || baseline.model_sha256 != augmented.model_sha256
        || baseline.tier != augmented.tier
        || baseline.quantization != augmented.quantization
        || baseline.kv_quantization != augmented.kv_quantization
        || baseline.backend_id != augmented.backend_id
        || baseline.hardware_profile != augmented.hardware_profile
        || baseline.seed != augmented.seed
        || baseline.context_tokens != augmented.context_tokens
        || baseline.output_token_limit != augmented.output_token_limit
    {
        return Err(BenchmarkError::UnmatchedControls);
    }
    Ok(())
}
