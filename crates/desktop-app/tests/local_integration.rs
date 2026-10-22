//! Private behavioral UI regressions, not human consent outcomes or native Pencil verification.
#[path = "local_integration/abort.rs"]
mod abort;
#[path = "local_integration/acknowledgement.rs"]
mod acknowledgement;
#[path = "local_integration/candidates.rs"]
mod candidates;
#[path = "../../workspace/tests/local_integration/support.rs"]
mod domain_support;
#[path = "local_integration/hooks.rs"]
mod hooks;
#[path = "local_integration/support.rs"]
mod integration_support;
#[path = "local_integration/layout.rs"]
mod layout;
#[path = "local_integration/merged_outside.rs"]
mod merged_outside;
#[path = "local_integration/privacy.rs"]
mod privacy;
#[path = "local_integration/release.rs"]
mod release;
#[path = "local_integration/stale.rs"]
mod stale;
mod support;
#[path = "local_integration/worktree_removal.rs"]
mod worktree_removal;
