use lu_core::benchmark_contract::{
    validate_matched_pair, BenchmarkCondition, BenchmarkError, BenchmarkRun, FailureLayer,
    ModelTier, BENCHMARK_SCHEMA_VERSION,
};

fn baseline(tier: ModelTier) -> BenchmarkRun {
    BenchmarkRun {
        schema_version: BENCHMARK_SCHEMA_VERSION,
        run_id: "baseline-run-1".into(),
        pair_id: "pair-1".into(),
        suite_id: "tools-and-memory".into(),
        task_id: "read-file-1".into(),
        model_id: "qwen-model".into(),
        model_family: "Qwen".into(),
        model_sha256: "a".repeat(64),
        tier,
        quantization: "Q4_K_M".into(),
        kv_quantization: "Q8_0".into(),
        backend_id: "llama-cpu".into(),
        hardware_profile: "win64-reference".into(),
        seed: 42,
        context_tokens: 4096,
        output_token_limit: 1024,
        condition: BenchmarkCondition::Baseline,
        task_success: true,
        tool_calls_total: 2,
        tool_calls_correct: 2,
        latency_ms: 1500,
        peak_memory_bytes: Some(1_000_000_000),
        energy_millijoules: None,
        failure_layer: None,
    }
}
fn matched(tier: ModelTier) -> (BenchmarkRun, BenchmarkRun) {
    let before = baseline(tier);
    let mut after = before.clone();
    after.run_id = "augmented-run-1".into();
    after.condition = BenchmarkCondition::WithCognitum;
    after.latency_ms = 1800;
    after.task_success = false;
    after.tool_calls_correct = 1;
    after.failure_layer = Some(FailureLayer::CognitumBackend);
    (before, after)
}

#[test]
fn matched_before_after_comparisons_work_for_all_five_model_tiers() {
    for tier in [
        ModelTier::Sub1b,
        ModelTier::Approx1b,
        ModelTier::From7bTo14b,
        ModelTier::Approx16b,
        ModelTier::From25bTo35b,
    ] {
        let (before, after) = matched(tier);
        assert_eq!(validate_matched_pair(&before, &after), Ok(()));
        let json = serde_json::to_string(&after).unwrap();
        let restored: BenchmarkRun = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, after);
        assert_eq!(restored.energy_millijoules, None);
        assert_eq!(restored.failure_layer, Some(FailureLayer::CognitumBackend));
    }
}

#[test]
fn mixed_quantization_kv_seed_model_backend_and_hardware_are_not_comparable() {
    let (before, after) = matched(ModelTier::From25bTo35b);
    for changed in 0..7 {
        let mut other = after.clone();
        match changed {
            0 => other.quantization = "Q5_K_M".into(),
            1 => other.kv_quantization = "Q4_0".into(),
            2 => other.seed = 12,
            3 => other.model_id = "not-the-same-model".into(),
            4 => other.model_sha256 = "b".repeat(64),
            5 => other.backend_id = "cuda".into(),
            _ => other.hardware_profile = "different-gpu".into(),
        }
        assert_eq!(
            validate_matched_pair(&before, &other),
            Err(BenchmarkError::UnmatchedControls)
        );
    }
}

#[test]
fn forbids_invalid_accuracy_failure_layer_or_comparison_conditions() {
    let (mut before, mut after) = matched(ModelTier::Approx1b);
    before.tool_calls_correct = 3;
    assert_eq!(before.validate(), Err(BenchmarkError::InvalidToolAccuracy));
    before.tool_calls_correct = 2;
    before.failure_layer = Some(FailureLayer::CognitumBackend);
    assert_eq!(
        before.validate(),
        Err(BenchmarkError::InvalidFailureAttribution)
    );
    before.failure_layer = Some(FailureLayer::LuToolRouting);
    assert_eq!(before.validate(), Ok(()));
    after.condition = BenchmarkCondition::Baseline;
    // The baseline cannot also claim a Cognitum-specific backend failure.
    // Clear that field so this check isolates pair-condition rejection.
    after.failure_layer = None;
    assert_eq!(
        validate_matched_pair(&before, &after),
        Err(BenchmarkError::WrongPairConditions)
    );
}

#[test]
fn rejects_unsupported_schema_unknown_keys_and_incomplete_fingerprints() {
    let mut run = baseline(ModelTier::Sub1b);
    run.schema_version += 1;
    assert_eq!(run.validate(), Err(BenchmarkError::UnsupportedSchema));
    run.schema_version = BENCHMARK_SCHEMA_VERSION;
    run.model_sha256 = "abcdef".into();
    assert_eq!(run.validate(), Err(BenchmarkError::InvalidFingerprint));
    run.model_sha256 = "a".repeat(64);
    let mut value = serde_json::to_value(&run).unwrap();
    value["mystery_setting"] = serde_json::json!(true);
    assert!(serde_json::from_value::<BenchmarkRun>(value).is_err());
}
