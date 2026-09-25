//! Words and icons for the states of a GitHub connection (`US3dx` in 04.05, the heads of
//! 04.C1–C5) and the ids of the command rows that fix them.

use orchester_core::connection::{GhAdvice, GitHubState};

use crate::ui::icon::Icon;

/// The status label of `state`: its icon and its key (all but one take no placeholders; the
/// missing scope names the scope as `scope`).
pub const fn status_label(state: &GitHubState) -> (Icon, &'static str) {
  match state {
    GitHubState::NotChecked => (Icon::Circle, "github.status.notChecked"),
    GitHubState::SignedIn { .. } => (Icon::CircleCheck, "github.status.signedIn"),
    GitHubState::NotSignedIn => (Icon::AlertCircle, "github.status.notSignedIn"),
    GitHubState::SignInUnknown => (Icon::CircleHelp, "github.status.signInUnknown"),
    GitHubState::WrongAccount { .. } => (Icon::AlertCircle, "github.status.wrongAccount"),
    GitHubState::MissingScope { .. } => (Icon::KeyRound, "github.status.missingScope"),
    GitHubState::ProgramMissing => (Icon::AlertCircle, "github.status.programMissing"),
    GitHubState::Unrecognized => (Icon::CircleHelp, "github.status.unrecognized"),
    GitHubState::CouldNotRun => (Icon::AlertCircle, "github.status.couldNotRun"),
  }
}

/// Ids of a command row and its Copy button.
pub const fn command_ids(advice: &GhAdvice) -> (&'static str, &'static str) {
  match advice {
    GhAdvice::Login => ("github-command-login", "github-command-login-copy"),
    GhAdvice::Switch(_) => ("github-command-switch", "github-command-switch-copy"),
    GhAdvice::Refresh => ("github-command-refresh", "github-command-refresh-copy"),
  }
}
