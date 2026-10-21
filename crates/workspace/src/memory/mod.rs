//! Task and project memory as the Memory screens show and change it (`docs/design/orchester-workspace.md`, "Task
//! and project memory" and "Memory screens"): listing, filtering and grouping records (`listing`), who wrote them
//! and where (`origin`), what the next agent of a task gets (`next`), reading them for a screen (`read`), a
//! person's changes (`change`, made of a `draft`), and the memory settings (`settings`). The rules of records
//! themselves are core's ([`orchester_core::memory`]).

pub mod change;
pub mod draft;
pub mod handoff;
pub mod listing;
pub mod next;
pub mod origin;
pub mod read;
pub mod settings;
