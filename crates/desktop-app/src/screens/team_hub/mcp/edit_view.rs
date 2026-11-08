//! What the panel of a team MCP server says while it is edited (`Agents · MCP server · admin · draft` A13 `WMtxm`,
//! `Publish refused · no longer an admin` A14 `gTLx6`, `Publish while offline` A16 `x3ywL`, `Publish refused · the
//! server rejects the content` A17 `rJbPm`): the draft's note, one notice for what the team server answered, the
//! team's part read-only where nothing can be changed, and the bar that publishes or discards.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};
use orchester_core::{resource::VersionNumber, time::Timestamp};
use orchester_workspace::{
  app_mcp::setup::{Header, NewTeamMcpServer, Runs},
  display,
  sharing::{team::TeamConfiguration, team_edit::EditRefused},
};

use crate::{
  screens::{
    hub::{
      draft_bar::{DraftBar, Standing},
      team::admins,
    },
    team_hub::{
      bar,
      model::Entry,
      rows::{self, Sub},
    },
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// How the draft stands on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kept {
  /// Saved, as of this time.
  At(Timestamp),
  /// The fields are the version's own: there is nothing to keep.
  Nothing,
  /// The fields do not make a server, so they are not kept yet.
  Not,
}

/// *Draft on v3 · saved 16:40 · only on this computer* over what it changes and what publishing makes of it (`Draft` in
/// A13).
pub(super) fn draft_note(ctx: &Ctx, number: VersionNumber, (kept, changed): (Kept, &[&'static str])) -> Element {
  let version = number.to_string();
  let status = match kept {
    Kept::At(at) => ctx.t_args(
      "team_hub.edit.draft.saved",
      [("version", version), ("time", display::clock(at))],
    ),
    Kept::Nothing => ctx.t_args("team_hub.edit.draft.nothingKept", [("version", version)]),
    Kept::Not => ctx.t_args("team_hub.edit.draft.unsaved", [("version", version)]),
  };
  let next = number.next().map_or_else(String::new, |next| next.to_string());
  let sub = if changed.is_empty() {
    ctx.t_args("team_hub.edit.draft.nothing", [("next", next)])
  } else {
    let fields: Vec<String> = changed.iter().map(|label| ctx.t(label).to_string()).collect();
    ctx.t_args(
      "team_hub.edit.draft.changes",
      [("fields", fields.join(", ")), ("next", next)],
    )
  };
  DraftBar::new("team-hub-edit-draft", Standing::Draft, status)
    .sub(sub)
    .into()
}

/// What the team server answered to *Publish for the team*, in one notice (A14, A17): the title says what was refused,
/// the words under it why and what to do, and the draft is kept.
pub(super) fn refusal(ctx: &Ctx, refused: &EditRefused, entry: &Entry) -> Element {
  let version = entry
    .number
    .and_then(VersionNumber::next)
    .map_or_else(String::new, |next| next.to_string());
  match refused {
    EditRefused::Refused(refusal) => Notice::new(
      "team-hub-edit-refused",
      NoticeTone::Danger,
      ctx.t_args(
        if refusal.is_local() {
          "team_hub.edit.rejectedHere.title"
        } else {
          "team_hub.edit.rejected.title"
        },
        [("version", version)],
      ),
    )
    .icon(Icon::CircleX)
    .body(ctx.t_args(
      "team_hub.edit.rejected.body",
      [("reason", message_text(ctx, &refusal.message()).to_string())],
    ))
    .stacked()
    .into(),
    EditRefused::Changed { latest } => Notice::new(
      "team-hub-edit-refused",
      NoticeTone::Danger,
      ctx.t_args("team_hub.edit.superseded.title", [("version", latest.to_string())]),
    )
    .icon(Icon::CircleX)
    .body(message_text(ctx, &refused.message()))
    .stacked()
    .into(),
    EditRefused::Failed(message) => Notice::new(
      "team-hub-edit-refused",
      NoticeTone::Danger,
      ctx.t_args("team_hub.edit.failed.title", [("version", version)]),
    )
    .icon(Icon::CircleX)
    .body(message_text(ctx, message))
    .stacked()
    .into(),
  }
}

/// *The team's server refused: you are no longer an admin*: the person who pressed Publish is not an admin any more,
/// and the page is read-only, with the draft kept here and `copy` to keep what was typed (A14). `by_server`: the server
/// said it; else this computer knows it from the last read of the members, and nothing was sent.
pub(super) fn not_admin(
  ctx: &Ctx,
  (name, team): (&str, &TeamConfiguration),
  (by_server, copy): (bool, Element),
) -> Element {
  Notice::new(
    "team-hub-edit-not-admin",
    NoticeTone::Danger,
    ctx.t(if by_server {
      "team_hub.edit.notAdmin.title"
    } else {
      "team_hub.edit.notAdminHere.title"
    }),
  )
  .icon(Icon::ShieldX)
  .body(ctx.t_args(
    "team_hub.edit.notAdmin.body",
    [("name", name.to_owned()), ("admins", admins(ctx, team))],
  ))
  .stacked()
  .action(copy)
  .into()
}

/// *Waiting to send · retries when the server is back*, and *Cancel sending* (A16): the server could not be reached, so
/// the version waits in the outbox and is sent once it answers. A new resource (`created`) has no version of the team's
/// to stay: it is created when the server answers, and cancelling takes it back as if never made.
pub(super) fn waiting(
  ctx: &Ctx,
  (number, created): (Option<VersionNumber>, Option<&str>),
  cancel: ActionButton,
) -> Element {
  let notice = Notice::new(
    "team-hub-edit-waiting",
    NoticeTone::Info,
    ctx.t("team_hub.edit.waiting.title"),
  )
  .icon(Icon::CloudUpload);
  let notice = match created {
    Some(name) => notice.body(ctx.t_args("team_hub.edit.waitingCreated.body", [("name", name)])),
    None => {
      let next = number
        .and_then(VersionNumber::next)
        .map_or_else(String::new, |next| next.to_string());
      let current = number.map_or_else(String::new, |number| number.to_string());
      notice.body(ctx.t_args("team_hub.edit.waiting.body", [("next", next), ("current", current)]))
    }
  };
  notice.stacked().action(cancel).into()
}

/// *Cancel sending* (A16): takes the version waiting in the outbox back as the draft; `press` is its action while it
/// can be pressed and `loading` while it is taken.
pub(super) fn cancel_sending(
  ctx: &Ctx,
  id: &str,
  (press, loading): (Option<Arc<dyn Fn() + Send + Sync>>, bool),
) -> ActionButton {
  let button = ActionButton::new(
    id.to_owned(),
    ButtonKind::Secondary,
    ctx.t("team_hub.edit.cancelSending"),
  )
  .icon(Icon::X);
  match (press, loading) {
    (_, true) => button.loading(),
    (Some(run), false) => button.on_click(move || run()),
    (None, false) => button.disabled(),
  }
}

/// The draft as text: one line per field, never a secret's value (the draft holds names only).
pub(super) fn changes_text(ctx: &Ctx, draft: &NewTeamMcpServer) -> String {
  let line = |key: &str, value: &str| ctx.t_args(key, [("value", value)]).to_string();
  let mut lines = vec![draft.name.clone()];
  match &draft.runs {
    Runs::Program {
      program,
      arguments,
      folder,
      variables,
    } => {
      lines.push(line("team_hub.edit.copy.program", program));
      lines.extend(
        arguments
          .iter()
          .map(|argument| line("team_hub.edit.copy.argument", argument)),
      );
      if !folder.is_empty() {
        lines.push(line("team_hub.edit.copy.folder", folder));
      }
      lines.extend(
        variables
          .iter()
          .map(|variable| line("team_hub.edit.copy.variable", variable)),
      );
    }
    Runs::Remote { url, header } => {
      lines.push(line("team_hub.edit.copy.url", url));
      if let Some(header) = header {
        lines.push(
          ctx
            .t_args(
              "team_hub.edit.copy.header",
              [("name", header.name.as_str()), ("secret", header.secret.as_str())],
            )
            .to_string(),
        );
      }
    }
  }
  if !draft.exempt_tools.is_empty() {
    lines.push(line("team_hub.edit.copy.exempt", &draft.exempt_tools.join(", ")));
  }
  lines.join("\n")
}

/// The pinned bar of an edit (`Actions` in A13): *Publish for the team* and *Discard draft*, and at its end who
/// publishes, or why the last press did not.
pub(super) struct EditBar<'a> {
  pub publish: ActionButton,
  pub discard: ActionButton,
  /// When the last press was refused: the hint says so in `danger`.
  pub refused_at: Option<Timestamp>,
  pub team: &'a TeamConfiguration,
}

impl EditBar<'_> {
  pub(super) fn render(self, ctx: &Ctx) -> Element {
    let hint = match self.refused_at {
      Some(at) => (
        ctx
          .t_args("team_hub.edit.bar.refused", [("time", display::clock(at))])
          .to_string(),
        PaletteColor::Danger,
      ),
      None => (
        ctx
          .t_args("team_hub.edit.bar.admins", [("admins", admins(ctx, self.team))])
          .to_string(),
        PaletteColor::TextMuted,
      ),
    };
    bar::bar_saying(vec![self.publish.into(), self.discard.into()], Some(hint))
  }
}

/// The bar of an edit that waits to be sent (A16): *Cancel sending* and the words that nothing is published yet.
pub(super) fn waiting_bar(ctx: &Ctx, cancel: ActionButton) -> Element {
  bar::bar(
    vec![cancel.into()],
    Some(ctx.t("team_hub.edit.bar.nothingPublished").to_string()),
  )
}

/// The bar of a draft that cannot be published by this person (A14): *Discard my draft*, and who to ask.
pub(super) fn read_only_bar(ctx: &Ctx, discard: ActionButton, team: &TeamConfiguration) -> Element {
  bar::bar(
    vec![discard.into()],
    Some(
      ctx
        .t_args("team_hub.edit.bar.askAdmin", [("admins", admins(ctx, team))])
        .to_string(),
    ),
  )
}

/// *Discard draft* / *Discard my draft* / *Cancel sending*-like secondary button of the bar.
pub(super) fn discard_button(ctx: &Ctx, id: &str, key: &str) -> ActionButton {
  ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t(key))
    .icon(Icon::Undo2)
    .pinned()
}

