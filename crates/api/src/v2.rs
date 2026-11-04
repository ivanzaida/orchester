//! Additive workspace-authority contracts. Existing root contracts and the `/api/v1` client are unchanged.
//! Servers adopt these models on a separate versioned surface; publishing them does not expose HTTP routes.

mod authority_error;
mod bundle;
mod bundle_wire;
mod capabilities;
mod commands;
mod delivery;
mod execution;
mod invites;
mod library;
mod membership;
mod module_read;
mod module_summary;
mod operations;
mod page;
mod policy;
mod projects;
pub mod subscription;
mod task_list;
mod task_read;
mod work_report;

pub use authority_error::*;
pub use bundle::*;
pub use capabilities::*;
pub use commands::*;
pub use delivery::*;
pub use execution::*;
pub use invites::*;
pub use library::*;
pub use membership::*;
pub use module_read::*;
pub use module_summary::*;
pub use operations::*;
pub use orchester_core::sharing::WorkspaceRole as MemberRole;
pub use page::{
  DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT, PAGE_CURSOR_RETENTION_SECONDS, Page, PageCursor, PageCursorError, PageRequest,
  PageRequestError,
};
pub use policy::*;
pub use projects::*;
pub use subscription::*;
pub use task_list::*;
pub use task_read::*;
pub use work_report::*;
