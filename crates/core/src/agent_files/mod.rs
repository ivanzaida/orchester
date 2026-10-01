//! A repository's own agent files (ORC-15, `docs/design/orchester-workspace.md`, "Instructions"):
//! `AGENTS.md`, `.agents/skills` and `.codex/skills` for Codex, `CLAUDE.md` and `.claude/skills` for Claude Code.
//!
//! - Project settings › General › *Repository agent files* ([`RepositoryAgentFiles`]) decides whether the agent CLIs
//!   may load them besides Orchester's own instructions and skills. It is set per project, read when an agent starts (a
//!   change applies to agents that start after it), and on by default, which keeps what the CLIs did before the setting
//!   existed. Off, the CLIs do not load them; an agent can still open them as ordinary files of its worktree.
//! - Import from repository ([`import`]) turns each `AGENTS.md` / `CLAUDE.md` into a draft project instruction and each
//!   skill folder into a draft skill, and never writes to the repository.
//!
//! How each CLI is kept from loading them, checked 2026-09-29 on Windows 11 without a model turn, by rendering the
//! prompt (`codex debug prompt-input`) and by pointing each CLI at a local endpoint that recorded the request it would
//! have sent (`.tmp/orc15/` of the ORC-15 worktree):
//!
//! - Codex CLI 0.159.0 reads `AGENTS.md` into its first user message and lists the `SKILL.md` files under
//!   `<worktree>/.agents/skills` (at any depth) among its skills; a skill named in the prompt (`$name`) has its body
//!   added. `-c project_doc_max_bytes=0` leaves `AGENTS.md` out; each repository skill is left out, even when named, by
//!   disabling it by path (`-c skills.config=[{path="…/SKILL.md",enabled=false},…]`), which the runner does for the
//!   files it finds when the agent starts. Codex 0.159.2 also loads the skills of `<worktree>/.codex/skills`, even with
//!   the repository untrusted (2026-10-01), so those are disabled the same way. Codex needs both on a resumed session
//!   too: one without them read `AGENTS.md` and the skills again. `skills.include_instructions=false` only hides the
//!   list (a named skill still loaded) and is not used. Codex's own skills in `CODEX_HOME/skills/.system` stay
//!   available (they are the CLI's, not the repository's); `-c skills.bundled.enabled=false` would remove them. A Codex
//!   version without `skills.config` ignores the key: this was not checked against older versions.
//! - Claude Code 2.1.89 is always started with `--setting-sources ""` (see
//!   [`crate::connection::invocation::ClaudeInvocation`]), and with it the request held neither `CLAUDE.md` nor a skill
//!   of `.claude/skills`; with `--setting-sources project` it held both, also with `--disable-slash-commands`. So
//!   Claude Code loads none of them whatever the setting says: *on* does not make it load them, because project
//!   settings would also bring the repository's permission rules and hooks.
//!
//! Both CLIs can keep the files out, so no profile is refused for the setting; macOS was not checked.

pub mod frontmatter;
pub mod import;

use serde::{Deserialize, Serialize};

use crate::connection::AgentProvider;

/// Whether a project's agents may load the repository's own agent files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryAgentFiles {
  /// The CLIs load them as they did before the setting existed.
  #[default]
  On,
  /// Neither CLI loads them.
  Off,
}

/// What an agent CLI does with the repository's agent files under the setting, and how.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentFilesMechanism {
  /// Codex reads `AGENTS.md` and offers the skills of `.agents/skills`.
  CodexLoads,
  /// `-c project_doc_max_bytes=0` and each skill of `.agents/skills` disabled by path.
  CodexLeftOut,
  /// `--setting-sources ""`: Claude Code reads neither `CLAUDE.md` nor `.claude/skills`, in both states.
  ClaudeNoProjectSettings,
}

impl RepositoryAgentFiles {
  pub const fn is_on(self) -> bool {
    matches!(self, Self::On)
  }

  /// How `provider` treats the repository's agent files under this setting.
  pub const fn mechanism(self, provider: AgentProvider) -> AgentFilesMechanism {
    match (provider, self) {
      (AgentProvider::CodexCli, Self::On) => AgentFilesMechanism::CodexLoads,
      (AgentProvider::CodexCli, Self::Off) => AgentFilesMechanism::CodexLeftOut,
      (AgentProvider::ClaudeCodeCli, _) => AgentFilesMechanism::ClaudeNoProjectSettings,
    }
  }
}
