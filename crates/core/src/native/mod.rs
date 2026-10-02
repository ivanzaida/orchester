//! The native agent's domain (`docs/architecture/native-agent.md`): Orchester runs the coding-agent loop
//! itself against a model API. Core holds what is provider-neutral and needs no network or processes:
//! the conversation ([`item`]), the model boundary ([`model`]), the tools and their typed inputs
//! ([`tools`]), where file tools may read and write ([`scope`]), which commands run or ask
//! ([`command`]), and the loop that decides what happens next ([`session`]). The HTTP clients and tool
//! executors live in `orchester-agent`, the driver of the loop in the runner.

pub mod command;
pub mod item;
pub mod model;
pub mod scope;
pub mod session;
pub mod tools;
