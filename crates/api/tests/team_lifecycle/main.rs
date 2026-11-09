//! Closing and reopening a team task (decision D4, `docs/cloud-api.md` "Team authority task lifecycle"): the rule and
//! the capability shapes ([`shapes`]), and the stand-in team server enforcing them ([`stand_in`], [`done_by_itself`],
//! on the harness in [`team`]).
//!
//! `cargo test -p orchester-api --features test-support --test team_lifecycle team_lifecycle_`

mod closed;
mod done_by_itself;
mod records;
mod shapes;
mod stand_in;
mod team;
