//! General (tLCm1, tQFZQ): project identity, GitHub, repository agent files and auto mode.
mod auto_mode;
mod github;
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{project_settings::Settings, tasks::outside::Outside};

use super::{
  TabProps, page,
  work::{self, Change, Read},
};
use crate::{
  theme::ExtraTypography,
  ui::{
    field::FieldInput,
    form_section::{Place, not_built, section},
    toggle::{Toggle, ToggleKind},
  },
};
#[derive(Clone)]
pub(super) struct General {
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
}
impl Component for General {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      stored: ctx.signal(None),
      revision: ctx.signal(0),
    };
    let save = work::action(ctx);
    work::follow(
      ctx,
      &save,
      ctx.props::<TabProps>().status.clone(),
      tab.revision.clone(),
      tab.stored.clone(),
    );
    tab
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let save = work::action(ctx);
    let outside = props.project.feed.outside.get();
    let revision = self.revision.get()
      + outside.of(Outside::AutoMode)
      + outside.of(Outside::Runs)
      + outside.of(Outside::Connections)
      + outside.of(Outside::ProjectSettings);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision);
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(work::pending(ctx, &state, &self.revision));
    };
    self.stored.set(Some(read.clone()));
    let settings = read.0;
    let identity = identity(ctx, &props);
    let github = github::section(ctx, &props, &settings, &save, self.revision.get());
    let files = agent_files(ctx, &props, &settings, &save);
    let auto = ctx.mount_keyed::<auto_mode::AutoMode>(
      "settings-auto-mode",
      auto_mode::Props {
        tab: props.clone(),
        settings: settings.clone(),
        parent_read: self.stored.clone(),
        refresh: self.revision.clone(),
      },
    );
    page::scroll(
      Column::new()
        .spacing(crate::theme::ExtraSpacing::Roomy.role())
        .with_children(work::error(ctx, &save))
        .child(identity)
        .child(github)
        .child(files)
        .child(auto),
    )
  }
}
fn locked(ctx: &mut Ctx, id: &str, label: &str, value: &str) -> Element {
  FieldInput::new(id.to_owned(), ctx.t(label), Signal::new(value.to_owned()))
    .locked()
    .width(Dimension::Pct(100.0))
    .render(ctx)
}
fn identity(ctx: &mut Ctx, props: &TabProps) -> Element {
  let project = &props.project.open;
  let name = locked(
    ctx,
    "settings-project-name",
    "project_settings.name",
    project.project.name().as_str(),
  );
  let key = locked(
    ctx,
    "settings-project-key",
    "project_settings.key",
    project.project.key().as_str(),
  );
  let folder = locked(
    ctx,
    "settings-project-folder",
    "project_settings.folder",
    &project.repo_root.display().to_string(),
  );
  let control = |element| {
    Column::new()
      .width(Dimension::Px(page::CONTROL_WIDTH))
      .child(element)
      .into()
  };
  let folder = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(folder)
    .child(page::note(
      "settings-folder-note",
      &ctx.t("project_settings.folderNote"),
    ))
    .into();
  section(
    "settings-project",
    &ctx.t("project_settings.project"),
    None,
    vec![
      page::row(
        ctx,
        "settings-name",
        ("project_settings.name", "project_settings.nameDetail"),
        control(name),
        Place::Within,
      ),
      page::row(
        ctx,
        "settings-key",
        ("project_settings.key", "project_settings.keyDetail"),
        control(key),
        Place::Within,
      ),
      page::row(
        ctx,
        "settings-folder",
        ("project_settings.folder", "project_settings.folderDetail"),
        folder,
        Place::Last,
      ),
    ],
  )
  .into()
}
fn agent_files(ctx: &mut Ctx, props: &TabProps, settings: &Settings, save: &work::Save) -> Element {
  let on = settings.agent_files.is_on();
  let start = work::start(save, props.project.open.repo_root.clone(), Change::Files(!on));
  let toggle = Toggle::new(
    "settings-agent-files",
    ToggleKind::Switch,
    on,
    ctx.t(if on {
      "project_settings.filesOn"
    } else {
      "project_settings.filesOff"
    }),
  );
  let toggle = if save.state().get().is_pending() {
    toggle
  } else {
    toggle.on_change(move |_| start())
  };
  let control = Column::new().width(Dimension::Px(page::CONTROL_WIDTH)).child(toggle);
  let row = page::row(
    ctx,
    "settings-files",
    ("project_settings.agentFiles", "project_settings.agentFilesDetail"),
    control.into(),
    Place::Last,
  );
  let codex = if on {
    ctx.t("project_settings.codexLoads")
  } else {
    ctx.t("project_settings.none")
  };
  let content = Column::new()
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("project_settings.filesPanel")).variant(ExtraTypography::Strong.role()))
    .child(files_pair(
      ctx,
      "settings-files-codex",
      "project_settings.codex",
      &codex,
    ))
    .child(files_pair(
      ctx,
      "settings-files-claude",
      "project_settings.claude",
      &ctx.t("project_settings.claudeNone"),
    ))
    .child(
      Text::new(&ctx.t("project_settings.filesNote"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(
      not_built(
        "settings-import",
        &ctx.t("project_settings.import"),
        &ctx.t("project_settings.importUnavailable"),
      )
      .align_items(lurq::layout::Alignment::Start),
    );
  let panel = Column::new()
    .id("settings-files-panel")
    .padding(lurq::node::padding::Padding::symmetric(
      crate::theme::ExtraSpacing::Inset.role(),
      SpacingSize::Md,
    ))
    .border(lurq::node::border::Border::inside(
      lurq::app::theme::BorderSize::Sm,
      PaletteColor::Border,
    ))
    .background(PaletteColor::SurfaceRaised)
    .rounded(crate::theme::ExtraRadius::Tile.role())
    .child(content);
  section(
    "settings-repository-agents",
    &ctx.t("project_settings.agentFilesSection"),
    None,
    vec![row, panel.into()],
  )
  .into()
}
/// The repository panel's aligned labels (hDIi9).
const AGENT_LABEL_WIDTH: f32 = 130.0;
fn files_pair(ctx: &Ctx, id: &str, key: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Text::new(&ctx.t(key))
        .id(id.to_owned())
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(AGENT_LABEL_WIDTH)),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-values"))
        .variant(lurq::app::theme::TypographyStyle::Mono)
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
    .into()
}
