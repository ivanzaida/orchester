//! A stand-in for Orchester Cloud on loopback and the tests' one cloud key, for the tests and QA builds of every crate
//! that talks to the cloud: the desktop app's and the workspace's here, and the end-to-end tests of the private
//! platform repository, which build this crate by git. Never part of a shipped program: only the `test-support`
//! feature builds it.

pub mod account_cloud;
pub mod cloud_keys;
