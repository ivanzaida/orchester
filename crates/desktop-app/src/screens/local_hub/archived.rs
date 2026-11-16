//! What archived looks like in the hub (`Agents · Archived` L13 `MXma9`, `Agents · Archived resource · Restore` L14
//! `KnPGN`): with nothing picked the panel says what archived means; an archived entry's panel says what archiving did
//! (the runs it stopped, which stay stopped) and what *Restore* does, and its one pinned action is *Restore*, a plain
//! action with no modal: restoring loses nothing and puts it back in the lists and pickers as it was.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{
  agents::local_hub::{LocalProfile, StoppedRun},
  connections::mcp::{McpServerText, TransportChoice},
  local_archive,
  messages::Message,
};

use super::{
  archived_profile,
  model::Entry,
  panel::{chips_of, frame},
  words::{archived_at, kind_where_key},
};
use crate::{
  screens::team_hub::{
    bar::bar,
    panel::{body, head_named},
    rows,
  },
  services::Services,
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The panel while nothing archived is picked (`Nothing picked` `gd374` in L13): a 20 glyph, the title and the line
/// under it, centered in the panel with no tile of their own. `id` names it, `<id>-title` and `<id>-line` its words,
/// whose keys are `title` and `detail`; a team project's hub says it in its own words (A36 `qvMZD`).
pub(in crate::screens) fn nothing_picked(ctx: &Ctx, id: &str, (title, detail): (&str, &str)) -> Element {
  Column::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(NOTHING_PICKED_SIDES, 0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::Archive, IconSize::Panel, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(title))
        .id(format!("{id}-title"))
        .variant(HubText::RowName.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Text::new(&ctx.t(detail))
        .id(format!("{id}-line"))
        .variant(HubText::ListNote.role())
        .width(Dimension::Pct(100.0))
        .text_align(TextAlign::Center),
    )
    .into()
}

/// The panel's room at each side of the explanation (`gd374`).
const NOTHING_PICKED_SIDES: f32 = 60.0;

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub root: std::path::PathBuf,
  pub project: ProjectId,
  pub entry: Entry,
  pub stopped: Vec<StoppedRun>,
  /// An MCP server's fields, shown read-only.
  pub details: Option<McpServerText>,
  /// A profile's values, shown read-only.
  pub profile: Option<Box<LocalProfile>>,
  pub now: Timestamp,
}
impl DevtoolsInspectable for Props {}

/// The panel of an archived entry, with *Restore*.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-restore-{:?}", props.entry.what);
  ctx.mount_keyed::<Restore>(&key, props)
}

struct Restore {
  restore: FutureAction<(), (), Message>,
}

impl Component for Restore {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    Self {
      restore: ctx.future_action(move |(): ()| {
        let (services, props) = (services.clone(), props.clone());
        async move {
          let what = props.entry.what;
          local_archive::restore(&props.root, what).await?;
          for change in what.changes(Some(props.entry.kind)) {
            services.changes.record(props.project, *change);
          }
          Ok(())
        }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let entry = &props.entry;
    let state = self.restore.state().get();
    let said = archived_at(ctx, props.now, entry.archived.unwrap_or(props.now));
    let body = body()
      .child(head_named(
        &entry.name,
        (ctx.t(kind_where_key(entry.kind)).to_string(), Some(said.to_string())),
        None,
      ))
      .child(chips_of(
        vec![(ctx.t("local_hub.chip.archived"), BadgeTone::Danger)],
        (!entry.used_by.is_empty())
          .then(|| ctx.t_args("local_hub.panel.wasUsedBy", [("names", entry.used_by.join(", "))])),
      ))
      .child(notice(ctx, entry, &props.stopped))
      .with_children(props.details.as_ref().map(|details| read_only(ctx, details)))
      .with_children(
        props
          .profile
          .as_ref()
          .map(|profile| archived_profile::read_only(ctx, profile)),
      )
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !state.is_pending())
          .map(|error| Text::new(&message_text(ctx, error)).variant(HubText::PanelHint.role())),
      );
    let action = self.restore.clone();
    let restore = ActionButton::new(
      "archive-restore",
      ButtonKind::Primary,
      ctx.t_args("local_hub.archived.restore", [("name", entry.name.as_str())]),
    )
    .icon(Icon::ArchiveRestore)
    .pinned();
    // Restored, the entry leaves *Archived* once the hub reads it again; until then Restore stays pending, as it was.
    let restore = if state.is_pending() || state.is_fulfilled() {
      restore.loading()
    } else {
      // A second press while the first restore is in flight leaves it to finish.
      restore.on_click(move || {
        action.run_if_idle(());
      })
    };
    frame(
      entry.what,
      body,
      Some(bar(
        vec![restore.into()],
        Some(ctx.t("local_hub.archived.restoreHint").to_string()),
      )),
    )
  }
}

