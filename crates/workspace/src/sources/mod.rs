//! Sources (`docs/plan.md`, M12; `orchester_core::source`) as the screens and automation read and change them:
//! folders and files outside the worktree that agents may read and search without asking, listed on the project
//! ([`project`]), a task ([`task`]), a pipeline and its steps (`crate::pipelines::sources`). Whether a path may become
//! one is [`check`]; how a listed one is on disk now is [`listed`].

pub mod check;
pub mod listed;
pub mod project;
pub mod task;
