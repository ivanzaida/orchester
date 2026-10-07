//! The native agent's domain (`docs/architecture/native-agent.md`): Orchester runs the coding-agent loop
//! itself against a model API. Core holds what is provider-neutral and needs no network or processes:
//! the conversation ([`item`]), the model boundary ([`model`]), the tools and their typed inputs
//! ([`tools`]), how an edit changes a file's text ([`edit`]), where file tools may read and write ([`scope`]), which
//! commands run or ask ([`command`]), the loop that decides what happens next ([`session`]) and where its
//! conversation is kept ([`store`]). The HTTP clients and
//! tool executors live in `orchester-agent`, the driver of the loop in the runner.

pub mod command;
pub mod edit;
pub mod item;
pub mod model;
pub mod scope;
pub mod session;
pub mod store;
pub mod tools;
