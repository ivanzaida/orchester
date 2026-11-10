//! What the Orchestrator runs on, in its composer (`Runs on` `KohZb` in `01 Conversation` `wRSL4`; the picker of
//! `05 · Account and model` `v6mWp8`, `05b · On DeepSeek` `AZp3I` and `Runs-on picker · states` `t7cdi` in
//! `Orchestrator · redesign` `hujxu`). The chip (24 high, radius 6, 8 at its sides, 6 apart: a 12 `waypoints`,
//! *CLI · account · model* or *provider · model* in 11.5 `text_secondary`, and a chevron; on the `pressed` fill while
//! its picker is open; *Not set up* in `warning` on `warning_muted` until the Orchester can answer) says what the next
//! answer runs on; its tooltip, how the Orchestrator's actions are checked, and after a switch to another account that
//! the conversation's session starts anew. It opens the shared compact picker (`ui::runs_on_picker`) over it, never
//! under it, 6 above it and aligned to its start: the connections on a rail ([`connections`] says what each shows), the
//! chosen one's models with Search and the chosen model's efforts. Choosing applies at once ([`picking`]): the choice
//! is published as the Orchestrator's next version, through the checks of its settings page
//! (`orchester_workspace::orchestrator::settings::save_in`), and the next answer runs on it. With no connection to
//! choose the picker is its foot alone ([`foot`]).

mod connections;
mod foot;
mod picking;
mod sources;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{CollisionStrategy, Ctx, FutureAction, Placement},
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::AgentAccountId, id::ProjectId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    runs_on::{ProviderUse, RunsOn, providers_use_in},
    settings::{OrchestratorSetup, Readiness, Saving, save_in},
  },
};

use self::{
  picking::{Chooser, Picking},
  sources::{Entry, Sources, cli_name},
};
use crate::{
  services::{
    Services,
    accounts::{self, account_name},
    plan_limits,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
    runs_on_picker::{Notice, NoticeTone, Pick, RunsOnPicker as Picker},
    tooltip::Tooltip,
  },
};

/// The chip's height (`KohZb`).
const CHIP_HEIGHT: f32 = 24.0;

/// What the chip and its picker are about: the project at `root`, what the Orchestrator runs on as read (`None` while
/// it is read or could not be), and whether reading it failed; `open` shows the picker. `session` is the account the
/// open conversation's session runs on, when it has one.
#[derive(Clone)]
pub(super) struct RunsOnProps {
  pub(super) root: PathBuf,
  pub(super) project: ProjectId,
  pub(super) setup: Option<OrchestratorSetup>,
  pub(super) failed: bool,
  pub(super) session: Option<AgentAccountId>,
  pub(super) open: Signal<bool>,
}

/// Compared by what it shows; the signal by which one it is.
impl PartialEq for RunsOnProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.root, self.project, &self.setup, self.failed, self.session, self.open.id())
      == (&other.root, other.project, &other.setup, other.failed, other.session, other.open.id())
  }
}

impl DevtoolsInspectable for RunsOnProps {}

/// Publishing what the picker chose.
type Publish = FutureAction<Saving, (), Message>;

pub(super) struct RunsOnPicker {
  anchor: ElementRef,
  picking: Picking,
  publish: Publish,
}

impl Component for RunsOnPicker {
  type Props = RunsOnProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let publish: Publish = ctx.future_action(move |saving: Saving| {
      let runners = services.runners.clone();
      async move {
        let root = saving.root.clone();
        background::run(move || save_in(&saving.root, &saving.settings, saving.keep))
          .await
          .and_then(|saved| saved.map(drop))?;
        // The chat reads what it runs on again.
        runners.chat_changed(&root);
        Ok(())
      }
    });
    Self {
      anchor: ElementRef::new(),
      picking: Picking::new(ctx),
      publish,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RunsOnProps>().clone();
    let accounts = accounts::listed(ctx);
    let readings = plan_limits::account_readings(ctx);
    let uses = provider_uses(ctx, &props);
    let open = props.open.get();
    let Some(setup) = props.setup.as_ref() else {
      return self.chip(ctx, &props, (None, false));
    };
    let sources = Sources {
      setup,
      accounts: &accounts,
      readings: &readings,
      uses: &uses,
    };
    let entries = sources.entries(ctx);
    let label = label(ctx, &sources);
    let switched = switched(&sources, &entries, &props);
    if !open {
      self.picking.forget();
      return self.chip(ctx, &props, (Some(&label), switched));
    }
    let panel = if entries.is_empty() {
      foot::empty_menu(ctx, &props)
    } else {
      self.picker(ctx, &props, (&sources, &entries))
    };
    let chip = self.chip(ctx, &props, (Some(&label), switched));
    // Over the chip, 6 away from it (`Runs on menu` `p3gAA1` over `v8BAOZ` in `v6mWp8`), and never flipped under it,
    // where it would cover the status bar.
    let picker = Popup::new(self.anchor.clone(), panel)
      .open(props.open.clone())
      .placement(Placement::TopStart)
      // Over its anchor, lurq counts the offset away from it.
      .offset(0.0, ExtraSpacing::Snug.value())
      .collision(CollisionStrategy::Clamp);
    Row::new().child(chip).child(picker).into()
  }
}

