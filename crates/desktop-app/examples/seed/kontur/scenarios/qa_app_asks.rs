//! What a scenario asks of the QA app started on it. `start_qa_app` passes a scenario and no `-SignedIn`, and
//! `tools/qa-app.ps1` names no scenario, so the seed leaves a file beside the application database, and the QA app
//! (`examples/qa_app`, which includes this file) looks for it before its window opens. Fixture data for QA, never
//! shipped.

/// It signs in to its stand-in cloud, as `--signed-in` does.
pub const SIGN_IN: &str = "qa-app-sign-in";
/// Signed in, it also joins a stand-in team server with a code, so the account has a server that answers.
pub const JOIN_TEAM: &str = "qa-app-join-team";
/// That team server's project holds a task, so a checkout with tasks of its own joins through the review.
pub const TEAM_HOLDS_TASK: &str = "qa-app-team-holds-task";
/// Signed in, its session has no two-factor code, as when the account turned two-factor authentication on after it.
pub const SESSION_WITHOUT_TWO_FACTOR: &str = "qa-app-session-without-two-factor";
/// Its secret store refuses every value it is given, as one that is locked does: a QA copy's keychain is in memory, so
/// this is how a refusal that needs no person is reached (`Agents · MCP server · save refused`).
pub const SECRET_STORE_REFUSES: &str = "qa-app-secret-store-refuses";
