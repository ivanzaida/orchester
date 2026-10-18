//! After a join (`After joining` and `Keys before joining` in `oOGMT`): *You joined KTR on Kontur team* with how many
//! tasks went up, how many were claimed and how many stay *Not shared*, the links the server dropped, *Open Tasks* and
//! *Dismiss*; it stays in Sharing until dismissed, after a restart too, kept in the application database's window
//! state. *Keys before joining* lists each local task's key then and now, and stays until *Make local*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_store::JoinedKey;
use orchester_workspace::workspace::Workspace;

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList},
  screens::project_settings::page,
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    form_section::section,
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What a join left to say, until the person dismisses it: the links the server dropped, as the keys before joining
/// of each task and the task it named.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
  pub dropped: Vec<(String, String)>,
}

impl lurq::app::component::DevtoolsInspectable for Note {}

/// The mapping's columns (`BEFORE`, `NOW` in `oOGMT`).
const BEFORE: f32 = 92.0;
const NOW: f32 = 102.0;

fn key(project: ProjectId) -> String {
  format!("orchester-app.sharing.joined.{project}")
}

/// Keeps what the join of `project` left to say; a failure to keep it only loses the callout, so it is logged.
pub fn keep(workspace: &Workspace, project: ProjectId, dropped: &[(String, String)]) {
  let text = serde_json::json!({ "dropped": dropped }).to_string();
  if let Err(error) = workspace.set_ui_state(&key(project), &text) {
    tracing::warn!(%error, "what the join did is not kept for Sharing");
  }
}

/// What the join of `project` left to say, until dismissed. Blocking.
pub fn read(workspace: &Workspace, project: ProjectId) -> Option<Note> {
  let text = match workspace.ui_state(&key(project)) {
    Ok(text) => text?,
    Err(error) => {
      tracing::warn!(%error, "what the join did can't be read");
      return None;
    }
  };
  let value: serde_json::Value = match serde_json::from_str(&text) {
    Ok(value) => value,
    // Only this app writes it; a note that can't be read is left out of Sharing rather than shown wrong.
    Err(error) => {
      tracing::warn!(%error, "what the join did can't be read: its note is corrupt");
      return None;
    }
  };
  let dropped = value
    .get("dropped")?
    .as_array()?
    .iter()
    .filter_map(|pair| {
      let pair = pair.as_array()?;
      Some((pair.first()?.as_str()?.to_owned(), pair.get(1)?.as_str()?.to_owned()))
    })
    .collect();
  Some(Note { dropped })
}

/// Puts the note of `project`'s join away. Blocking; a failure leaves it shown, and is logged.
pub fn dismiss(workspace: &Workspace, project: ProjectId) {
  if let Err(error) = workspace.remove_ui_state(&key(project)) {
    tracing::warn!(%error, "what the join did stays in Sharing");
  }
}

/// *You joined KTR on Kontur team*, with *Dismiss* and *Open Tasks*.
pub(in crate::screens::project_settings::sharing) fn callout(
  ctx: &mut Ctx,
  (project, key, server): (ProjectId, &str, &str),
  (note, joined): (&Note, &[JoinedKey]),
  dismiss: impl Fn() + Send + Sync + 'static,
) -> Element {
  let uploaded = joined.iter().filter(|task| !task.not_shared).count();
  let claimed = joined.iter().filter(|task| task.claimed_run.is_some()).count();
  let kept = joined.len() - uploaded;
  let mut body = ctx
    .t_args(
      "sharing.joined.detail",
      [
        ("uploaded", uploaded.to_string()),
        ("claimed", claimed.to_string()),
        ("kept", kept.to_string()),
        ("key", key.to_owned()),
      ],
    )
    .to_string();
  if !note.dropped.is_empty() {
    let links: Vec<String> = note
      .dropped
      .iter()
      .map(|(task, blocking)| {
        ctx
          .t_args(
            "sharing.joined.link",
            [("task", task.as_str()), ("blocking", blocking.as_str())],
          )
          .to_string()
      })
      .collect();
    let links = links.join(&ctx.t("sharing.joined.separator"));
    body = format!("{body} {}", ctx.t_args("sharing.joined.dropped", [("links", links)]));
  }
  let navigation = Navigation::of(ctx);
  // The neutral notice, its actions at the end (`YdKuq`), as Make local's outcome.
  Notice::new(
    "sharing-joined",
    NoticeTone::Info,
    ctx.t_args("sharing.joined.heading", [("key", key), ("server", server)]),
  )
  .icon(Icon::CircleCheck)
  .body(body)
  .action(
    ActionButton::new(
      "sharing-joined-dismiss",
      ButtonKind::Secondary,
      ctx.t("sharing.joined.dismiss"),
    )
    .on_click(dismiss),
  )
  .action(
    ActionButton::new(
      "sharing-joined-open",
      ButtonKind::Primary,
      ctx.t("sharing.joined.openTasks"),
    )
    .icon(Icon::ListChecks)
    .opens(navigation.to(Route::Project(project, ProjectPage::Tasks(TaskList::default())))),
  )
  .into()
}

