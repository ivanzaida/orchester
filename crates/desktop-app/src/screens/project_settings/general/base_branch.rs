//! *Base branch* (`Base branch` `nqtSC` in 04 `rBSzW`): base branches are set on tasks and modules now, so the row
//! says where, with *Open Modules* at its end. For a project whose base branch moved to its open modules
//! (`orchester_workspace::project_settings::BaseMoved`), the row says once under its hint, in `info`, when it moved,
//! which modules took it and which kept their own (`Moved` `HcizH` in 04b `NDTCw`); no notice card. The line shows
//! until the settings close after it was shown ([`MovedSeen`]).

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{name::Name, usage::local_date};
use orchester_workspace::project_settings::{BaseMoved, forget_base_moved_in};
use tokio::runtime::Handle;

use super::super::{TabProps, kit::SettingRow};
use crate::{
  navigation::Navigation,
  routes::{ModuleListTab, ProjectPage, Route},
  screens::usage::words::day,
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::icon::{Icon, icon},
};

/// Whether General said how the base branch moved while the settings were open: closing them forgets the move, so the
/// line shows once. Clones share the mark; the settings screen and its tabs hold them, so the last goes as it closes.
#[derive(Clone)]
pub(in crate::screens) struct MovedSeen(Arc<Seen>);

struct Seen {
  shown: AtomicBool,
  root: PathBuf,
  runtime: Handle,
}

impl MovedSeen {
  pub(in crate::screens) fn new(root: PathBuf, runtime: Handle) -> Self {
    Self(Arc::new(Seen {
      shown: AtomicBool::new(false),
      root,
      runtime,
    }))
  }

  fn shown(&self) {
    self.0.shown.store(true, Ordering::Relaxed);
  }
}

/// The settings closed: the move General said is forgotten.
impl Drop for Seen {
  fn drop(&mut self) {
    if !self.shown.load(Ordering::Relaxed) {
      return;
    }
    let root = self.root.clone();
    self.runtime.spawn_blocking(move || {
      if let Err(error) = forget_base_moved_in(&root) {
        tracing::warn!(%error, "cannot forget how the base branch moved");
      }
    });
  }
}

pub(super) fn row(ctx: &mut Ctx, props: &TabProps, moved: Option<&BaseMoved>) -> Element {
  let moved = moved.map(|moved| {
    props.moved_seen.shown();
    moved_line(ctx, moved)
  });
  SettingRow::new("settings-base-branch", ctx.t("project_settings.baseBranch"))
    .hint(ctx.t("project_settings.baseBranchHint"))
    .lines(moved)
    .control(open_modules(ctx, props))
    .render(ctx)
}

/// *Open Modules ↗* (`Link` `Reqm3`): 12 `text_secondary`, a 12 `text_muted` arrow, 6 apart.
fn open_modules(ctx: &mut Ctx, props: &TabProps) -> Element {
  let navigation = Navigation::of(ctx);
  let modules = Route::Project(
    props.project.open.project.id(),
    ProjectPage::Modules(ModuleListTab::default()),
  );
  let label = ctx.t("project_settings.openModules");
  crate::ui::pressable::opening(navigation.to(modules))
    .id("settings-base-branch-modules")
    .describe("a11y_name", label.clone())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// When the base branch moved and to which modules, in `info` beside a `git-branch` (`HcizH`).
fn moved_line(ctx: &Ctx, moved: &BaseMoved) -> Element {
  let date = day(ctx, local_date(moved.at));
  let args = |modules: &str, kept: &str| {
    [
      ("date", date.to_string()),
      ("branch", moved.branch.to_string()),
      ("modules", modules.to_owned()),
      ("kept", kept.to_owned()),
    ]
  };
  let took = listed(ctx, &moved.took);
  let kept = listed(ctx, &moved.kept);
  let key = match (moved.took.is_empty(), moved.kept.len()) {
    (true, _) => "project_settings.baseBranchMoved.none",
    (false, 0) => "project_settings.baseBranchMoved.took",
    (false, 1) => "project_settings.baseBranchMoved.keptOne",
    (false, _) => "project_settings.baseBranchMoved.keptMany",
  };
  let words = ctx.t_args(key, args(&took, &kept));
  Row::new()
    .id("settings-base-branch-moved")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::GitBranch, IconSize::Sm, PaletteColor::Info))
    .child(
      Text::new(&words)
        .id("settings-base-branch-moved-text")
        .variant(ProjectSettingsText::Hint.role())
        .color(PaletteColor::Info)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// `names` as one list: "A", "A and B", "A, B and C".
fn listed(ctx: &Ctx, names: &[Name]) -> String {
  match names {
    [] => String::new(),
    [one] => one.as_str().to_owned(),
    [rest @ .., last] => {
      let rest: Vec<&str> = rest.iter().map(Name::as_str).collect();
      let rest = rest.join(&ctx.t("project_settings.listSeparator"));
      ctx
        .t_args(
          "project_settings.listAnd",
          [("rest", rest.as_str()), ("last", last.as_str())],
        )
        .to_string()
    }
  }
}