impl RunsOnPicker {
  /// The shared picker over `entries`, what it hands back applied through the chooser.
  fn picker(&self, ctx: &mut Ctx, props: &RunsOnProps, (sources, entries): (&Sources<'_>, &[Entry])) -> Element {
    let state = self.publish.state().get();
    let settings = self.picking.chosen(sources.setup, state.error.is_some());
    let chooser = Chooser {
      picking: self.picking.clone(),
      setup: sources.setup.clone(),
      chosen: settings.clone(),
      publish: self.publish.clone(),
      root: props.root.clone(),
    };
    let ids: Vec<_> = entries.iter().map(|entry| entry.id).collect();
    let on_pick = move |pick: Pick| match pick {
      Pick::Connection(id) => {
        if let Some(connection) = ids.iter().find(|connection| connection.to_string() == *id) {
          chooser.connection(*connection);
        }
      }
      Pick::Model { connection, model } => {
        if let Some(connection) = ids.iter().find(|candidate| candidate.to_string() == *connection) {
          chooser.model(*connection, &model);
        }
      }
      Pick::Effort(effort) => chooser.effort(effort.as_deref()),
    };
    let open = props.open.clone();
    let connections = connections::connections(ctx, sources, entries, (settings.as_ref(), props.session));
    let mut notes = Vec::new();
    if let Some(error) = state.error.as_ref().filter(|_| self.picking.chose()) {
      notes.push(Notice {
        icon: Icon::CircleAlert,
        tone: NoticeTone::Danger,
        text: ctx.t_args(
          "orchestrator.settings.saveFailed",
          [("reason", &*message_text(ctx, error))],
        ),
      });
    }
    if self.picking.rejected() {
      notes.push(Notice {
        icon: Icon::CircleAlert,
        tone: NoticeTone::Danger,
        text: ctx.t("orchestrator.runsOn.badModel"),
      });
    }
    if sources.setup.draft.is_some() {
      notes.push(Notice {
        icon: Icon::Info,
        tone: NoticeTone::Neutral,
        text: ctx.t("orchestrator.runsOn.overDraft"),
      });
    }
    Picker::new(
      "orchestrator-runs-on-menu",
      (connections, connections::chosen(settings.as_ref())),
      on_pick,
      move || open.set(false),
    )
    .notes(notes)
    .element(ctx)
  }

  /// The chip, saying what the next answer runs on (`label`: `None` while it is read); `switched` once the Orchestrator
  /// runs as another account than the conversation's session.
  fn chip(&self, ctx: &mut Ctx, props: &RunsOnProps, (label, switched): (Option<&Label>, bool)) -> Element {
    let open = props.open.get();
    let (glyph, words, warns) = match (label, props.failed) {
      (Some(Label::Ready(words)), _) => (Icon::Waypoints, words.clone(), false),
      (Some(Label::NotSetUp), _) => (Icon::TriangleAlert, ctx.t("orchestrator.runsOn.notSetUp"), true),
      (None, true) => (Icon::TriangleAlert, ctx.t("orchestrator.runsOn.unread"), true),
      (None, false) => (Icon::Waypoints, Arc::from(""), false),
    };
    let (glyph_color, words_color) = if warns {
      (PaletteColor::Warning, PaletteColor::Warning)
    } else {
      (PaletteColor::TextSecondary, PaletteColor::TextSecondary)
    };
    let toggle = props.open.clone();
    let chip = pressable(move || toggle.update(|open| *open = !*open))
      .id("orchestrator-runs-on-chip")
      .ref_element(self.anchor.clone())
      .describe("state", if open { "open" } else { "closed" })
      .describe("set_up", (!warns).to_string())
      .height(Dimension::Px(CHIP_HEIGHT))
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(glyph, IconSize::Sm, glyph_color))
      .child(
        Text::new(&words)
          .id("orchestrator-runs-on")
          .variant(ExtraTypography::Note.role())
          .color(words_color)
          .nowrap()
          .text_overflow(lurq::components::TextOverflow::Elipsis)
          .min_width(Dimension::Px(0.0)),
      )
      .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted));
    let chip = match (open, warns) {
      (true, _) => chip.background(ExtraColor::Pressed.role()),
      (false, true) => chip.background(PaletteColor::WarningMuted),
      (false, false) => chip,
    };
    let tooltip = Tooltip::new("orchestrator-runs-on-chip", ctx.t("orchestrator.runsOn.tip")).silent(open);
    let tooltip = if switched {
      tooltip.note(ctx.t("orchestrator.runsOn.newSession"))
    } else {
      tooltip
    };
    tooltip.wrap(ctx, chip)
  }
}

