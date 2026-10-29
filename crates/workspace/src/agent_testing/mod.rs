//! Services and stacks (`docs/architecture/agent-testing.md`, decision 1) as automation reads and saves them in the
//! project database: what each is now ([`data`]) and what a change to a service's program would let runs start
//! ([`widening`]).

pub mod data;
pub mod widening;
