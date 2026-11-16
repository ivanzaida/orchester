//! Context › *Repository agent files* (`gY4HX` in `S04 · Context` `oTBHM`, between Sources and Code index): whether
//! agents may read the repository's own AGENTS.md, CLAUDE.md and skills folders, with *What each CLI loads* in a
//! popover (what Codex and Claude Code load from the repository, that the files are not versioned with the run, and
//! Import from repository, not built yet). Turning them on puts what the repository says in every agent's prompt
//! (ORC-30): an MCP client's input turns them on only under *Let automation change agent permissions*, as
//! `set_repository_agent_files` does; turning them off stays open. A refused press is refused to the client, and the
//! row stays as it was.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{project_settings::Settings, prompt_changes};

use crate::{
  mcp::input_gate::{Guarded, InputGate, may_press},
  queries::Kept,
  screens::project_settings::{
    TabProps,
    kit::{Group, SettingRow},
    work::{self, Change, Read},
  },
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
    popover::{PopoverSide, popover, popover_panel},
    skeleton,
    toggle::{Toggle, ToggleKind},
  },
};

/// The popover's width, and its labels' (hDIi9).
const POPOVER_WIDTH: f32 = 420.0;
const AGENT_LABEL_WIDTH: f32 = 100.0;

/// The group, saving its switch itself as Sources and Code index do.
pub(super) struct AgentFiles {
  save: work::Save,
  /// What the page's last save read back, until the settings' query has it.
  kept: Kept<Read>,
  gate: Option<InputGate>,
  /// *What each CLI loads*' popover, and its link.
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for AgentFiles {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let section = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      kept: ctx.props::<TabProps>().kept.clone(),
      gate: ctx.use_context::<InputGate>(),
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    };
    work::follow(
      ctx,
      &section.save,
      ctx.props::<TabProps>().status.clone(),
      section.kept.clone(),
    );
    section
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let loaded = work::load(ctx, props.project.open.repo_root.clone());
    let Some(read) = loaded.shown(&self.kept, self.save.state().get().is_pending()) else {
      return work::pending(ctx, &loaded, bones);
    };
    let row = row(
      ctx,
      (&props, &read.0, &self.save),
      self.gate.clone(),
      (&self.open, &self.anchor),
    );
    let mut group = Group::new("settings-repository-files", ctx.t("project_settings.agentFiles")).row(row);
    if let Some(error) = work::error(ctx, &self.save) {
      group = group.after(error);
    }
    if let Some(error) = loaded.state.error {
      group = group.after(field_error("settings-files-load-error", &message_text(ctx, &error)));
    }
    group.into()
  }
}

/// The group while the settings are read (`L04L` `dFXLk`), the tab's second: its heading, the row's label and hint,
/// *What each CLI loads* and a switch's bone.
fn bones(ctx: &mut Ctx) -> Column {
  let loads = ActionButton::new(
    "settings-files-loads-reading",
    ButtonKind::Ghost,
    ctx.t("project_settings.whatEachLoads"),
  )
  .compact();
  let row = SettingRow::new("settings-files-reading", ctx.t("project_settings.readAgentFiles"))
    .info(ctx.t("project_settings.agentFilesDetail"))
    .hint(ctx.t("project_settings.agentFilesHint"))
    .control(loads)
    .control(skeleton::switch())
    .render(ctx);
  skeleton::lower_group(
    0,
    Group::new(
      "settings-repository-files-reading",
      ctx.t("project_settings.agentFiles"),
    )
    .row(row),
  )
}

fn row(
  ctx: &mut Ctx,
  (props, settings, save): (&TabProps, &Settings, &work::Save),
  gate: Option<InputGate>,
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
    toggle.on_change(move |_| {
      let may = !prompt_changes::agent_files_change_prompts(on, !on)
        || may_press(
          gate.as_ref(),
          Guarded::PermissionWidening,
          "turn on the repository's agent files, which reach every agent's prompt",
        );
      if may {
        start();
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
  SettingRow::new("settings-files", ctx.t("project_settings.readAgentFiles"))
    .info(ctx.t("project_settings.agentFilesDetail"))
    .hint(ctx.t("project_settings.agentFilesHint"))
    .control(Row::new().child(link).child(panel))
    .control(toggle)
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
