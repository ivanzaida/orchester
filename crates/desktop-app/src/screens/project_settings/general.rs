//! General (`01 · General` `hz6eK`, its states `01b` `nHr9b`): the project's name, key and folder as plain values, and
//! the repository: its GitHub repository with Check again, the GitHub account the project expects with how `gh`
//! matches it, telling working agents when the base branch moves (ORC-54), and the repository's agent files with what
//! each CLI loads in a popover. Base branches are set on tasks and modules (`Base branch · redesign`, `ri5ru`).
mod agent_files;
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
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::{
    github_mark::github_mark,
    icon::{Icon, icon},
    skeleton,
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
      return page::scroll(work::pending(ctx, &state, &self.revision, bones));
    };
    self.stored.set(Some(read.clone()));
    let settings = read.0;
    let identity = identity(ctx, &props);
    let mut rows = github::rows(ctx, &props, &settings, &save, self.revision.get());
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
/// The General tab while its settings are read (`09 · Project settings · General · loading` `XvyXm`): the Project and
/// Repository headings, every row label and its fixed help line stay; the name, key, folder, repository, signed-in
/// account and both switches are bones in the value column, the lower rows fading.
fn bones(ctx: &mut Ctx) -> lurq::components::Column {
  let mono = |ctx: &Ctx, width| skeleton::line(ctx, ProjectSettingsText::MonoValue.role(), width);
  let marked = |ctx: &Ctx, glyph: Element, width| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(glyph)
      .child(mono(ctx, width))
  };
  let project = [
    SettingRow::new("settings-name-reading", ctx.t("project_settings.nameLabel")).control(skeleton::line(
      ctx,
      ProjectSettingsText::Value.role(),
      40.0,
    )),
    SettingRow::new("settings-key-reading", ctx.t("project_settings.keyLabel"))
      .hint(ctx.t("project_settings.keyHint"))
      .control(mono(ctx, 18.0)),
    SettingRow::new("settings-folder-reading", ctx.t("project_settings.folderLabel"))
      .hint(ctx.t("project_settings.folderHint"))
      .control(marked(
        ctx,
        icon(Icon::Folder, IconSize::Base, PaletteColor::TextMuted).into(),
        120.0,
      )),
  ]
  .map(|row| row.render(ctx));
  let github = github_mark(ctx, IconSize::Base, PaletteColor::TextSecondary);
  let repository = [
    SettingRow::new("settings-github-repo-reading", ctx.t("project_settings.githubRepo"))
      .info(ctx.t("project_settings.githubRepoDetail"))
      .hint(ctx.t("project_settings.githubRepoHint"))
      .control(marked(ctx, github, 58.0)),
    SettingRow::new(
      "settings-github-login-reading",
      ctx.t("project_settings.githubAccountLabel"),
    )
    .info(ctx.t("project_settings.githubAccountDetail"))
    .hint_reading(152.0)
    .control(mono(ctx, 42.0)),
    SettingRow::new(
      "settings-base-moved-reading",
      ctx.t("project_settings.baseMovedSection"),
    )
    .info(ctx.t("project_settings.baseMovedDetail"))
    .hint(ctx.t("project_settings.baseMovedHint"))
    .control(skeleton::switch()),
    SettingRow::new("settings-files-reading", ctx.t("project_settings.agentFiles"))
      .info(ctx.t("project_settings.agentFilesDetail"))
      .hint(ctx.t("project_settings.agentFilesHint"))
      .control(skeleton::switch()),
  ]
  .map(|row| row.render(ctx));
  let groups = vec![
    lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .child(Group::new("settings-project-reading", ctx.t("project_settings.project")).rows(project)),
    lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .child(Group::new("settings-repository-reading", ctx.t("project_settings.repository")).rows(repository)),
  ];
  lurq::components::Column::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .with_children(skeleton::fading(groups))
}
