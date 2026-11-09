//! The form of a new MCP server in a local project (`Agents · New MCP server` L4 `P7TFFJ`): every field is the
//! person's own, so it is the fields of a saved server's panel ([`crate::screens::local_hub::mcp`]) over an empty
//! server (how it runs, the program's path with *Choose…*, its arguments and folder, the tools it runs without asking,
//! the variables, the values of the secrets they take, and whether runs may use it), with its name over them. A local
//! MCP server has no versions: Create makes the server and the values go to the secret store with it.

use lurq::{app::ctx::Ctx, components::Row, core::Signal};
use orchester_core::connection::mcp::McpServerName;
use orchester_workspace::connections::mcp::McpServerText;

use super::{
  fields::{self, Body, Error, Look as FieldLook, Shown, Spec},
  work::{LocalMcp, Make},
};
use crate::screens::local_hub::mcp::{Form, Look, Mark, Taken, rows_of, switch_row};

const NAME: Spec = Spec::new("mcp-name", "team_new.mcp.name")
  .sub("local_new.mcp.nameSub")
  .required();

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
    let Some(listed) = shown.connections else {
      // The project's secrets and server names are read: the name can be typed meanwhile.
      return Body {
        rows: vec![self.name_row(ctx, shown, None)],
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
    let name_says = if typed.name.trim().is_empty() {
      shown.tried.then_some(Error::Required)
    } else {
      errors.name.clone().map(Error::Said)
    };
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
    let saved = McpServerText::new_server();
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
    let mut rows = vec![self.name_row(ctx, shown, name_says.as_ref())];
    rows.extend(self.form.server_rows(ctx, &look));
    rows.extend(rows_of(ctx, &self.form.asking, look.secrets));
    let on = self.on.clone();
    rows.push(switch_row(ctx, (self.on.get(), "local_new.mcp.onHint"), move |now| {
      on.set(now)
    }));
    Body {
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

  fn name_row(&self, ctx: &mut Ctx, shown: &Shown<'_>, error: Option<&Error>) -> Row {
    fields::name_row(
      ctx,
      shown,
      &NAME,
      error,
      (&self.form.fields.name, "team_new.mcp.namePlaceholder"),
      FieldLook::Plain,
    )
  }
}
