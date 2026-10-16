//! What the app's MCP server answers while this computer is signed out of Orchester Cloud (the person's decision,
//! 2026-10-16): every tool refuses with `sign_in_required`, the app's, the domain's and lurq's window tools alike, so
//! nothing behind the sign-in gate leaks and nothing new starts. `sign_in_state` answers, and says so. A QA copy and a
//! development build also let lurq's looking tools answer ([`LOOKS_ON_GATE`]): the window shows only the gate then, and
//! the gate is verified against its design through them. The installed app never does. Signing in is never possible
//! over MCP: the window's *Sign in* refuses synthetic input.

use orchester_workspace::build::Build;

use super::WINDOW_TEXT;
use crate::mcp::tools::SIGN_IN_STATE;

/// The refusal's stable code.
pub const CODE: &str = "sign_in_required";

/// What a refused client reads.
pub const REFUSAL: &str = "Sign in required: Orchester is signed out of Orchester Cloud on this computer, so every \
  tool is refused until a person signs in, in the app's window. Signing in can't be done over MCP. sign_in_state \
  answers whether it is signed in.";

/// lurq's tools that look at the window and still answer on the gate in a QA copy or a development build.
pub const LOOKS_ON_GATE: &[&str] = &["lurq_windows"];

/// Whether lurq's looking tools ([`LOOKS_ON_GATE`] and the window's text tools) answer on the gate in `build`: in a QA
/// copy or a development build, never in the installed app.
pub const fn looks_on_gate(build: Build) -> bool {
  matches!(build, Build::Other)
}

/// Whether `tool` answers while signed out; `looks` in a QA copy or a development build.
pub fn answers(tool: &str, looks: bool) -> bool {
  tool == SIGN_IN_STATE || (looks && (WINDOW_TEXT.contains(&tool) || LOOKS_ON_GATE.contains(&tool)))
}
