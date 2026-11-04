//! What the Orchestrator runs on, in its composer (`Runs on` `KohZb` in `01 Conversation` `wRSL4`; the picker of
//! `05 · Account and model` `v6mWp8`, `05b · On DeepSeek` `AZp3I` and `Runs-on picker · states` `t7cdi` in
//! `Orchestrator · redesign` `hujxu`). The chip (24 high, radius 6, 8 at its sides, 6 apart: a 12 `waypoints`,
//! *CLI · account · model* or *provider · model* in 11.5 `text_secondary`, and a chevron; on the `pressed` fill while
//! its picker is open; *Not set up* in `warning` on `warning_muted` until the Orchester can answer) says what the next
//! answer runs on; its tooltip, how the Orchestrator's actions are checked. It opens the picker over it, never under
//! it, and no taller than the room over it (the models scroll inside their column, the foot stays): one panel
//! ([`panel`]) with the connections on a rail ([`rail`]), the chosen one's models with Search ([`models`]) and the
//! chosen model's efforts ([`effort`]), and no list ever opens inside it. Choosing applies at once ([`picking`]): the
//! choice is published as the Orchestrator's next version, through the checks of its settings page
//! (`orchester_workspace::orchestrator::settings::save_in`), and the next answer runs on it.

mod effort;
mod header;
mod item;
mod keys;
mod models;
mod panel;
mod picking;
mod rail;

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
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::Message,
  orchestrator::{
    runs_on::{ProviderUse, RunsOn, providers_use_in},
    settings::{OrchestratorSetup, Readiness, Saving, save_in},
  },
};

use self::{
  picking::Picking,
  rail::{Sources, cli_name},
};
use crate::{
  chrome::TITLE_BAR_HEIGHT,
  services::{
    Services,
    accounts::{self, account_name},
    plan_limits,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// The chip's height (`KohZb`).
const CHIP_HEIGHT: f32 = 24.0;

/// What the chip and its picker are about: the project at `root`, what the Orchestrator runs on as read (`None` while
/// it is read or could not be), and whether reading it failed; `open` shows the picker.
#[derive(Clone)]
pub(super) struct RunsOnProps {
  pub(super) root: PathBuf,
  pub(super) project: ProjectId,
  pub(super) setup: Option<OrchestratorSetup>,
  pub(super) failed: bool,
  pub(super) open: Signal<bool>,
}

/// Compared by what it shows; the signal by which one it is.
impl PartialEq for RunsOnProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.root, self.project, &self.setup, self.failed, self.open.id())
      == (&other.root, other.project, &other.setup, other.failed, other.open.id())
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
      return self.chip(ctx, &props, None);
    };
    let sources = Sources {
      setup,
      accounts: &accounts,
      readings: &readings,
      uses: &uses,
    };
    let label = label(ctx, &sources);
    if !open {
      self.picking.forget();
      return self.chip(ctx, &props, Some(&label));
    }
    let margin = ctx.theme().spacing_value(SpacingSize::Md).to_px();
    let room = room_over(&self.anchor, (ExtraSpacing::Snug.value(), margin));
    let panel = panel::panel(ctx, (&props, &sources), (&self.picking, &self.publish), room);
    let chip = self.chip(ctx, &props, Some(&label));
    // Over the chip, 6 away from it, and never flipped under it, where it would cover the status bar: it is as tall
    // as the room over the chip lets it be, and its models scroll inside their column.
    let gap = ExtraSpacing::Snug.value();
    let picker = Popup::new(self.anchor.clone(), panel)
      .open(props.open.clone())
      .placement(Placement::TopStart)
      // Over its anchor, lurq counts the offset away from it.
      .offset(0.0, gap)
      .collision(CollisionStrategy::Clamp);
    Row::new().child(chip).child(picker).into()
  }
}

/// How tall the picker over the chip `anchor` may be: the room between the content's top under the title bar, kept
/// `margin` clear, and the chip, `gap` under the picker; `None` before the chip was laid out.
fn room_over(anchor: &ElementRef, (gap, margin): (f32, f32)) -> Option<f32> {
  (anchor.height() > 0.0).then(|| (anchor.y() - TITLE_BAR_HEIGHT - gap - margin).max(0.0))
}

impl RunsOnPicker {
  /// The chip, saying what the next answer runs on (`label`: `None` while it is read).
  fn chip(&self, ctx: &mut Ctx, props: &RunsOnProps, label: Option<&Label>) -> Element {
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
    Tooltip::new("orchestrator-runs-on-chip", ctx.t("orchestrator.runsOn.tip"))
      .silent(open)
      .wrap(ctx, chip)
  }
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
