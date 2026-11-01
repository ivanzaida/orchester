//! Context › *Repository agent files* (`gY4HX` in `S04 · Context` `oTBHM`, between Sources and Code index): whether
//! agents may read the repository's own AGENTS.md, CLAUDE.md and skills folders, with *What each CLI loads* in a
//! popover (what Codex and Claude Code load from the repository, that the files are not versioned with the run, and
//! Import from repository, not built yet). Turning them on puts what the repository says in every agent's prompt
//! (ORC-30): an MCP client's input turns them on only under *Let automation change agent permissions*, as
//! `set_repository_agent_files` does; turning them off stays open. A refused press says so under the row.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{project_settings::Settings, prompt_changes, tasks::outside::Outside};

use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::project_settings::{
    TabProps,
    kit::{Group, SettingRow},
    work::{self, Change, Read},
  },
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::not_built,
    message::message_text,
    popover::{PopoverSide, popover, popover_panel},
    skeleton,
    tinted_note::person_only,
    toggle::{Toggle, ToggleKind},
  },
};

/// The popover's width, and its labels' (hDIi9).
const POPOVER_WIDTH: f32 = 420.0;
const AGENT_LABEL_WIDTH: f32 = 100.0;

/// The group, saving its switch itself as Sources and Code index do.
pub(super) struct AgentFiles {
  save: work::Save,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
  gate: Option<InputGate>,
  /// An MCP client's input asked to turn the repository's agent files on and was refused.
  refused: Signal<bool>,
  /// *What each CLI loads*' popover, and its link.
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for AgentFiles {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let section = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(ctx, &[Outside::ProjectSettings]),
      gate: ctx.use_context::<InputGate>(),
      refused: ctx.signal(false),
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    };
    work::follow(
      ctx,
      &section.save,
      ctx.props::<TabProps>().status.clone(),
      (section.revision.clone(), section.stored.clone(), section.own.clone()),
    );
    section
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let revision = self.revision.get() + self.own.count(&props.project.feed);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision, &self.own);
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return work::pending(ctx, &state, &self.revision, bones);
    };
    self.stored.set(Some(read.clone()));
    let row = row(
      ctx,
      (&props, &read.0, &self.save),
      (self.gate.clone(), &self.refused),
      (&self.open, &self.anchor),
    );
    let mut group = Group::new("settings-repository-files", ctx.t("project_settings.agentFiles")).row(row);
    if let Some(error) = work::error(ctx, &self.save) {
      group = group.after(error);
    }
    if let Some(error) = state.error {
      group = group.after(field_error("settings-files-load-error", &message_text(ctx, &error)));
    }
    group.into()
  }
}

/// The group while the settings are read: its heading, the row's label and hint, and a switch's bone.
fn bones(ctx: &mut Ctx) -> Group {
  let row = SettingRow::new("settings-files-reading", ctx.t("project_settings.readAgentFiles"))
    .info(ctx.t("project_settings.agentFilesDetail"))
    .hint(ctx.t("project_settings.agentFilesHint"))
    .control(skeleton::switch())
    .render(ctx);
  Group::new(
    "settings-repository-files-reading",
    ctx.t("project_settings.agentFiles"),
  )
  .row(row)
}

fn row(
  ctx: &mut Ctx,
  (props, settings, save): (&TabProps, &Settings, &work::Save),
  (gate, refused): (Option<InputGate>, &Signal<bool>),
  (open, anchor): (&Signal<bool>, &ElementRef),
) -> Element {
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
  )
  .hide_label();
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
  let shown = open.clone();
  let link = ActionButton::new(
    "settings-files-loads",
    ButtonKind::Ghost,
    ctx.t("project_settings.whatEachLoads"),
  )
  .compact()
  .element_ref(anchor.clone())
  .on_click(move || shown.update(|open| *open = !*open));
  let panel = popover(anchor.clone(), open.clone(), PopoverSide::BelowEnd, panel(ctx, on));
  let refusal = refused.get().then(|| {
    person_only(
      "settings-agent-files-person-only",
      &message_text(ctx, &prompt_changes::agent_files_person_only()),
    )
  });
  SettingRow::new("settings-files", ctx.t("project_settings.readAgentFiles"))
    .info(ctx.t("project_settings.agentFilesDetail"))
    .hint(ctx.t("project_settings.agentFilesHint"))
    .control(Row::new().child(link).child(panel))
    .control(toggle)
    .under_all(refusal.map(Element::from))
    .render(ctx)
}

/// What each CLI may load from the repository, why the files are not versioned with the run, and the import that is
/// not built yet.
fn panel(ctx: &Ctx, on: bool) -> Column {
  let codex = if on {
    ctx.t("project_settings.codexLoads")
  } else {
    ctx.t("project_settings.none")
  };
  popover_panel("settings-files-panel", POPOVER_WIDTH)
    .child(Text::new(&ctx.t("project_settings.filesPanel")).variant(ExtraTypography::Strong.role()))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(pair(ctx, "settings-files-codex", "project_settings.codex", &codex))
        .child(pair(
          ctx,
          "settings-files-claude",
          "project_settings.claude",
          &ctx.t("project_settings.claudeNone"),
        )),
    )
    .child(
      Text::new(&ctx.t("project_settings.filesNote"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      not_built(
        "settings-import",
        &ctx.t("project_settings.import"),
        &ctx.t("project_settings.importUnavailable"),
      )
      .align_items(Alignment::Start),
    )
}

fn pair(ctx: &Ctx, id: &str, key: &str, value: &str) -> Row {
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
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
}
