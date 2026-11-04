//! Services and stacks (`docs/architecture/agent-testing.md`, decision 1) as automation reads and saves them in the
//! project database: what each is now ([`data`]), what a change to a service's program would let runs start
//! ([`widening`]), and what Project settings › Services lists with the test suites ([`listing`]).

pub mod data;
pub mod listing;
pub mod widening;
