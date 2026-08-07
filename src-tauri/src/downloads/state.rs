use crate::domain::JobState;

pub fn can_transition(from: JobState, to: JobState) -> bool {
    use JobState::{
        Canceled, Completed, Downloading, Failed, Interrupted, Paused, Queued, Resolving,
    };

    matches!(
        (from, to),
        (Queued, Resolving | Paused | Canceled)
            | (
                Resolving,
                Downloading | Paused | Failed | Canceled | Interrupted
            )
            | (
                Downloading,
                Paused | Completed | Failed | Canceled | Interrupted
            )
            | (Paused, Queued | Canceled)
            | (Interrupted, Queued | Canceled)
            | (Failed, Queued | Canceled)
    )
}
