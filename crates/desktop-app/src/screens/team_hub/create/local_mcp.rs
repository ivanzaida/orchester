//! The form of a new MCP server in a local project (`Agents · New MCP server` L4 `P7TFFJ`): every field is the
//! person's own, so it is the fields of a saved server's panel ([`crate::screens::local_hub::mcp`]) over an empty
//! server (how it runs, the program's path with *Choose…*, its arguments and folder, the tools it runs without asking,
//! the variables, the values of the secrets they take, and whether runs may use it), with its name over them. A local
//! MCP server has no versions: Create makes the server and the values go to the secret store with it.

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::connection::mcp::McpServerName;
use orchester_workspace::connections::{
  data::ProjectConnections,
  mcp::{McpServerErrors, McpServerText},
};

use super::{
  fields::{Body, Shown},
  work::{LocalMcp, Make},
};
use crate::screens::local_hub::mcp::{Form, Look, Mark, Taken, rows_of, switch_row};

pub(super) struct Fields {
  form: Form,
  /// Whether runs may use the server once it is made.
  on: Signal<bool>,
}

impl Fields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      form: Form::new(ctx, &McpServerText::new_server()),
      on: ctx.signal(true),
    }
  }

  pub(super) fn body(&self, ctx: &mut Ctx, shown: &Shown<'_>) -> Body {
    let typed = self.form.fields.text();
    let saved = McpServerText::new_server();
    let Some(listed) = shown.connections else {
      // The project's secrets and server names are read: the name can be typed meanwhile.
      let unread = ProjectConnections::default();
      let look = Look {
        saved: &saved,
        typed: &typed,
        listed: &unread,
        errors: &McpServerErrors::default(),
        tried: shown.tried,
        program: None,
        secrets: (&[], Mark::None),
        program_refused: false,
        creating: true,
      };
      return Body {
        lead: None,
        after: Vec::new(),
        rows: vec![self.form.name_row(ctx, &look, Some(shown.first))],
        computer: None,
        make: None,
        off: None,
        overlay: None,
      };
    };
    let names: Vec<McpServerName> = listed.mcp_names.iter().map(|(_, name)| name.clone()).collect();
    let checked = typed.draft(&names, &listed.secrets);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let taken: Vec<Taken> = checked.as_ref().map_or_else(
      |_| Vec::new(),
      |draft| {
        draft
          .secrets()
          .into_iter()
          .filter_map(|id| listed.secrets.iter().find(|secret| secret.id() == id))
          .map(|secret| Taken {
            id: secret.id(),
            name: secret.name().clone(),
            has_value: false,
          })
          .collect()
      },
    );
    let look = Look {
      saved: &saved,
      typed: &typed,
      listed,
      errors: &errors,
      tried: shown.tried,
      program: None,
      secrets: (&taken, Mark::None),
      program_refused: false,
      creating: true,
    };
    let mut rows = vec![self.form.name_row(ctx, &look, Some(shown.first))];
    rows.extend(self.form.server_rows(ctx, &look));
    rows.extend(rows_of(ctx, &self.form.asking, look.secrets));
    let on = self.on.clone();
    rows.push(switch_row(ctx, (self.on.get(), "local_new.mcp.onHint"), move |now| {
      on.set(now)
    }));
    Body {
      lead: None,
      after: Vec::new(),
      rows,
      computer: None,
      make: checked.ok().map(|draft| {
        Make::LocalMcpServer(Box::new(LocalMcp {
          draft,
          values: self.form.asking.pending.clone(),
          enabled: self.on.get(),
        }))
      }),
      off: None,
      overlay: self.form.asking.dialog(ctx, true),
    }
  }
}
