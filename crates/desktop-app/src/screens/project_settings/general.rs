//! General (tLCm1, tQFZQ): project identity, GitHub, repository agent files, telling working agents when the base
//! branch moves (ORC-54), auto mode and the base branch (ORC-81). Neither ORC-54 nor ORC-81 has a frame of its own:
//! they are laid out as the tab's switch and select rows.
mod auto_mode;
mod base_branch;
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
use orchester_workspace::{project_settings::Settings, prompt_changes, tasks::outside::Outside};

use super::{
  TabProps, page,
  work::{self, Change, Read},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  theme::ExtraTypography,
  ui::{
    field::FieldInput,
    form_section::{Place, not_built, section},
    message::message_text,
    tinted_note::person_only,
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
}
impl Component for General {
  type Props = TabProps;
  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(
        ctx,
        &[
          Outside::AutoMode,
          Outside::Runs,
          Outside::Connections,
          Outside::ProjectSettings,
        ],
      ),
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(false),
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
    let github = github::section(ctx, &props, &settings, &save, self.revision.get());
    let files = agent_files(ctx, (&props, &settings, &save), (self.gate.clone(), &self.refused));
    let base = base_notices(ctx, &props, &settings, &save);
    let auto = ctx.mount_keyed::<auto_mode::AutoMode>(
      "settings-auto-mode",
      auto_mode::Props {
        tab: props.clone(),
        settings: settings.clone(),
        parent_read: self.stored.clone(),
        refresh: self.revision.clone(),
        own: self.own.clone(),
      },
    );
    let branch = ctx.mount_keyed::<base_branch::BaseBranch>("settings-base-branch-setting", props.clone());
    let branch = section(
      "settings-base-branch-setting-section",
      &ctx.t("project_settings.baseBranchSection"),
      None,
      vec![branch],
    );
    page::scroll(
      Column::new()
        .spacing(crate::theme::ExtraSpacing::Roomy.role())
        .with_children(work::error(ctx, &save))
        .child(identity)
        .child(github)
        .child(files)
        .child(base)
        .child(auto)
        .child(branch),
    )
  }
}
/// *Tell working agents when the base branch moves* (ORC-54), laid out as the repository agent files' switch row.
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
  );
  let toggle = if save.state().get().is_pending() {
    toggle
  } else {
    toggle.on_change(move |_| start())
  };
  let control = Column::new().width(Dimension::Px(page::CONTROL_WIDTH)).child(toggle);
  let row = page::row(
    ctx,
    "settings-base-moved",
    ("project_settings.baseMoved", "project_settings.baseMovedDetail"),
    control.into(),
    Place::Last,
  );
  section(
    "settings-base-branch",
    &ctx.t("project_settings.baseMovedSection"),
    None,
    vec![row],
  )
  .into()
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
/// The switch of the repository's agent files. Turning them on puts what the repository says in every agent's prompt
/// (ORC-30): an MCP client's input turns them on only under *Let automation change agent permissions*, as
/// `set_repository_agent_files` does; turning them off stays open. A refused press says so under the switch
/// (`refused`).
fn agent_files_switch(
  ctx: &Ctx,
  (props, settings, save): (&TabProps, &Settings, &work::Save),
  (gate, refused): (Option<InputGate>, &Signal<bool>),
) -> Column {
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
    let refusal = refused.clone();
    toggle.on_change(move |_| {
      let may = !prompt_changes::agent_files_change_prompts(on, !on)
        || input_may_decide(gate.as_ref(), Guarded::PermissionWidening);
      refusal.set(!may);
      if may {
        start();
      } else {
        tracing::warn!("synthetic input turned on the repository's agent files; not taken");
      }
    })
  };
  Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(toggle)
    .with_children(refused.get().then(|| {
      person_only(
        "settings-agent-files-person-only",
        &message_text(ctx, &prompt_changes::agent_files_person_only()),
      )
    }))
}
fn agent_files(
  ctx: &mut Ctx,
  (props, settings, save): (&TabProps, &Settings, &work::Save),
  gated: (Option<InputGate>, &Signal<bool>),
) -> Element {
  let on = settings.agent_files.is_on();
  let control = agent_files_switch(ctx, (props, settings, save), gated);
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
