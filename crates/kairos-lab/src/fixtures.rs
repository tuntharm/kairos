use crate::{DatasetCaseV1, DatasetSplitV1, LabError};
use std::collections::{BTreeMap, BTreeSet};

const SYNTHETIC_FIXTURES: &str = include_str!("../fixtures/synthetic_cases.jsonl");

/// Loads the public, synthetic contract examples embedded in this crate.
pub fn synthetic_fixture_cases() -> Result<Vec<DatasetCaseV1>, LabError> {
    let mut cases = Vec::new();
    let mut ids = BTreeSet::new();
    let mut group_splits = BTreeMap::<_, DatasetSplitV1>::new();

    for line in SYNTHETIC_FIXTURES
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        if line.len() > 32 * 1024 {
            return Err(LabError::InputTooLarge);
        }
        let case: DatasetCaseV1 = serde_json::from_str(line)?;
        if case.schema_version != 1 || case.origin != crate::CaseOriginV1::Synthetic {
            return Err(LabError::InvalidContract(
                "invalid synthetic fixture identity",
            ));
        }
        if case.source_evidence_ids.is_empty()
            || !case
                .source_evidence_ids
                .iter()
                .all(|id| id.as_str().starts_with("synthetic-"))
        {
            return Err(LabError::InvalidContract(
                "unsafe synthetic evidence identity",
            ));
        }
        if !ids.insert(case.case_id.clone()) {
            return Err(LabError::InvalidContract("duplicate fixture case ID"));
        }
        if group_splits
            .insert(case.scenario_group.clone(), case.split)
            .is_some_and(|existing| existing != case.split)
        {
            return Err(LabError::InvalidContract("scenario group crosses splits"));
        }
        cases.push(case);
    }
    if cases.len() != 12 {
        return Err(LabError::InvalidContract(
            "fixture pack must contain exactly 12 cases",
        ));
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CaseOriginV1, DatasetSplitV1};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn twelve_synthetic_cases_are_safe_parseable_and_group_separated() {
        let cases = synthetic_fixture_cases().unwrap();
        assert_eq!(cases.len(), 12);

        let mut group_splits = BTreeMap::<String, BTreeSet<DatasetSplitV1>>::new();
        for case in &cases {
            assert_eq!(case.origin, CaseOriginV1::Synthetic);
            assert!(
                case.source_evidence_ids
                    .iter()
                    .all(|id| id.as_str().starts_with("synthetic-"))
            );
            group_splits
                .entry(case.scenario_group.as_str().to_owned())
                .or_default()
                .insert(case.split);
        }
        assert!(group_splits.values().all(|splits| splits.len() == 1));
        assert!(cases.iter().any(|case| case.split == DatasetSplitV1::Train));
        assert!(
            cases
                .iter()
                .any(|case| case.split == DatasetSplitV1::Validation)
        );
        assert!(cases.iter().any(|case| case.split == DatasetSplitV1::Test));

        let raw = include_str!("../fixtures/synthetic_cases.jsonl");
        for forbidden in [
            "/Users/",
            "90_Private",
            "agent_access",
            ".env",
            "BEGIN PRIVATE",
        ] {
            assert!(
                !raw.contains(forbidden),
                "fixture leaked forbidden marker {forbidden}"
            );
        }
    }
}
