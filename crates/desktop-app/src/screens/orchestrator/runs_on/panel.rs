//! The runs-on picker's panel (`Runs on menu` `r2owi1` in `05 · Account and model` `v6mWp8`, `x6h0l` in `05b · On
//! DeepSeek` `AZp3I`): 640 wide on `surface_raised` in a `border_strong` stroke, radius 10, lifted by the `lg` shadow.
//! Its body is three columns, top-aligned: the rail ([`super::rail`]), the models of the connection shown
//! ([`super::models`]) and the chosen model's efforts ([`super::effort`]). Under them, 8 from the sides and the foot:
//! why the last choice was refused, on an API provider where prompts and code go (11.5 `text_secondary` beside a 12
//! `send` in `warning`, on `warning_muted`, radius 6, 8 by 10), and that choosing replaces a draft saved in the
//! Orchestrator's settings. Its foot, 8 by 12 under a 1 `border` line, says that another account starts a new session
//! and offers *Settings ↗*; with no connection at all the picker is its foot alone, with no line over it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::runs_on::RunsOn;

use super::{
  Publish, RunsOnProps, effort, header,
  keys::keys,
  models::{self, ModelItem},
  picking::{Chooser, Lists, Part, Picking, efforts_for},
  rail::{self, Entry, Sources},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    tinted_note::tinted_note,
  },
};

/// The picker's width (`r2owi1`) and its foot's line's height (the top stroke of `Foot` `SwrjI`).
const WIDTH: f32 = 640.0;
const RULE_HEIGHT: f32 = 1.0;

