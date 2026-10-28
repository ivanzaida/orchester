//! General (`01 · General` `hz6eK`, its states `01b` `nHr9b`): the project's name, key and folder as plain values, and
//! the repository: its GitHub repository with Check again, the GitHub account the project expects with how `gh`
//! matches it, the base branch (ORC-81), telling working agents when the base branch moves (ORC-54), and the
//! repository's agent files with what each CLI loads in a popover.
mod agent_files;
mod base_branch;
mod github;
use lurq::{
  app::{component::Component, ctx::Ctx, theme::PaletteColor},
  components::Row,
  core::{ElementRef, Signal},
  layout::Alignment,
  node::Element,
};
use orchester_workspace::{project_settings::Settings, tasks::outside::Outside};

use super::{
  TabProps,
  kit::{Group, SettingRow, mono_value, value},
  page,
  work::{self, Change, Read},
};
use crate::{
  mcp::input_gate::InputGate,
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};
#[derive(Clone)]
pub(super) struct General {
  save: work::Save,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
  gate: Option<InputGate>,
  /// An MCP client's input asked to turn the repository's agent files on and was refused.
  refused: Signal<bool>,
  /// *What each CLI loads*' popover, and its link.
  files_open: Signal<bool>,
  files_anchor: ElementRef,
}
impl Component for General {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(ctx, &[Outside::Connections, Outside::ProjectSettings]),
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(false),
      files_open: ctx.signal(false),
      files_anchor: ctx.element_ref(),
    };
    let save = tab.save.clone();
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      (tab.revision.clone(), tab.stored.clone(), tab.own.clone()),
    );
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = self.save.clone();
    let revision = self.revision.get() + self.own.count(&props.project.feed);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision, &self.own);
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(work::pending(ctx, &state, &self.revision));
    };
    self.stored.set(Some(read.clone()));
    let settings = read.0;
    let identity = identity(ctx, &props);
    let mut rows = github::rows(ctx, &props, &settings, &save, self.revision.get());
    rows.push(ctx.mount_keyed::<base_branch::BaseBranch>("settings-base-branch-setting", props.clone()));
    rows.push(base_notices(ctx, &props, &settings, &save));
    rows.push(agent_files::row(
      ctx,
      (&props, &settings, &save),
      (self.gate.clone(), &self.refused),
      (&self.files_open, &self.files_anchor),
    ));
    let repository = Group::new("settings-repository", ctx.t("project_settings.repository")).rows(rows);
    page::scroll(
      lurq::components::Column::new()
        .width(lurq::node::dimension::Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Roomy.role())
        .with_children(work::error(ctx, &save))
        .child(identity)
        .child(repository),
    )
  }
}
/// *When the base branch moves* (ORC-54): whether working agents are told to merge the moved base early.
fn base_notices(ctx: &mut Ctx, props: &TabProps, settings: &Settings, save: &work::Save) -> Element {
  let on = settings.base_notices;
  let start = work::start(save, props.project.open.repo_root.clone(), Change::BaseNotices(!on));
  let toggle = Toggle::new(
    "settings-base-notices",
    ToggleKind::Switch,
    on,
    ctx.t(if on {
      "project_settings.baseMovedOn"
    } else {
      "project_settings.baseMovedOff"
    }),
  )
  .hide_label();
  let toggle = if save.state().get().is_pending() {
    toggle
  } else {
    toggle.on_change(move |_| start())
  };
  SettingRow::new("settings-base-moved", ctx.t("project_settings.baseMovedSection"))
    .info(ctx.t("project_settings.baseMovedDetail"))
    .hint(ctx.t(if on {
      "project_settings.baseMovedHint"
    } else {
      "project_settings.baseMovedOffHint"
    }))
    .control(toggle)
    .render(ctx)
}
/// The project's name, key and folder, as plain values: none of them changes here.
fn identity(ctx: &mut Ctx, props: &TabProps) -> Element {
  let project = &props.project.open;
  let folder = project.repo_root.display().to_string();
  let folder = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Folder, IconSize::Base, PaletteColor::TextMuted))
    .child(mono_value("settings-project-folder", &folder));
  let rows = [
    SettingRow::new("settings-name", ctx.t("project_settings.nameLabel"))
      .control(value("settings-project-name", project.project.name().as_str())),
    SettingRow::new("settings-key", ctx.t("project_settings.keyLabel"))
      .hint(ctx.t("project_settings.keyHint"))
      .control(mono_value("settings-project-key", project.project.key().as_str())),
    SettingRow::new("settings-folder", ctx.t("project_settings.folderLabel"))
      .hint(ctx.t("project_settings.folderHint"))
      .control(folder),
  ]
  .map(|row| row.render(ctx));
  Group::new("settings-project", ctx.t("project_settings.project"))
    .rows(rows)
    .into()
}
