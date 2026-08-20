use crate::{DatasetState, ReadinessVerdictV1, Sha256Digest, StableId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const MINIMUM_WEIGHTED_SCORE: f64 = 0.85;
const REQUIRED_VALIDITY: f64 = 1.0;
const MINIMUM_ADAPTER_GAIN: f64 = 0.05;
const MAXIMUM_CRITICAL_FIX_REGRESSION: f64 = 0.02;
const SCORE_EPSILON: f64 = 1e-12;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCandidateKindV1 {
    NoTraining,
    Adapter,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationMetricsV1 {
    pub weighted_score: Option<f64>,
    pub schema_validity: Option<f64>,
    pub tool_validity: Option<f64>,
    pub fabricated_number_count: Option<u32>,
    pub fabricated_path_count: Option<u32>,
    pub fabricated_citation_count: Option<u32>,
    pub privacy_failure_count: Option<u32>,
    pub critical_calibration_failure_count: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriticalFixEvidenceV1 {
    pub case_id: StableId,
    pub failure_kind: StableId,
    pub before_failure_count: u32,
    pub after_failure_count: u32,
    pub before_output_sha256: Sha256Digest,
    pub after_output_sha256: Sha256Digest,
    pub adjudication_sha256: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationReportV1 {
    pub schema_version: u16,
    pub evaluation_id: StableId,
    pub specialist_id: StableId,
    pub candidate_kind: ReleaseCandidateKindV1,
    pub test_dataset_state: DatasetState,
    pub test_dataset_sha256: Sha256Digest,
    pub untouched_base_runtime_sha256: Sha256Digest,
    pub no_training_runtime_sha256: Sha256Digest,
    pub candidate_runtime_sha256: Sha256Digest,
    pub evaluated_boundary_sha256: Sha256Digest,
    pub untouched_base: EvaluationMetricsV1,
    pub no_training: EvaluationMetricsV1,
    pub candidate: EvaluationMetricsV1,
    pub documented_critical_fix: Option<CriticalFixEvidenceV1>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityDecisionV1 {
    pub verdict: ReadinessVerdictV1,
    pub reasons: Vec<String>,
}

/// Applies the frozen V0 release rules. Callers cannot weaken these thresholds.
pub fn assess_candidate_eligibility(report: &EvaluationReportV1) -> EligibilityDecisionV1 {
    let mut inconclusive = Vec::new();
    if report.schema_version != 1 {
        inconclusive.push("unsupported evaluation schema".to_owned());
    }
    if report.test_dataset_state != DatasetState::Frozen {
        inconclusive.push("test dataset is not frozen".to_owned());
    }
    if report.untouched_base_runtime_sha256 != report.no_training_runtime_sha256
        || report.no_training_runtime_sha256 != report.candidate_runtime_sha256
    {
        inconclusive.push("evaluated runtimes differ".to_owned());
    }
    if !metrics_complete_and_valid(&report.untouched_base)
        || !metrics_complete_and_valid(&report.no_training)
        || !metrics_complete_and_valid(&report.candidate)
    {
        inconclusive.push("required metrics are missing or malformed".to_owned());
    }
    if !inconclusive.is_empty() {
        return EligibilityDecisionV1 {
            verdict: ReadinessVerdictV1::Inconclusive,
            reasons: inconclusive,
        };
    }

    let candidate = &report.candidate;
    let mut failures = release_gate_failures(candidate);
    if report.candidate_kind == ReleaseCandidateKindV1::Adapter {
        let candidate_score = candidate.weighted_score.expect("completeness checked");
        let reference_score = report
            .no_training
            .weighted_score
            .expect("completeness checked");
        let has_material_gain =
            candidate_score + SCORE_EPSILON >= reference_score + MINIMUM_ADAPTER_GAIN;
        let has_documented_critical_fix = report
            .documented_critical_fix
            .as_ref()
            .is_some_and(valid_critical_fix)
            && candidate_score + SCORE_EPSILON >= reference_score - MAXIMUM_CRITICAL_FIX_REGRESSION;
        if !has_material_gain && !has_documented_critical_fix {
            failures.push(
                "adapter lacks a 0.05 gain or documented critical fix without material regression"
                    .to_owned(),
            );
        }
    }

    EligibilityDecisionV1 {
        verdict: if failures.is_empty() {
            ReadinessVerdictV1::Eligible
        } else {
            ReadinessVerdictV1::NotEligible
        },
        reasons: failures,
    }
}

fn valid_critical_fix(evidence: &CriticalFixEvidenceV1) -> bool {
    evidence.before_failure_count > 0
        && evidence.after_failure_count == 0
        && evidence.before_output_sha256 != evidence.after_output_sha256
}

fn release_gate_failures(metrics: &EvaluationMetricsV1) -> Vec<String> {
    let mut failures = Vec::new();
    if metrics.weighted_score.expect("completeness checked") + SCORE_EPSILON
        < MINIMUM_WEIGHTED_SCORE
    {
        failures.push("weighted score is below 0.85".to_owned());
    }
    if metrics.schema_validity.expect("completeness checked") != REQUIRED_VALIDITY {
        failures.push("schema validity is not 100 percent".to_owned());
    }
    if metrics.tool_validity.expect("completeness checked") != REQUIRED_VALIDITY {
        failures.push("tool validity is not 100 percent".to_owned());
    }
    for (name, count) in [
        ("fabricated numbers", metrics.fabricated_number_count),
        ("fabricated paths", metrics.fabricated_path_count),
        ("fabricated citations", metrics.fabricated_citation_count),
        ("privacy failures", metrics.privacy_failure_count),
        (
            "critical calibration failures",
            metrics.critical_calibration_failure_count,
        ),
    ] {
        if count.expect("completeness checked") != 0 {
            failures.push(format!("{name} must be zero"));
        }
    }
    failures
}

fn metrics_complete_and_valid(metrics: &EvaluationMetricsV1) -> bool {
    [
        metrics.weighted_score,
        metrics.schema_validity,
        metrics.tool_validity,
    ]
    .into_iter()
    .all(|value| value.is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value)))
        && [
            metrics.fabricated_number_count,
            metrics.fabricated_path_count,
            metrics.fabricated_citation_count,
            metrics.privacy_failure_count,
            metrics.critical_calibration_failure_count,
        ]
        .into_iter()
        .all(|value| value.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::parse(character.to_string().repeat(64)).unwrap()
    }

    fn complete_metrics(weighted_score: f64) -> EvaluationMetricsV1 {
        EvaluationMetricsV1 {
            weighted_score: Some(weighted_score),
            schema_validity: Some(1.0),
            tool_validity: Some(1.0),
            fabricated_number_count: Some(0),
            fabricated_path_count: Some(0),
            fabricated_citation_count: Some(0),
            privacy_failure_count: Some(0),
            critical_calibration_failure_count: Some(0),
        }
    }

    fn report(
        kind: ReleaseCandidateKindV1,
        reference_score: f64,
        candidate_score: f64,
    ) -> EvaluationReportV1 {
        EvaluationReportV1 {
            schema_version: 1,
            evaluation_id: StableId::parse("evaluation-001").unwrap(),
            specialist_id: StableId::parse("surrogate-experiment-reviewer").unwrap(),
            candidate_kind: kind,
            test_dataset_state: DatasetState::Frozen,
            test_dataset_sha256: digest('a'),
            untouched_base_runtime_sha256: digest('b'),
            no_training_runtime_sha256: digest('b'),
            candidate_runtime_sha256: digest('b'),
            evaluated_boundary_sha256: digest('c'),
            untouched_base: complete_metrics(0.70),
            no_training: complete_metrics(reference_score),
            candidate: complete_metrics(candidate_score),
            documented_critical_fix: None,
            created_at: "2026-08-20T00:00:00Z".parse().unwrap(),
        }
    }

    #[test]
    fn no_training_can_pass_the_absolute_frozen_gates() {
        let decision =
            assess_candidate_eligibility(&report(ReleaseCandidateKindV1::NoTraining, 0.86, 0.86));
        assert_eq!(decision.verdict, ReadinessVerdictV1::Eligible);
        assert!(decision.reasons.is_empty());
    }

    #[test]
    fn adapter_scoring_point_seven_nine_against_point_eight_is_not_eligible() {
        let decision =
            assess_candidate_eligibility(&report(ReleaseCandidateKindV1::Adapter, 0.80, 0.79));
        assert_eq!(decision.verdict, ReadinessVerdictV1::NotEligible);
    }

    #[test]
    fn adapter_needs_material_gain_or_a_documented_critical_fix() {
        let insufficient =
            assess_candidate_eligibility(&report(ReleaseCandidateKindV1::Adapter, 0.86, 0.88));
        assert_eq!(insufficient.verdict, ReadinessVerdictV1::NotEligible);

        let material_gain =
            assess_candidate_eligibility(&report(ReleaseCandidateKindV1::Adapter, 0.86, 0.91));
        assert_eq!(material_gain.verdict, ReadinessVerdictV1::Eligible);

        let mut critical_fix = report(ReleaseCandidateKindV1::Adapter, 0.87, 0.86);
        critical_fix.documented_critical_fix = Some(CriticalFixEvidenceV1 {
            case_id: StableId::parse("heldout-case-001").unwrap(),
            failure_kind: StableId::parse("critical-calibration").unwrap(),
            before_failure_count: 1,
            after_failure_count: 0,
            before_output_sha256: digest('d'),
            after_output_sha256: digest('e'),
            adjudication_sha256: digest('f'),
        });
        assert_eq!(
            assess_candidate_eligibility(&critical_fix).verdict,
            ReadinessVerdictV1::Eligible
        );
    }

    #[test]
    fn missing_is_inconclusive_but_measured_zero_score_is_a_real_failure() {
        let mut missing = report(ReleaseCandidateKindV1::NoTraining, 0.86, 0.86);
        missing.candidate.schema_validity = None;
        assert_eq!(
            assess_candidate_eligibility(&missing).verdict,
            ReadinessVerdictV1::Inconclusive
        );

        let zero = report(ReleaseCandidateKindV1::NoTraining, 0.86, 0.0);
        assert_eq!(
            assess_candidate_eligibility(&zero).verdict,
            ReadinessVerdictV1::NotEligible
        );
    }

    #[test]
    fn fabricated_or_private_output_and_runtime_mismatch_fail_closed() {
        let mut fabricated = report(ReleaseCandidateKindV1::NoTraining, 0.90, 0.90);
        fabricated.candidate.fabricated_path_count = Some(1);
        fabricated.candidate.privacy_failure_count = Some(1);
        assert_eq!(
            assess_candidate_eligibility(&fabricated).verdict,
            ReadinessVerdictV1::NotEligible
        );

        let mut mismatched = report(ReleaseCandidateKindV1::NoTraining, 0.90, 0.90);
        mismatched.candidate_runtime_sha256 = digest('d');
        assert_eq!(
            assess_candidate_eligibility(&mismatched).verdict,
            ReadinessVerdictV1::Inconclusive
        );
    }
}
