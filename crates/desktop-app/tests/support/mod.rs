//! The app's tests' window, data folder and stand-in pickers ([`orchester_app::test_support`], shared with the
//! end-to-end tests of the private platform repository), and what only the tests here use: link presses, profiles,
//! read counts and the real runner's programs.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

pub use orchester_app::test_support::*;

pub mod link_press;
pub mod profile;
pub mod reads;
#[path = "../../../workspace/tests/support/runner_programs.rs"]
pub mod runner_programs;