/// The panel over the chip, no taller than `room` when it is known.
pub(super) fn panel(
  ctx: &mut Ctx,
  (props, sources): (&RunsOnProps, &Sources<'_>),
  (picking, publish): (&Picking, &Publish),
  room: Option<f32>,
) -> Element {
  let entries = sources.entries(ctx);
  picking.seed(sources.setup, &entries);
  let state = publish.state().get();
  let chooser = Chooser {
    picking: picking.clone(),
    setup: sources.setup.clone(),
    chosen: picking.chosen(sources.setup, state.error.is_some()),
    publish: publish.clone(),
    root: props.root.clone(),
  };
  let viewing = picking
    .viewing()
    .and_then(|viewing| entries.iter().find(|entry| entry.id == viewing));
  let body = viewing.map(|entry| body(ctx, sources, (&entries, entry), &chooser));
  let refused = state.error.as_ref().filter(|_| picking.chose()).map(|error| {
    tinted_note(
      "orchestrator-runs-on-error",
      (Icon::CircleAlert, PaletteColor::Danger),
      &ctx.t_args(
        "orchestrator.settings.saveFailed",
        [("reason", &*message_text(ctx, error))],
      ),
    )
    .into()
  });
  let notes = notes(ctx, (sources, viewing), refused);
  let (body, handler) = body.unzip();
  picking.take_focus(ctx);
  let menu = Column::new()
    .id("orchestrator-runs-on-menu")
    .width(Dimension::Px(WIDTH))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .with_children(body)
    .with_children(notes)
    .with_children((!entries.is_empty()).then(rule))
    .child(foot(ctx, props));
  let menu = match handler {
    Some(handler) => menu.on_key_down(handler),
    None => menu,
  };
  match room {
    Some(room) => menu.max_height(Dimension::Px(room)).into(),
    None => menu.into(),
  }
}

/// What the columns show of `entry`: what the keys move over, the model and effort chosen on it, and whether it is an
/// API provider's.
struct Shown {
  lists: Lists,
  model: Option<ModelId>,
  effort: Option<String>,
  provider: bool,
}

/// What the columns show of `entry` among `entries`.
fn shown(sources: &Sources<'_>, (entries, entry): (&[Entry], &Entry), chooser: &Chooser) -> Shown {
  let picking = &chooser.picking;
  let chosen = chooser.chosen_on(entry.id);
  let model = chosen.map(|chosen| chosen.model().clone());
  let effort = chosen.and_then(|chosen| chosen.effort().map(str::to_owned));
  let provider = matches!(entry.runs_on, RunsOn::ApiProvider { .. });
  let models = models::items(
    sources.setup,
    (entry.id, model.as_ref()),
    (&picking.query.get(), picking.all.get(), provider),
  );
  let efforts: Vec<Option<String>> = model
    .as_ref()
    .map(|model| efforts_for(sources.setup, entry.id, model))
    .filter(|efforts| !efforts.is_empty())
    .map(|efforts| std::iter::once(None).chain(efforts.into_iter().map(Some)).collect())
    .unwrap_or_default();
  let homes = [
    entries.iter().position(|shown| shown.id == entry.id),
    models
      .iter()
      .position(|place| matches!(place, ModelItem::Listed(listed) if Some(listed) == model.as_ref())),
    efforts.iter().position(|listed| *listed == effort),
  ];
  Shown {
    lists: Lists {
      rail: entries.iter().map(|entry| (entry.id, entry.choosable)).collect(),
      models,
      efforts,
      homes,
    },
    model,
    effort,
    provider,
  }
}

/// The three columns for `entries`, `entry`'s models shown, and what the keys do over them.
fn body(
  ctx: &mut Ctx,
  sources: &Sources<'_>,
  (entries, entry): (&[Entry], &Entry),
  chooser: &Chooser,
) -> (Element, impl Fn(KeyboardEvent) + Send + Sync + 'static + use<>) {
  let shown = shown(sources, (entries, entry), chooser);
  let picking = &chooser.picking;
  let lists = &shown.lists;
  let choose: Arc<dyn Fn(ConnectionId) + Send + Sync> = {
    let chooser = chooser.clone();
    Arc::new(move |connection| chooser.connection(connection))
  };
  let chosen = chooser.chosen.as_ref().map(OrchestratorSettings::connection);
  let rail = rail::rail(
    ctx,
    entries,
    (chosen, picking.cursor(Part::Rail, lists.rail.len())),
    &choose,
  );
  let head = header::header(ctx, sources, entry);
  let models = models::column(
    ctx,
    (head, &lists.models),
    (
      shown.model.as_ref(),
      picking.cursor(Part::Models, lists.models.len()),
      shown.provider,
    ),
    chooser,
  );
  let efforts = shown.model.as_ref().filter(|_| !lists.efforts.is_empty()).map(|model| {
    effort::column(
      ctx,
      (model, &lists.efforts),
      (
        shown.effort.as_deref(),
        picking.cursor(Part::Effort, lists.efforts.len()),
      ),
      chooser,
    )
  });
  let body = Row::new()
    .id("orchestrator-runs-on-body")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .min_height(Dimension::Px(0.0))
    .align_items(Alignment::Start)
    .child(rail)
    .child(models)
    .with_children(efforts);
  (body.into(), keys(chooser, shown.lists))
}

/// The notes under the columns: why the last choice was `refused`, where an API provider shown sends prompts, and that
/// choosing replaces a draft; nothing when there is none.
fn notes(ctx: &Ctx, (sources, viewing): (&Sources<'_>, Option<&Entry>), refused: Option<Element>) -> Option<Element> {
  let sent_to = viewing.and_then(|entry| match &entry.runs_on {
    RunsOn::ApiProvider { base_url, .. } => Some(sent_to(ctx, base_url.host())),
    RunsOn::SignIn(_) | RunsOn::Native => None,
  });
  let over_draft = (viewing.is_some() && sources.setup.draft.is_some()).then(|| {
    tinted_note(
      "orchestrator-runs-on-over-draft",
      (Icon::Info, PaletteColor::TextMuted),
      &ctx.t("orchestrator.runsOn.overDraft"),
    )
    .into()
  });
  let notes: Vec<Element> = refused.into_iter().chain(sent_to).chain(over_draft).collect();
  (!notes.is_empty()).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex_shrink(0.0)
      .padding(
        Padding::new()
          .left(SpacingSize::Sm)
          .right(SpacingSize::Sm)
          .bottom(SpacingSize::Sm),
      )
      .spacing(ExtraSpacing::Snug.role())
      .with_children(notes)
      .into()
  })
}

/// Where an API provider at `host` sends prompts, repository content and command output.
fn sent_to(ctx: &Ctx, host: &str) -> Element {
  Row::new()
    .id("orchestrator-runs-on-sent-to")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .background(PaletteColor::WarningMuted)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::Send, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t_args("orchestrator.runsOn.sentTo", [("host", host)]))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The `border` line over the foot, 1 high: only when something is over it.
fn rule() -> Element {
  Row::new()
    .id("orchestrator-runs-on-rule")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(RULE_HEIGHT))
    .flex_shrink(0.0)
    .background(PaletteColor::Border)
    .into()
}

/// The picker's foot: that another account starts a new session, and *Settings ↗*.
fn foot(ctx: &mut Ctx, props: &RunsOnProps) -> Element {
  let navigation = Navigation::of(ctx);
  let settings = opening(navigation.to(Route::Project(props.project, ProjectPage::OrchestratorSettings)))
    .id("orchestrator-settings")
    .describe("a11y_name", ctx.t("orchestrator.runsOn.settings"))
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.settings"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    );
  Row::new()
    .id("orchestrator-runs-on-foot")
    .width(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("orchestrator.runsOn.newSession"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(settings)
    .into()
}
