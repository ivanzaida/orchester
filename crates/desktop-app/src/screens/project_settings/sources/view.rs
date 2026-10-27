//! The parts of Context › Sources: the group's heading with its count, a row per source (`E:\docs\api` `waZCD`: its
//! path, whether it is a folder or a file, and Remove; a source runs cannot read now says why in `warning` with Check
//! again and Remove), the empty row (`Empty · No sources yet` `JKOnV`), a removed source with Undo, *Add a folder or
//! file* (`GzqFE`, its Add opening the path box under it), and what agents may do outside the worktree in the
//! heading's (i).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};
use orchester_core::source::SourcePath;
use orchester_workspace::sources::{
  check::Kind,
  listed::{Listed, State},
  project::Removed,
};

use super::Read;
use crate::{
  screens::project_settings::kit::{EmptyRow, Group, SettingRow, Tone},
  theme::IconSize,
  ui::{
    add_source::{AddSource, AddSourceProps, Layout},
    button::{ActionButton, ButtonKind},
    enforcement_row::Held,
    icon::{Icon, icon},
    icon_button::IconButton,
    message::message_text,
    tooltip::Tooltip,
  },
};

type Remove = Arc<dyn Fn(SourcePath) + Send + Sync>;
type CheckAgain = Arc<dyn Fn() + Send + Sync>;

/// The group's heading, with how many sources there are and how many runs cannot read.
pub(super) fn heading(ctx: &Ctx, read: &Read) -> Group {
  let missing = read
    .listed
    .iter()
    .filter(|listed| listed.state != State::Available)
    .count();
  let count = read.listed.len().to_string();
  let note = match (read.listed.len(), missing) {
    (0, _) => None,
    (1, 0) => Some(ctx.t("project_settings.sourcesTab.countOne")),
    (_, 0) => Some(ctx.t_args("project_settings.sourcesTab.countOther", [("count", count)])),
    (_, missing) => Some(ctx.t_args(
      "project_settings.sourcesTab.countMissing",
      [("count", count), ("missing", missing.to_string())],
    )),
  };
  let group = Group::new("settings-sources", ctx.t("project_settings.sourcesTab.title"));
  match note {
    Some(note) => group.end(
      lurq::components::Text::new(&note)
        .id("settings-sources-note")
        .variant(crate::theme::ExtraTypography::Note.role()),
    ),
    None => group,
  }
}

/// A row per source, or the empty row, whose Add opens the path box.
pub(super) fn rows(
  ctx: &mut Ctx,
  read: &Read,
  (remove, check_again): (Remove, CheckAgain),
  busy: bool,
  adding: &Signal<bool>,
) -> Vec<Element> {
  if read.listed.is_empty() {
    let add = ActionButton::new(
      "settings-sources-empty-add",
      ButtonKind::Secondary,
      ctx.t("project_settings.sourcesTab.addShort"),
    )
    .icon(Icon::Plus)
    .in_form();
    let add = if adding.get() || busy {
      add.disabled()
    } else {
      let adding = adding.clone();
      add.on_click(move || adding.set(true))
    };
    return vec![
      EmptyRow::new(
        "settings-sources-empty",
        Icon::FolderSearch,
        ctx.t("project_settings.sourcesTab.emptyTitle"),
        ctx.t("project_settings.sourcesTab.emptyShort"),
      )
      .action(add)
      .into(),
    ];
  }
  read
    .listed
    .iter()
    .enumerate()
    .map(|(index, listed)| {
      row(
        ctx,
        &format!("settings-source-{index}"),
        listed,
        &read.checked,
        (&remove, &check_again, busy),
      )
    })
    .collect()
}

fn row(
  ctx: &mut Ctx,
  id: &str,
  listed: &Listed,
  checked: &str,
  (remove, check_again, busy): (&Remove, &CheckAgain, bool),
) -> Element {
  let meta = match (listed.kind, listed.state) {
    (Some(Kind::Folder), State::Available) => ctx.t("project_settings.sourcesTab.folder"),
    (Some(Kind::File), State::Available) => ctx.t("project_settings.sourcesTab.file"),
    (Some(Kind::Folder), _) => ctx.t_args("project_settings.sourcesTab.folderChecked", [("time", checked)]),
    (Some(Kind::File), _) => ctx.t_args("project_settings.sourcesTab.fileChecked", [("time", checked)]),
    (None, _) => ctx.t_args("project_settings.sourcesTab.goneChecked", [("time", checked)]),
  };
  let path = listed.path.as_str();
  let row = SettingRow::new(id.to_owned(), path).small_label();
  let row = match listed.problem() {
    Some(problem) => row.toned(message_text(ctx, &problem), Tone::Warning).info(meta),
    None => row.hint(meta),
  };
  let label = ctx.t_args("ui.source.remove", [("path", path)]);
  let remove_button = IconButton::new(format!("{id}-remove"), Icon::X, label).small();
  let remove_button = if busy {
    remove_button
  } else {
    let (remove, path) = (remove.clone(), listed.path.clone());
    remove_button.on_click(move || remove(path.clone()))
  };
  let remove_button = remove_button.render(ctx);
  if listed.state == State::Available {
    return row.control(remove_button).render(ctx);
  }
  let check = ActionButton::new(
    format!("{id}-check"),
    ButtonKind::Secondary,
    ctx.t("project_settings.sourcesTab.checkAgain"),
  )
  .icon(Icon::RefreshCw)
  .in_form();
  let check = if busy {
    check.disabled()
  } else {
    let again = check_again.clone();
    check.on_click(move || again())
  };
  row.control(check).control(remove_button).render(ctx)
}