/// What archiving did and what Restore does (`Notice` in L14).
fn notice(ctx: &Ctx, entry: &Entry, stopped: &[StoppedRun]) -> Element {
  let words = if stopped.is_empty() {
    ctx.t_args("local_hub.archived.stoppedNone", [("name", entry.name.as_str())])
  } else {
    let runs: Vec<String> = stopped
      .iter()
      .map(|run| {
        ctx
          .t_args(
            "local_hub.archived.run",
            [("key", run.key.as_str()), ("title", run.title.as_str())],
          )
          .to_string()
      })
      .collect();
    let named: Vec<&str> = runs.iter().map(String::as_str).collect();
    ctx.t_args(
      if stopped.len() == 1 {
        "local_hub.archived.stoppedOne"
      } else {
        "local_hub.archived.stopped"
      },
      [
        ("count", stopped.len().to_string()),
        (
          "runs",
          crate::screens::team_hub::archive::words::listed(ctx, &named).to_string(),
        ),
        ("name", entry.name.clone()),
      ],
    )
  };
  Notice::new(
    "local-hub-archived-notice",
    NoticeTone::Neutral,
    ctx.t("local_hub.archived.title"),
  )
  .icon(Icon::Archive)
  .title_style(HubText::NoticeTitle.role())
  .body(words)
  .flat()
  .into()
}

/// One row of the read-only table: its label and its value, in mono where it is a path or a name.
pub(super) fn read_row(id: &str, label: &str, value: &str, mono: bool) -> Row {
  let id = format!("local-hub-archived-{id}");
  let shown = format!("{id}-value");
  let value = if mono {
    rows::mono(&shown, value)
  } else {
    rows::words(&shown, value)
  };
  rows::row(&id, label, None, value)
}

/// What a program runs with: its path, arguments and folder; a remote server's address.
fn how_it_runs(ctx: &Ctx, text: &McpServerText) -> Vec<Row> {
  match text.transport {
    TransportChoice::Stdio => {
      let (folder, mono) = if text.working_directory.is_empty() {
        (ctx.t("connections.mcp.theWorktree").to_string(), false)
      } else {
        (text.working_directory.clone(), true)
      };
      vec![
        read_row(
          "program",
          &ctx.t("connections.mcp.property.program"),
          &text.program,
          true,
        ),
        read_row(
          "arguments",
          &ctx.t("connections.mcp.property.arguments"),
          &text.arguments.lines().collect::<Vec<_>>().join(" "),
          true,
        ),
        read_row("folder", &ctx.t("connections.mcp.property.folder"), &folder, mono),
      ]
    }
    TransportChoice::Http => vec![read_row(
      "url",
      &ctx.t("connections.mcp.property.endpoint"),
      &text.url,
      true,
    )],
  }
}

/// *Server · read-only until it is restored* over what the server was, as values (`Server` in L14).
fn read_only(ctx: &Ctx, text: &McpServerText) -> Column {
  let tools = text.exempt_tools.lines().collect::<Vec<_>>().join(", ");
  let tools = if tools.is_empty() {
    ctx.t("local_hub.profile.none").to_string()
  } else {
    tools
  };
  let runs_as = ctx.t(match text.transport {
    TransportChoice::Stdio => "local_hub.mcp.asProgram",
    TransportChoice::Http => "local_hub.mcp.asRemote",
  });
  let mut rows = vec![read_row("runs-as", &ctx.t("local_hub.mcp.runsAs"), &runs_as, false)];
  rows.extend(how_it_runs(ctx, text));
  rows.push(read_row("tools", &ctx.t("local_hub.mcp.tools"), &tools, true));
  if text.transport == TransportChoice::Stdio && !text.variables.is_empty() {
    rows.push(read_row(
      "variables",
      &ctx.t("connections.mcp.variables"),
      &super::mcp::variables::Notation::new(ctx).shown(&text.variables),
      true,
    ));
  }
  rows::section(
    "local-hub-archived-server",
    &ctx.t("connections.mcp.dialog.server"),
    vec![
      Text::new(&ctx.t("local_hub.archived.readOnly"))
        .id("local-hub-archived-read-only")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .into(),
    ],
    rows,
  )
}
