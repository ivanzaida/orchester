//! Services and stacks (`docs/architecture/agent-testing.md`, decision 1) as automation reads and saves them in the
//! project database: what each is now ([`data`]), what a change to a service's program would let runs start
//! ([`widening`]), what Project settings › Services lists with the test suites ([`listing`]), a service's, a stack's
//! and a test suite's page there ([`editor`], [`stack_editor`], [`suite_editor`]), and the project's flaky tests with
//! their quarantine ([`flaky`]).

pub mod data;
pub mod editor;
pub mod flaky;
pub mod listing;
pub mod stack_editor;
pub mod suite_editor;
pub mod widening;
