use crate::domain::JobState;

pub fn can_transition(from: JobState, to: JobState) -> bool {
    use JobState::{
        Canceled, Completed, Downloading, Failed, Interrupted, Paused, Queued, Resolving, Waiting,
    };

    matches!(
        (from, to),
        (
            Queued,
            Resolving | Waiting | Downloading | Paused | Canceled
        ) | (
            Resolving,
            Queued | Waiting | Downloading | Paused | Failed | Canceled | Interrupted
        ) | (
            Waiting,
            Queued | Resolving | Downloading | Paused | Failed | Canceled | Interrupted
        ) | (
            Downloading,
            Queued | Resolving | Waiting | Paused | Completed | Failed | Canceled | Interrupted
        ) | (Paused, Queued | Canceled)
            | (Interrupted, Queued | Canceled)
            | (Failed, Queued | Canceled)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiting_jobs_can_start_or_stop() {
        assert!(can_transition(JobState::Waiting, JobState::Downloading));
        assert!(can_transition(JobState::Waiting, JobState::Paused));
        assert!(can_transition(JobState::Waiting, JobState::Canceled));
    }
}
