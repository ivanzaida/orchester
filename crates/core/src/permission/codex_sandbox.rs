//! *Codex sandbox* (ORC-789): whether a Codex agent's commands run in Codex's own sandbox, which is what enforces its
//! repository access, network and writable folders ([`super::capability`]). Only a person turns it off: it widens
//! what agents may do ([`super::widening::Widening::CodexSandboxOff`]). Off, the runner starts and resumes the Codex
//! thread in `danger-full-access`, without the sandbox's launch settings (`crate::connection::invocation::codex`);
//! everything else of the launch stays. Other CLIs ignore it, and a shared project's profile runs with it on
//! whatever it says.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexSandbox {
  /// Commands run in `read-only` or `workspace-write`, as the repository access says.
  #[default]
  On,
  /// Commands run in `danger-full-access`: Codex enforces neither repository access, network nor writable folders.
  Off,
}

impl CodexSandbox {
  pub const fn is_on(&self) -> bool {
    matches!(self, Self::On)
  }
}