/// The Orchestrator runs as another account than the open conversation's session: its next answer starts a new session,
/// which the chip's tooltip says until that first answer was sent.
fn switched(sources: &Sources<'_>, entries: &[Entry], props: &RunsOnProps) -> bool {
  let chosen = sources.setup.settings.as_ref().map(|settings| settings.connection());
  let account = entries
    .iter()
    .find(|entry| Some(entry.id) == chosen)
    .and_then(|entry| connections::account_of(sources, entry));
  props.session.zip(account).is_some_and(|(session, account)| session != account)
}

/// What the chip says.
#[derive(Clone, Debug, PartialEq)]
enum Label {
  Ready(Arc<str>),
  NotSetUp,
}

/// What the published version runs on, as the chip says it: *Claude Code · Work · Opus 5.5*, *DeepSeek ·
/// deepseek-v4-pro*.
fn label(ctx: &Ctx, sources: &Sources<'_>) -> Label {
  let setup = sources.setup;
  let (Readiness::Ready { connection, model }, Some(settings)) = (setup.readiness(), setup.settings.as_ref()) else {
    return Label::NotSetUp;
  };
  let chosen = settings.connection();
  let provider = setup
    .connections()
    .into_iter()
    .find(|choice| choice.id == chosen)
    .map(|choice| choice.provider);
  let words = match (setup.runs_on(chosen), provider) {
    (Some(runs_on @ RunsOn::SignIn(_)), Some(provider)) => match sources.account(&runs_on, provider) {
      Some(account) => ctx.t_args(
        "orchestrator.runsOn.signIn",
        [
          ("cli", &*cli_name(ctx, provider)),
          ("account", &account_name(ctx, account)),
          ("model", model.as_str()),
        ],
      ),
      None => ctx.t_args(
        "orchestrator.runsOn.cli",
        [("cli", &*cli_name(ctx, provider)), ("model", model.as_str())],
      ),
    },
    _ => ctx.t_args(
      "orchestrator.runsOn.provider",
      [("provider", &*connection), ("model", model.as_str())],
    ),
  };
  Label::Ready(words)
}

/// What the project's agents spent today on each API provider the Orchestrator could run on, read again with what it
/// runs on; nothing while it is read.
fn provider_uses(ctx: &mut Ctx, props: &RunsOnProps) -> Vec<ProviderUse> {
  let providers: Vec<_> = props
    .setup
    .iter()
    .flat_map(|setup| {
      setup
        .connections()
        .into_iter()
        .filter_map(|choice| match setup.runs_on(choice.id) {
          Some(RunsOn::ApiProvider { base_url, key }) => Some((choice.id, base_url, key)),
          _ => None,
        })
    })
    .collect();
  let root = props.root.clone();
  let day = Timestamp::now().local_day_start();
  ctx
    .future((root, providers, day), |(root, providers, day)| async move {
      if providers.is_empty() {
        return Ok(Vec::new());
      }
      background::run(move || providers_use_in(&root, &providers, day))
        .await
        .and_then(|read| read)
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}
