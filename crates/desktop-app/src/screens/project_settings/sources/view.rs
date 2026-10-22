//! The parts of Project settings › Sources: the section's heading and words (`Section · Sources` `oCZK1`), its list
//! (`Source list` `OL8b1`) or that it is empty (`Empty` `VXZ0x`), a missing source's actions (`Actions` `j5K80`),
//! the notice of a removed one (`Notice · Removed` `UWffE`), *Add source* (`PDego`), what agents may do outside the
//! worktree (`jWGJR`) and when changes apply (`qaIE2`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Row, Text},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::source::SourcePath;
use orchester_workspace::sources::{
  check::Kind,
  listed::{Listed, State},
  project::Removed,
};

use super::{Read, TabProps};
use crate::{
  theme::{ExtraTypography, SourcesSpacing},
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    button::{ActionButton, ButtonKind},
    enforcement_row::{Held, enforcement_row},
    form_section::section,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    source_row::SourceRow,
  },
};

type Remove = Arc<dyn Fn(SourcePath) + Send + Sync>;
type CheckAgain = Arc<dyn Fn() + Send + Sync>;

/// The width of an empty list's words (`Text` `CooJx`).
const EMPTY_TEXT_WIDTH: f32 = 520.0;

/// The section's heading, with how many sources there are and how many runs cannot read, and its words.
pub(super) fn heading(ctx: &Ctx, props: &TabProps, read: &Read) -> Column {
  let missing = read
    .listed
    .iter()
    .filter(|listed| listed.state != State::Available)
    .count();
  let count = read.listed.len().to_string();
  let note = match (read.listed.len(), missing) {
    (0, _) => ctx.t("project_settings.sourcesTab.none"),
    (_, 0) => ctx.t_args("project_settings.sourcesTab.count", [("count", count)]),
    (_, missing) => ctx.t_args(
      "project_settings.sourcesTab.countMissing",
      [("count", count), ("missing", missing.to_string())],
    ),
  };
  let project = props.project.open.project.name().as_str();
  let detail = Text::new(&ctx.t_args("project_settings.sourcesTab.detail", [("project", project)]))
    .id("settings-sources-detail")
    .variant(TypographyStyle::Description)
    .width(Dimension::Pct(100.0));
  section(
    "settings-sources",
    &ctx.t("project_settings.sourcesTab.title"),
    Some(&note),
    vec![detail.into()],
  )
  .spacing(SourcesSpacing::Section.role())
}

/// The project's sources, or that it has none.
pub(super) fn list(ctx: &mut Ctx, read: &Read, (remove, check_again): (Remove, CheckAgain), busy: bool) -> Element {
  if read.listed.is_empty() {
    return Column::new()
      .id("settings-sources-empty")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SourcesSpacing::EmptyTop.role())
          .bottom(crate::theme::ExtraSpacing::Snug.role()),
      )
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .child(Text::new(&ctx.t("project_settings.sourcesTab.emptyTitle")).variant(ExtraTypography::Strong.role()))
      .child(
        Text::new(&ctx.t("project_settings.sourcesTab.emptyText"))
          .variant(TypographyStyle::Description)
          .width(Dimension::Px(EMPTY_TEXT_WIDTH)),
      )
      .into();
  }
  let mut rows = Column::new().id("settings-sources-list").width(Dimension::Pct(100.0));
  for (index, listed) in read.listed.iter().enumerate() {
    let id = format!("settings-source-{index}");
    rows = rows.child(row(ctx, &id, listed, &read.checked, (&remove, busy)));
    if listed.state != State::Available {
      rows = rows.child(actions(ctx, &id, listed, (&remove, &check_again), busy));
    }
  }
  rows.into()
}

fn row(ctx: &mut Ctx, id: &str, listed: &Listed, checked: &str, (remove, busy): (&Remove, bool)) -> Element {
  let meta = match (listed.kind, listed.state) {
    (Some(Kind::Folder), State::Available) => ctx.t("project_settings.sourcesTab.folder"),
    (Some(Kind::File), State::Available) => ctx.t("project_settings.sourcesTab.file"),
    (Some(Kind::Folder), _) => ctx.t_args("project_settings.sourcesTab.folderChecked", [("time", checked)]),
    (Some(Kind::File), _) => ctx.t_args("project_settings.sourcesTab.fileChecked", [("time", checked)]),
    (None, _) => ctx.t_args("project_settings.sourcesTab.goneChecked", [("time", checked)]),
  };
  let path = listed.path.as_str();
  let mut source = SourceRow::new(id, path, listed.kind, meta).status(listed.state);
  if let Some(problem) = listed.problem() {
    source = source.note(message_text(ctx, &problem));
  }
  if !busy {
    let (remove, path) = (remove.clone(), listed.path.clone());
    source = source.removable(ctx.t_args("ui.source.remove", [("path", path.as_str())]), move || {
      remove(path.clone())
    });
  }
  source.render(ctx)
}

