//! Services and stacks (`docs/architecture/agent-testing.md`, decision 1) as automation reads and saves them in the
//! project database: what each is now ([`data`]), what a change to a service's program would let runs start
//! ([`widening`]), what Project settings › Services lists with the test suites ([`listing`]), a service's, a stack's
//! and a test suite's page there ([`editor`], [`stack_editor`], [`suite_editor`]), the project's flaky tests with
//! their quarantine ([`flaky`]), and a first draft of services and stacks read from the checkout ([`import`]); and
//! whether the native UI driver can work on this computer, with the macOS permissions it misses ([`native_driver`]).

pub mod data;
pub mod editor;
pub mod flaky;
pub mod import;
pub mod listing;
pub mod native_driver;
pub mod stack_editor;
pub mod suite_editor;
pub mod widening;
