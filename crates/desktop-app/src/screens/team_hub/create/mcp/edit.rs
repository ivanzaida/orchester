//! The form of a new MCP server as the edit of an existing one (`Agents · MCP server · admin · draft` A13 `WMtxm`): its
//! fields start from the version this computer uses or from the draft kept here, the name is the resource's own and has
//! no row, and a field that differs from the version says *changed in your draft*.

use lurq::{app::ctx::Ctx, components::Row, core::ElementRef};
use orchester_core::connection::mcp::McpServerName;
use orchester_workspace::app_mcp::setup::{Header, NewTeamMcpServer, Runs, team_mcp_server};

use super::{Fields, INSIDE, Marks, PROGRAM_RUN, RUNS, RUNS_AS, WORKTREE, fields, lines_text};
use crate::screens::team_hub::{
  create::fields::Shown,
  model::{Kind, Listing},
};

/// What an edit's rows need besides the fields.
pub(in crate::screens::team_hub) struct EditView<'a> {
  pub listing: &'a Listing,
  /// The version this computer uses, which the fields are compared with.
  pub published: &'a NewTeamMcpServer,
  /// The fields are not changed: the edit is being published, or waits for the team server.
  pub locked: bool,
}

/// The rows of the edit and whether the fields make a server that can be published.
pub(in crate::screens::team_hub) struct Edited {
  pub rows: Vec<Row>,
  pub valid: bool,
  /// The labels (keys) of the fields that differ from the version, in the order of the rows.
  pub changed: Vec<&'static str>,
}

impl Fields {
  /// The fields of `input`.
  pub(in crate::screens::team_hub) fn of(ctx: &mut Ctx, input: &NewTeamMcpServer) -> Self {
    let fields = Self::new(ctx);
    fields.load(input);
    fields
  }

  /// Sets every field to what `input` says, as it stands in a version.
  pub(in crate::screens::team_hub) fn load(&self, input: &NewTeamMcpServer) {
    let fields = self;
    fields.name.set(input.name.clone());
    fields.exempt.set(input.exempt_tools.join(", "));
    match &input.runs {
      Runs::Program {
        program,
        arguments,
        folder,
        variables,
      } => {
        fields.runs.set(PROGRAM_RUN.to_owned());
        fields.program.set(program.clone());
        fields.arguments.set(lines_text(arguments));
        fields
          .folder_choice
          .set(if folder.is_empty() { WORKTREE } else { INSIDE }.to_owned());
        fields.folder.set(folder.clone());
        fields.variables.set(lines_text(variables));
      }
      Runs::Remote { url, header } => {
        fields.runs.set(RUNS[1].0.to_owned());
        fields.url.set(url.clone());
        let (name, secret, prefix) = header
          .as_ref()
          .map_or(("", "", ""), |Header { name, secret, prefix }| (name, secret, prefix));
        fields.header.set(name.to_owned());
        fields.secret.set(secret.to_owned());
        fields.prefix.set(prefix.to_owned());
      }
    }
  }

  /// The rows of the edit, with the form's own refusals said under their fields.
  pub(in crate::screens::team_hub) fn edit(&self, ctx: &mut Ctx, view: &EditView<'_>) -> Edited {
    let input = self.input();
    let others: Vec<McpServerName> = view
      .listing
      .shown(Kind::McpServer.filter())
      .filter(|entry| entry.name != input.name)
      .filter_map(|entry| McpServerName::new(&entry.name).ok())
      .collect();
    let checked = team_mcp_server(&input, &others);
    let invalid = checked.as_ref().err().map(|invalid| (**invalid).clone());
    let first = ElementRef::default();
    let shown = Shown {
      kind: Kind::McpServer,
      listing: view.listing,
      tried: true,
      locked: view.locked,
      agents: None,
      first: &first,
      refused_name: None,
      published: Some(view.published),
    };
    let marks = Marks::of(&input, shown.published);
    let program = matches!(input.runs, Runs::Program { .. });
    let mut rows = vec![fields::row(
      ctx,
      &RUNS_AS,
      marks.on(None, marks.runs).as_ref(),
      fields::choice(ctx, &RUNS_AS, &self.runs, &RUNS, view.locked),
    )];
    rows.extend(if program {
      self.program_rows(ctx, &shown, invalid.as_ref())
    } else {
      self.remote_rows(ctx, &shown, invalid.as_ref())
    });
    rows.push(self.exempt_row(ctx, &shown));
    if program {
      rows.push(self.variables_row(ctx, &shown));
    }
    Edited {
      rows,
      valid: checked.is_ok() && !self.folder_missing(),
      changed: marks.labels(),
    }
  }
}
