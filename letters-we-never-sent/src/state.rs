//! State enum for letter triage.

use serde::{Deserialize, Serialize};

/// The triage state for a letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Awaiting curation decision (default).
    Pending,
    /// Curator accepted the letter for the annual binding.
    Accepted,
    /// Curator declined the letter.
    Declined,
    /// Marked for actual sending (rare; explicit user opt-in).
    SendReal,
}

impl State {
    /// Canonical string form (matches CLI flag values and frontmatter strings).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::SendReal => "send-real",
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

/// Filter applied to `list` (a superset of `State` plus `All`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateFilter {
    /// Show every letter regardless of state.
    All,
    /// Only the named state.
    Only(State),
}

impl StateFilter {
    /// Whether a given state passes this filter.
    #[must_use]
    pub fn matches(self, state: State) -> bool {
        match self {
            Self::All => true,
            Self::Only(s) => s == state,
        }
    }
}
