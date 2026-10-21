//! The project's tasks as screens and automation need them: the new task form, the task list and
//! its tabs, runs and their conversations, and reading and saving through the project database.
//! Task rules themselves are `orchester_core::task`; this module only maps between them and what is
//! on screen.

mod approval_choices;
pub mod approvals;
pub mod attention;
mod awaited;
pub mod board;
pub mod browse;
pub mod cancel_run;
pub mod changes;
pub mod checkout;
pub mod conflict;
pub mod conversation;
pub mod data;
pub mod delivery;
pub mod detail;
mod engines;
mod first_agent;
pub mod form;
pub mod group;
pub mod history;
pub mod hold;
pub mod index_use;
pub mod integration;
pub mod integration_jobs;
pub mod last_words;
pub mod list;
pub mod local_integration;
pub mod needs_you;
pub mod outside;
pub mod overview;
pub mod permissions;
pub mod plan;
pub mod plan_limit;
pub mod progress;
pub mod question;
pub mod raise;
pub mod ready_to_run;
mod requirements;
pub mod resolve;
pub mod return_work;
pub mod run;
pub mod run_again;
pub mod run_approvals;
pub(crate) mod run_times;
pub mod standing;
mod step_executors;
pub mod timeline;
pub mod transcript;
pub mod unreadable;
mod unverified;
pub mod usage;
pub mod writer;
