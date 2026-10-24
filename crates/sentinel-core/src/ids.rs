//! Typed ids. A newtype stops a task id from being passed where a run id belongs.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }
    };
}

string_id!(
    /// A backlog task, such as `T-04`.
    TaskId
);
string_id!(
    /// One worker run of one task.
    RunId
);
string_id!(
    /// A paired phone, tablet, or Telegram chat.
    DeviceId
);
string_id!(
    /// A worker tool, such as `claude` or `opencode`.
    ToolId
);
string_id!(
    /// One approval card.
    ApprovalId
);
string_id!(
    /// One decision prompt.
    DecisionId
);
string_id!(
    /// One Brain note.
    NoteId
);

impl RunId {
    /// A fresh random id.
    pub fn generate() -> Self {
        Self::new(uuid::Uuid::new_v4().to_string())
    }
}

/// Row id of one reservation in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReservationId(i64);

impl ReservationId {
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    pub fn get(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_round_trips_as_a_plain_string() {
        let id = TaskId::new("T-04");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"T-04\"");
        assert_eq!(serde_json::from_str::<TaskId>(&json).unwrap(), id);
    }

    #[test]
    fn generated_run_ids_differ() {
        assert_ne!(RunId::generate(), RunId::generate());
    }

    #[test]
    fn display_shows_the_inner_value() {
        assert_eq!(ToolId::from("claude").to_string(), "claude");
    }
}
