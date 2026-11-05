//! The runs-on picker's panel (`Runs on menu` `r2owi1` in `05 · Account and model` `v6mWp8`, `x6h0l` in `05b · On
//! DeepSeek` `AZp3I`): 640 wide on `surface_raised` in a `border_strong` stroke, radius 10, lifted by the `lg` shadow.
//! Its body is three columns, top-aligned: the rail ([`super::rail`]), the models of the connection shown
//! ([`super::models`]) and the chosen model's efforts ([`super::effort`]); under them its notes and its foot
//! ([`super::foot`]). With no connection at all the picker is its foot alone, 360 wide (`State · No connections`
//! `k95WLp`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle},
  },
  components::{Column, Row},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::ConnectionId, orchestrator::OrchestratorSettings, resource::ModelId};
use orchester_workspace::orchestrator::runs_on::RunsOn;

use super::{
  Publish, RunsOnProps, effort, foot, header,
  keys::{EffortItem, Lists, keys},
  models::{self, ModelItem},
  picking::{Chooser, Part, Picking, efforts_for},
  rail::{self, Entry, Sources},
};

/// The picker's width (`r2owi1`), and with no connection at all its foot's alone (`cjnbn` in `k95WLp`).
const WIDTH: f32 = 640.0;
const FOOT_ONLY_WIDTH: f32 = 360.0;

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
  let refused = state.error.as_ref().filter(|_| picking.chose());
  let unavailable = picking
    .unavailable()
    .and_then(|connection| entries.iter().find(|entry| entry.id == connection));
  let notes = foot::notes(ctx, (sources, viewing), (refused, unavailable));
  let (body, handler) = body.unzip();
  let width = if entries.is_empty() { FOOT_ONLY_WIDTH } else { WIDTH };
  let menu = Column::new()
    .id("orchestrator-runs-on-menu")
    .width(Dimension::Px(width))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .with_children(body)
    .with_children(notes)
    .child(foot::foot(ctx, props, !entries.is_empty()));
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
    (&picking.query.get(), provider),
  );
  let efforts = model
    .as_ref()
    .map(|model| efforts(sources, entry.id, (model, provider)))
    .unwrap_or_default();
  let homes = [
    entries.iter().position(|shown| shown.id == entry.id),
    models
      .iter()
      .position(|place| matches!(place, ModelItem::Listed(listed) if Some(listed) == model.as_ref())),
    efforts.iter().position(|place| match place {
      EffortItem::Default => effort.is_none(),
      EffortItem::Listed(listed) => effort.as_ref() == Some(listed),
      EffortItem::Typed => effort.is_some(),
    }),
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

/// The places of the Effort column for `model` of `connection`: *Default* and the efforts it lists, or on an API
/// `provider` *Default* and the typed effort; none for a CLI connection that lists no efforts.
fn efforts(sources: &Sources<'_>, connection: ConnectionId, (model, provider): (&ModelId, bool)) -> Vec<EffortItem> {
  if provider {
    return vec![EffortItem::Default, EffortItem::Typed];
  }
  let listed = efforts_for(sources.setup, connection, model);
  if listed.is_empty() {
    return Vec::new();
  }
  std::iter::once(EffortItem::Default)
    .chain(listed.into_iter().map(EffortItem::Listed))
    .collect()
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
  picking.settle(ctx, lists.homes[1]);
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
