//! Orchester's native agent outside core (`docs/architecture/native-agent.md` §2, §3, §4): the model
//! clients that speak a provider's wire format over HTTP and SSE, and the executors of the file and
//! search tools. Core decides what the loop does next ([`orchester_core::native`]); this crate does the
//! talking and the file work; the runner drives both. The desktop does not depend on it: API keys are
//! read in the runner only.

#[cfg(feature = "http")]
pub mod anthropic;
#[cfg(feature = "http")]
mod check;
#[cfg(feature = "http")]
pub mod endpoint;
pub mod files;
#[cfg(feature = "http")]
mod http;
#[cfg(feature = "http")]
mod retry;
#[cfg(feature = "http")]
pub mod sse;

pub use self::files::FileTools;
#[cfg(feature = "http")]
pub use self::{
  anthropic::AnthropicClient,
  check::{ApiCheck, check_anthropic},
  endpoint::{Endpoint, Timing, WireQuirks},
};