/// *Keys before joining*: each local task's key then and now, its title, and whether it went up claimed or stays
/// *Not shared*.
pub(in crate::screens::project_settings::sharing) fn keys(ctx: &Ctx, joined: &[JoinedKey], key: &str) -> Element {
  let example = joined.iter().find(|task| !task.not_shared);
  let detail = match example {
    Some(task) => ctx.t_args(
      "sharing.mapping.detail",
      [
        ("key", key.to_owned()),
        ("before", task.former.clone()),
        ("now", task.key.to_string()),
      ],
    ),
    None => ctx.t("sharing.mapping.detailNone"),
  };
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(heading(&ctx.t("sharing.mapping.before"), Some(BEFORE)))
    .child(heading(&ctx.t("sharing.mapping.now"), Some(NOW)))
    .child(heading(&ctx.t("sharing.mapping.task"), None));
  let rows = joined.iter().map(|task| mapping_row(ctx, task));
  section(
    "sharing-keys",
    &ctx.t("sharing.mapping.heading"),
    None,
    vec![
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Md))
        .spacing(SpacingSize::Sm)
        .child(
          Text::new(&detail)
            .id("sharing-keys-detail")
            .variant(ExtraTypography::Prose.role())
            .width(Dimension::Pct(100.0)),
        )
        .child(
          Column::new()
            .width(Dimension::Pct(100.0))
            .child(header)
            .with_children(rows),
        )
        .child(page::wide_note(
          "sharing-keys-branches",
          Icon::GitBranch,
          &ctx.t("sharing.mapping.branches"),
        ))
        .into(),
    ],
  )
  .into()
}

fn heading(words: &str, width: Option<f32>) -> Text {
  let text = Text::new(words).variant(ExtraTypography::ColumnHeading.role()).nowrap();
  match width {
    Some(width) => text.width(Dimension::Px(width)),
    None => text.flex(1.0),
  }
}

/// `KON-12 · KTR-41 · Add retry to webhook sender · claimed for you · run #3`.
fn mapping_row(ctx: &Ctx, task: &JoinedKey) -> Row {
  let id = format!("sharing-keys-{}", task.former);
  let now = if task.not_shared {
    Text::new(&ctx.t("sharing.mapping.notShared"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
  } else {
    Text::new(&task.key.to_string())
      .variant(ExtraTypography::MonoKey.role())
      .color(PaletteColor::TextPrimary)
  };
  let note = if task.not_shared {
    Some(ctx.t("sharing.mapping.notSharedNote"))
  } else {
    task
      .claimed_run
      .map(|run| ctx.t_args("sharing.mapping.claimed", [("number", run.to_string())]))
  };
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&task.former)
        .id(format!("{id}-before"))
        .variant(ExtraTypography::MonoKey.role())
        .width(Dimension::Px(BEFORE)),
    )
    .child(now.id(format!("{id}-now")).width(Dimension::Px(NOW)))
    .child(
      Text::new(task.title.as_str())
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
    .with_children(note.map(|note| {
      Text::new(&note)
        .id(format!("{id}-note"))
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }))
}