/// *Check again* and *Remove* under a source runs cannot read now.
fn actions(ctx: &Ctx, id: &str, listed: &Listed, (remove, check_again): (&Remove, &CheckAgain), busy: bool) -> Row {
  let check = ActionButton::new(
    format!("{id}-check"),
    ButtonKind::Secondary,
    ctx.t("project_settings.sourcesTab.checkAgain"),
  )
  .icon(Icon::RefreshCw);
  let drop = ActionButton::new(
    format!("{id}-drop"),
    ButtonKind::Ghost,
    ctx.t("project_settings.sourcesTab.remove"),
  );
  let (check, drop) = if busy {
    (check.disabled(), drop.disabled())
  } else {
    let (again, remove, path) = (check_again.clone(), remove.clone(), listed.path.clone());
    (
      check.on_click(move || again()),
      drop.on_click(move || remove(path.clone())),
    )
  };
  Row::new()
    .id(format!("{id}-actions"))
    .padding(Padding::vertical(lurq::app::theme::SpacingSize::Sm))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(check)
    .child(drop)
}

/// That `removed` is off the list, which runs keep it, and *Undo*, which asks again before it is back.
pub(super) fn removed(ctx: &Ctx, removed: &Removed, undo: impl Fn() + Send + Sync + 'static) -> Element {
  Notice::new(
    "settings-sources-removed",
    NoticeTone::Info,
    ctx.t_args("project_settings.sourcesTab.removed", [("path", removed.path.as_str())]),
  )
  .icon(Icon::CircleMinus)
  .body(message_text(ctx, &removed.message()))
  .action(
    ActionButton::new(
      "settings-sources-undo",
      ButtonKind::Secondary,
      ctx.t("project_settings.sourcesTab.undo"),
    )
    .on_click(undo),
  )
  .into()
}

/// *Add source*, checked against the project and its sources.
pub(super) fn add(ctx: &mut Ctx, read: &Read, locked: bool, on_add: Arc<dyn Fn(SourcePath) + Send + Sync>) -> Element {
  ctx.mount_keyed::<AddSource>(
    "settings-sources-add",
    AddSourceProps {
      id: "settings-sources-add".into(),
      place: Some(read.place.clone()),
      layout: Layout::Line,
      add: ctx.t("project_settings.sourcesTab.add"),
      locked,
      on_add,
    },
  )
}

/// What agents may do outside the worktree (each row's element id and its keys), and what holds it.
const OUTSIDE: [(&str, [&str; 3], Held); 5] = [
  (
    "settings-sources-read",
    [
      "project_settings.sourcesTab.readSource",
      "project_settings.sourcesTab.readSourceRule",
      "project_settings.sourcesTab.readSourceHow",
    ],
    Held::Enforced,
  ),
  (
    "settings-sources-elsewhere",
    [
      "project_settings.sourcesTab.readElsewhere",
      "project_settings.sourcesTab.readElsewhereRule",
      "project_settings.sourcesTab.readElsewhereHow",
    ],
    Held::Enforced,
  ),
  (
    "settings-sources-change",
    [
      "project_settings.sourcesTab.change",
      "project_settings.sourcesTab.changeRule",
      "project_settings.sourcesTab.changeHow",
    ],
    Held::Enforced,
  ),
  (
    "settings-sources-refused",
    [
      "project_settings.sourcesTab.refused",
      "project_settings.sourcesTab.refusedRule",
      "project_settings.sourcesTab.refusedHow",
    ],
    Held::Enforced,
  ),
  (
    "settings-sources-commands",
    [
      "project_settings.sourcesTab.commands",
      "project_settings.sourcesTab.commandsRule",
      "project_settings.sourcesTab.commandsHow",
    ],
    Held::Elsewhere,
  ),
];

pub(super) fn outside(ctx: &Ctx) -> Element {
  let enforced = ctx.t("project_settings.sourcesTab.enforced");
  let last = OUTSIDE.len() - 1;
  let rows = OUTSIDE
    .iter()
    .enumerate()
    .map(|(index, (id, [setting, rule, how], held))| {
      enforcement_row(
        id,
        (&ctx.t(setting), &ctx.t(rule), &ctx.t(how)),
        *held,
        &enforced,
        index == last,
      )
    });
  section(
    "settings-sources-outside",
    &ctx.t("project_settings.sourcesTab.outsideTitle"),
    None,
    rows.collect(),
  )
  .spacing(lurq::app::theme::SpacingSize::Xs)
  .into()
}

/// That a change applies to the runs that start after it.
pub(super) fn later(ctx: &Ctx) -> Element {
  Notice::new(
    "settings-sources-later",
    NoticeTone::Info,
    ctx.t("project_settings.sourcesTab.laterTitle"),
  )
  .icon(Icon::History)
  .body(ctx.t("project_settings.sourcesTab.laterDetail"))
  .into()
}
