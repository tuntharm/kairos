use crate::LabError;
use serde::{Deserialize, Serialize};

macro_rules! state_machine {
    ($name:ident { $($variant:ident),+ $(,)? }, |$from:ident, $to:ident| $allowed:expr) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn transition(&mut self, next: Self) -> Result<(), LabError> {
                let $from = *self;
                let $to = next;
                if $allowed {
                    *self = next;
                    Ok(())
                } else {
                    Err(LabError::InvalidTransition)
                }
            }
        }
    };
}

state_machine!(
    DatasetState {
        Draft,
        Reviewed,
        Frozen,
        Superseded
    },
    |from, to| matches!(
        (from, to),
        (DatasetState::Draft, DatasetState::Reviewed)
            | (DatasetState::Reviewed, DatasetState::Frozen)
            | (DatasetState::Frozen, DatasetState::Superseded)
    )
);

state_machine!(
    JobState {
        Queued,
        Preflight,
        Running,
        Validating,
        Succeeded,
        Failed,
        Cancelled,
        Interrupted,
    },
    |from, to| matches!(
        (from, to),
        (JobState::Queued, JobState::Preflight)
            | (JobState::Queued, JobState::Cancelled)
            | (JobState::Preflight, JobState::Running)
            | (
                JobState::Preflight,
                JobState::Failed | JobState::Cancelled | JobState::Interrupted
            )
            | (JobState::Running, JobState::Validating)
            | (
                JobState::Running,
                JobState::Failed | JobState::Cancelled | JobState::Interrupted
            )
            | (
                JobState::Validating,
                JobState::Succeeded
                    | JobState::Failed
                    | JobState::Cancelled
                    | JobState::Interrupted
            )
    )
);

state_machine!(
    CandidateState {
        Frozen,
        Evaluated,
        Eligible,
        NotEligible,
        Inconclusive,
        Proposed,
        Activated,
        Rejected,
    },
    |from, to| matches!(
        (from, to),
        (CandidateState::Frozen, CandidateState::Evaluated)
            | (
                CandidateState::Evaluated,
                CandidateState::Eligible
                    | CandidateState::NotEligible
                    | CandidateState::Inconclusive
            )
            | (CandidateState::Eligible, CandidateState::Proposed)
            | (
                CandidateState::NotEligible | CandidateState::Inconclusive,
                CandidateState::Rejected
            )
            | (
                CandidateState::Proposed,
                CandidateState::Activated | CandidateState::Rejected
            )
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_transition_chain_is_exact_and_invalid_changes_are_atomic() {
        let mut state = DatasetState::Draft;
        state.transition(DatasetState::Reviewed).unwrap();
        state.transition(DatasetState::Frozen).unwrap();
        let before = state;
        assert!(state.transition(DatasetState::Reviewed).is_err());
        assert_eq!(state, before);
        state.transition(DatasetState::Superseded).unwrap();
    }

    #[test]
    fn job_failures_are_terminal_and_success_requires_validation() {
        let mut job = JobState::Queued;
        job.transition(JobState::Preflight).unwrap();
        job.transition(JobState::Running).unwrap();
        assert!(job.transition(JobState::Succeeded).is_err());
        assert_eq!(job, JobState::Running);
        job.transition(JobState::Validating).unwrap();
        job.transition(JobState::Succeeded).unwrap();
        assert!(job.transition(JobState::Running).is_err());
    }

    #[test]
    fn only_eligible_candidates_can_be_proposed() {
        let mut eligible = CandidateState::Frozen;
        eligible.transition(CandidateState::Evaluated).unwrap();
        eligible.transition(CandidateState::Eligible).unwrap();
        eligible.transition(CandidateState::Proposed).unwrap();
        eligible.transition(CandidateState::Activated).unwrap();

        let mut rejected = CandidateState::Frozen;
        rejected.transition(CandidateState::Evaluated).unwrap();
        rejected.transition(CandidateState::NotEligible).unwrap();
        assert!(rejected.transition(CandidateState::Proposed).is_err());
        rejected.transition(CandidateState::Rejected).unwrap();
    }
}
