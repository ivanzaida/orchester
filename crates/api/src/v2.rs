//! Additive workspace-authority contracts. Existing root contracts and the `/api/v1` client are unchanged.
//! Servers adopt these models on a separate versioned surface; publishing them does not expose HTTP routes.

mod invites;
mod membership;
mod projects;

pub use invites::*;
pub use membership::*;
pub use orchester_core::sharing::WorkspaceRole as MemberRole;
pub use projects::*;
