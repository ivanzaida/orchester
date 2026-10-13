//! Private behavioral UI regressions, not human consent outcomes or native Pencil verification.
#[path = "local_integration/abort.rs"]
mod abort;
#[path = "local_integration/acknowledgement.rs"]
mod acknowledgement;
#[path = "local_integration/candidates.rs"]
mod candidates;
#[path = "../../workspace/tests/local_integration/support.rs"]
mod domain_support;
#[path = "local_integration/support.rs"]
mod integration_support;
#[path = "local_integration/privacy.rs"]
mod privacy;
#[path = "local_integration/release.rs"]
mod release;
mod support;
#[path = "local_integration/worktree_removal.rs"]
mod worktree_removal;
