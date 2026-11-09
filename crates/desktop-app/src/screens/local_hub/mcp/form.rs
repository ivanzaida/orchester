//! The fields of a local MCP server and what they need to be drawn and edited: the typed text, *Choose…* for the
//! program, the secret values typed so far and the one-line forms of the arguments and of the tools that run without
//! asking. A saved server's panel ([`super::editor`]) and the form that makes a new one
//! ([`crate::screens::team_hub`]'s New, in a local project) hold one each and draw its rows ([`super::rows`]).

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_workspace::{connections::mcp::McpServerText, messages::Message};

use super::{
  actions::{self, Lines},
  secrets::Asking,
};
use crate::screens::connections::mcp_form::fields::Fields;

pub(in crate::screens) struct Form {
  pub(in crate::screens) fields: Fields,
  /// Asks for the program's file; the field takes it.
  pub(super) choose: FutureAction<(), Option<String>, Message>,
  pub(in crate::screens) asking: Asking,
  /// The arguments in one line, as the panel draws them: split where there is a space.
  pub(super) arguments_line: Signal<String>,
  /// Whether an argument has a space in it, which one line cannot say: the arguments are one a line then.
  pub(super) one_line_arguments: bool,
  /// The tools that run without asking, in one line.
  pub(super) tools_line: Signal<String>,
}

impl Form {
  /// The fields holding `text`.
  pub(in crate::screens) fn new(ctx: &mut Ctx, text: &McpServerText) -> Self {
    let fields = Fields::new(ctx, text);
    let choose = actions::choose(ctx, fields.program.clone());
    let lines = Lines::new(ctx, &fields, text);
    Self {
      fields,
      choose,
      asking: Asking::new(ctx),
      arguments_line: lines.arguments,
      one_line_arguments: lines.one_line,
      tools_line: lines.tools,
    }
  }
}

/// The values typed for secrets are overwritten when the form goes: after a save, a cancel, or another row.
impl Drop for Form {
  fn drop(&mut self) {
    self.asking.wipe();
  }
}
