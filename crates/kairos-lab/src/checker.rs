use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentityEvidenceV1 {
    pub declared: String,
    pub observed: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperimentMetadataV1 {
    pub objective: Option<String>,
    pub dataset: Option<IdentityEvidenceV1>,
    pub split: Option<IdentityEvidenceV1>,
    pub seeds: Vec<u64>,
    pub model: Option<IdentityEvidenceV1>,
    pub checkpoint: Option<IdentityEvidenceV1>,
    pub selection_policy: Option<String>,
    pub evaluation_horizon: Option<u64>,
    pub reported_evaluation_horizon: Option<u64>,
    pub metrics: Vec<String>,
    pub baselines: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperimentContractCheckV1 {
    pub present_fields: Vec<String>,
    pub missing_fields: Vec<String>,
    pub conflicting_fields: Vec<String>,
}

pub fn check_experiment_contract(input: &ExperimentMetadataV1) -> ExperimentContractCheckV1 {
    let mut present = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut conflicting = BTreeSet::new();

    check_text(
        "objective",
        input.objective.as_deref(),
        &mut present,
        &mut missing,
    );
    check_identity(
        "dataset",
        input.dataset.as_ref(),
        &mut present,
        &mut missing,
        &mut conflicting,
    );
    check_identity(
        "split",
        input.split.as_ref(),
        &mut present,
        &mut missing,
        &mut conflicting,
    );
    check_identity(
        "model",
        input.model.as_ref(),
        &mut present,
        &mut missing,
        &mut conflicting,
    );
    check_identity(
        "checkpoint",
        input.checkpoint.as_ref(),
        &mut present,
        &mut missing,
        &mut conflicting,
    );
    check_text(
        "selection_policy",
        input.selection_policy.as_deref(),
        &mut present,
        &mut missing,
    );
    check_list("metrics", &input.metrics, &mut present, &mut missing);
    check_list("baselines", &input.baselines, &mut present, &mut missing);
    check_list(
        "limitations",
        &input.limitations,
        &mut present,
        &mut missing,
    );

    if input.seeds.is_empty() {
        missing.insert("seeds".to_owned());
    } else {
        present.insert("seeds".to_owned());
        if input.seeds.iter().collect::<BTreeSet<_>>().len() != input.seeds.len() {
            conflicting.insert("seeds".to_owned());
        }
    }

    match input.evaluation_horizon {
        Some(horizon) if horizon > 0 => {
            present.insert("evaluation_horizon".to_owned());
            if input.reported_evaluation_horizon != Some(horizon) {
                conflicting.insert("evaluation_horizon".to_owned());
            }
        }
        _ => {
            missing.insert("evaluation_horizon".to_owned());
        }
    }

    ExperimentContractCheckV1 {
        present_fields: present.into_iter().collect(),
        missing_fields: missing.into_iter().collect(),
        conflicting_fields: conflicting.into_iter().collect(),
    }
}

fn check_text(
    field: &str,
    value: Option<&str>,
    present: &mut BTreeSet<String>,
    missing: &mut BTreeSet<String>,
) {
    if value.is_some_and(|value| !value.trim().is_empty()) {
        present.insert(field.to_owned());
    } else {
        missing.insert(field.to_owned());
    }
}

fn check_list(
    field: &str,
    values: &[String],
    present: &mut BTreeSet<String>,
    missing: &mut BTreeSet<String>,
) {
    if !values.is_empty() && values.iter().all(|value| !value.trim().is_empty()) {
        present.insert(field.to_owned());
    } else {
        missing.insert(field.to_owned());
    }
}

fn check_identity(
    field: &str,
    evidence: Option<&IdentityEvidenceV1>,
    present: &mut BTreeSet<String>,
    missing: &mut BTreeSet<String>,
    conflicting: &mut BTreeSet<String>,
) {
    let Some(evidence) = evidence else {
        missing.insert(field.to_owned());
        return;
    };
    if evidence.declared.trim().is_empty()
        || evidence
            .observed
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        missing.insert(field.to_owned());
        return;
    }
    present.insert(field.to_owned());
    if evidence.observed.as_deref() != Some(evidence.declared.as_str()) {
        conflicting.insert(field.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matching(value: &str) -> IdentityEvidenceV1 {
        IdentityEvidenceV1 {
            declared: value.to_owned(),
            observed: Some(value.to_owned()),
        }
    }

    #[test]
    fn checker_reports_missing_and_conflicting_contract_fields_deterministically() {
        let input = ExperimentMetadataV1 {
            objective: Some("Compare one controlled temporal stride".to_owned()),
            dataset: Some(matching("synthetic-dataset-v1")),
            split: Some(IdentityEvidenceV1 {
                declared: "split-a".to_owned(),
                observed: Some("split-b".to_owned()),
            }),
            seeds: vec![42, 42],
            model: Some(matching("synthetic-model")),
            checkpoint: None,
            selection_policy: None,
            evaluation_horizon: Some(100),
            reported_evaluation_horizon: Some(80),
            metrics: vec!["mae".to_owned()],
            baselines: Vec::new(),
            limitations: vec!["Synthetic evidence only".to_owned()],
        };

        let result = check_experiment_contract(&input);
        assert_eq!(
            result.missing_fields,
            vec!["baselines", "checkpoint", "selection_policy"]
        );
        assert_eq!(
            result.conflicting_fields,
            vec!["evaluation_horizon", "seeds", "split"]
        );
        assert!(result.present_fields.contains(&"objective".to_owned()));
    }

    #[test]
    fn checker_accepts_a_complete_consistent_contract() {
        let input = ExperimentMetadataV1 {
            objective: Some("Compare one controlled temporal stride".to_owned()),
            dataset: Some(matching("synthetic-dataset-v1")),
            split: Some(matching("grouped-split-v1")),
            seeds: vec![42],
            model: Some(matching("synthetic-model")),
            checkpoint: Some(matching("checkpoint-001")),
            selection_policy: Some("validation-only".to_owned()),
            evaluation_horizon: Some(100),
            reported_evaluation_horizon: Some(100),
            metrics: vec!["mae".to_owned()],
            baselines: vec!["untouched-base".to_owned()],
            limitations: vec!["Synthetic evidence only".to_owned()],
        };

        let result = check_experiment_contract(&input);
        assert!(result.missing_fields.is_empty());
        assert!(result.conflicting_fields.is_empty());
    }
}