/// That `removed` is off the list and which runs keep it, with *Undo*, which asks again before it is back.
pub(super) fn removed(ctx: &mut Ctx, removed: &Removed, undo: impl Fn() + Send + Sync + 'static) -> Element {
  SettingRow::new(
    "settings-sources-removed",
    ctx.t_args("project_settings.sourcesTab.removed", [("path", removed.path.as_str())]),
  )
  .small_label()
  .hint(message_text(ctx, &removed.message()))
  .control(
    ActionButton::new(
      "settings-sources-undo",
      ButtonKind::Secondary,
      ctx.t("project_settings.sourcesTab.undo"),
    )
    .icon(Icon::Undo2)
    .in_form()
    .on_click(undo),
  )
  .render(ctx)
}

/// *Add a folder or file*: Add opens the path box under the row, checked against the project and its sources.
pub(super) fn add(
  ctx: &mut Ctx,
  read: &Read,
  (locked, adding): (bool, &Signal<bool>),
  on_add: Arc<dyn Fn(SourcePath) + Send + Sync>,
) -> Element {
  let row = SettingRow::new(
    "settings-sources-add-row",
    ctx.t("project_settings.sourcesTab.addTitle"),
  )
  .hint(ctx.t("project_settings.sourcesTab.addHint"))
  .info(ctx.t("project_settings.sourcesTab.addInfo"));
  if !adding.get() {
    let open = adding.clone();
    return row
      .control(
        ActionButton::new(
          "settings-sources-open-add",
          ButtonKind::Secondary,
          ctx.t("project_settings.sourcesTab.addShort"),
        )
        .icon(Icon::Plus)
        .in_form()
        .on_click(move || open.set(true)),
      )
      .render(ctx);
  }
  let box_ = ctx.mount_keyed::<AddSource>(
    "settings-sources-add",
    AddSourceProps {
      id: "settings-sources-add".into(),
      place: Some(read.place.clone()),
      layout: Layout::Line,
      add: ctx.t("project_settings.sourcesTab.add"),
      locked,
      on_add,
    },
  );
  row.under(box_).render(ctx)
}

/// What agents may do outside the worktree (its keys: what, the rule, what holds it), and whether Orchester holds it.
const OUTSIDE: [([&str; 3], Held); 5] = [
  (
    [
      "project_settings.sourcesTab.readSource",
      "project_settings.sourcesTab.readSourceRule",
      "project_settings.sourcesTab.readSourceHow",
    ],
    Held::Enforced,
  ),
  (
    [
      "project_settings.sourcesTab.readElsewhere",
      "project_settings.sourcesTab.readElsewhereRule",
      "project_settings.sourcesTab.readElsewhereHow",
    ],
    Held::Enforced,
  ),
  (
    [
      "project_settings.sourcesTab.change",
      "project_settings.sourcesTab.changeRule",
      "project_settings.sourcesTab.changeHow",
    ],
    Held::Enforced,
  ),
  (
    [
      "project_settings.sourcesTab.refused",
      "project_settings.sourcesTab.refusedRule",
      "project_settings.sourcesTab.refusedHow",
    ],
    Held::Enforced,
  ),
  (
    [
      "project_settings.sourcesTab.commands",
      "project_settings.sourcesTab.commandsRule",
      "project_settings.sourcesTab.commandsHow",
    ],
    Held::Elsewhere,
  ),
];

/// What agents may do outside the worktree, which the design leaves out of the tab (`03 · Context` `ux0Kh`): the
/// Sources heading's (i), each rule on a line of its own with what holds it, *Enforced* where Orchester holds it.
pub(super) fn outside_info(ctx: &mut Ctx) -> Element {
  let lines = OUTSIDE
    .iter()
    .map(|([setting, rule, how], held)| {
      let key = match held {
        Held::Enforced => "project_settings.sourcesTab.outsideEnforced",
        Held::Elsewhere => "project_settings.sourcesTab.outsideLine",
      };
      ctx
        .t_args(
          key,
          [("setting", ctx.t(setting)), ("rule", ctx.t(rule)), ("how", ctx.t(how))],
        )
        .to_string()
    })
    .collect::<Vec<_>>();
  let text = ctx.t_args("project_settings.sourcesTab.outsideInfo", [("rules", lines.join("\n"))]);
  let glyph = icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted).id("settings-sources-outside-info");
  Tooltip::new("settings-sources-outside-info", text).wrap(ctx, glyph)
}
