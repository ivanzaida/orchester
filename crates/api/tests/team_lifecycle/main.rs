//! Closing and reopening a team task (decision D4, `docs/cloud-api.md` "Team authority task lifecycle"): the rule and
//! the capability shapes ([`shapes`]), and the stand-in team server enforcing them ([`stand_in`]).
//!
//! `cargo test -p orchester-api --features test-support --test team_lifecycle team_lifecycle_`

mod shapes;
