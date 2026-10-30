//! What a scenario asks of the QA app started on it. `start_qa_app` passes a scenario and no `-SignedIn`, and
//! `tools/qa-app.ps1` names no scenario, so the seed leaves a file beside the application database, and the QA app
//! (`examples/qa_app`, which includes this file) looks for it before its window opens. Fixture data for QA, never shipped.

/// It signs in to its stand-in cloud, as `--signed-in` does.
pub const SIGN_IN: &str = "qa-app-sign-in";
/// Signed in, it also joins a stand-in team server with a code, so the account has a server that answers.
pub const JOIN_TEAM: &str = "qa-app-join-team";
