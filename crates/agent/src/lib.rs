//! Orchester's native agent outside core (`docs/architecture/native-agent.md` §2, §3, §4): the model
//! clients that speak a provider's wire format over HTTP and SSE, and the executors of the file and
//! search tools. Core decides what the loop does next ([`orchester_core::native`]); this crate does the
//! talking and the file work; the runner drives both. The desktop does not depend on it: API keys are
//! read in the runner only.

pub mod anthropic;
mod check;
pub mod endpoint;
pub mod files;
mod http;
mod retry;
pub mod sse;

pub use self::{
  anthropic::AnthropicClient,
  check::{ApiCheck, check_anthropic},
  endpoint::{Endpoint, Timing, WireQuirks},
  files::FileTools,
};