/// The rows of the team's part as values.
struct ValueRows<'a> {
  ctx: &'a Ctx,
  rows: Vec<Row>,
}

impl ValueRows<'_> {
  /// One row: `label` over *changed in your draft* when `changed`, and `value`.
  fn add(&mut self, id: &str, label: &str, (changed, value): (bool, String)) {
    let sub = changed.then(|| Sub {
      words: self.ctx.t("team_hub.edit.mark"),
      color: PaletteColor::Warning,
    });
    self.rows.push(rows::row(
      &format!("team-hub-edit-{id}"),
      &self.ctx.t(label),
      sub,
      rows::mono(&format!("team-hub-edit-{id}-value"), &value),
    ));
  }

  fn program(&mut self, (program, arguments, folder): ProgramOf<'_>, was: ProgramOf<'_>) {
    let ctx = self.ctx;
    self.add(
      "runs",
      "team_new.mcp.runsAs",
      (false, ctx.t("team_new.mcp.runsProgram").to_string()),
    );
    self.add("program", "team_new.mcp.program", (program != was.0, program.clone()));
    self.add(
      "arguments",
      "team_new.mcp.arguments",
      (arguments != was.1, arguments.join(" ")),
    );
    let folder_words = if folder.is_empty() {
      ctx.t("team_new.mcp.folderWorktree").to_string()
    } else {
      folder.clone()
    };
    self.add("folder", "team_new.mcp.folder", (folder != was.2, folder_words));
  }

  /// The variables the server needs, after the tools it may run without asking (A13).
  fn variables(&mut self, variables: &[String], was: &[String]) {
    let ctx = self.ctx;
    if !variables.is_empty() || !was.is_empty() {
      let lines: Vec<String> = variables
        .iter()
        .map(|name| {
          ctx
            .t_args("team_hub.mcp.fromTeam.variable", [("name", name.as_str())])
            .to_string()
        })
        .collect();
      self.add(
        "variables",
        "team_new.mcp.variables",
        (variables != was, lines.join(", ")),
      );
    }
  }

  fn remote(&mut self, (url, header): RemoteOf<'_>, was: RemoteOf<'_>) {
    self.add("url", "team_new.mcp.url", (url != was.0, url.clone()));
    if let Some(header) = header {
      self.add(
        "header",
        "team_new.mcp.header",
        (
          Some(header) != was.1.as_ref(),
          format!("{} <- {}", header.name, header.secret),
        ),
      );
    }
  }
}

