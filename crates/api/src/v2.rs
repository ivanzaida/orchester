//! Additive workspace-authority contracts. Existing root contracts and the `/api/v1` client are unchanged.
//! Servers adopt these models on a separate versioned surface; publishing them does not expose HTTP routes.

mod authority_error;
mod invites;
mod membership;
mod operations;
mod policy;
mod projects;

pub use authority_error::*;
pub use invites::*;
pub use membership::*;
pub use operations::*;
pub use orchester_core::sharing::WorkspaceRole as MemberRole;
pub use policy::*;
pub use projects::*;