type ProgramOf<'a> = (&'a String, &'a Vec<String>, &'a String);
type RemoteOf<'a> = (&'a String, &'a Option<Header>);

/// The team's part as values: the fields of `shown` where a form cannot be used (the draft of a person who is no longer
/// an admin, A14, and the version that waits to be sent, A16), each that differs from `published` saying so.
pub(super) fn read_only_rows(ctx: &Ctx, shown: &NewTeamMcpServer, published: &NewTeamMcpServer) -> Vec<Row> {
  let mut values = ValueRows { ctx, rows: Vec::new() };
  // The variables come after the tools it may run without asking, as the form has them (A13).
  let mut variables = None;
  match (&shown.runs, &published.runs) {
    (
      Runs::Program {
        program,
        arguments,
        folder,
        variables: now,
      },
      Runs::Program {
        program: was_program,
        arguments: was_arguments,
        folder: was_folder,
        variables: was,
      },
    ) => {
      values.program((program, arguments, folder), (was_program, was_arguments, was_folder));
      variables = Some((now, was));
    }
    (
      Runs::Remote { url, header },
      Runs::Remote {
        url: was_url,
        header: was_header,
      },
    ) => {
      values.remote((url, header), (was_url, was_header));
    }
    // Another way of running: the whole of it is the draft's.
    (Runs::Program { program, .. }, Runs::Remote { .. }) => {
      values.add("program", "team_new.mcp.program", (true, program.clone()));
    }
    (Runs::Remote { url, .. }, Runs::Program { .. }) => values.add("url", "team_new.mcp.url", (true, url.clone())),
  }
  if !shown.exempt_tools.is_empty() || !published.exempt_tools.is_empty() {
    values.add(
      "exempt",
      "team_new.mcp.exempt",
      (
        shown.exempt_tools != published.exempt_tools,
        shown.exempt_tools.join(", "),
      ),
    );
  }
  if let Some((now, was)) = variables {
    values.variables(now, was);
  }
  values.rows
}

/// *queued 13:52* at the chips' end of an edit that waits to be sent (A16).
pub(super) fn queued(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  ctx.t_args("team_hub.edit.queued", [("time", display::clock(at))])
}
