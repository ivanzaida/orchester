//! The filter chips (`C2pTYF`, `e5PbG`, `YxAed`): *Project* (across projects), *CLI*, *Model*,
//! *Agent profile*, *Pipeline* and *Tag*, each a multi-select menu, with search where the list is
//! long (*Model*). A chip shows *All*, the one value chosen or *2 of 4*; an active chip is on
//! `$hover` with a `$subtle` stroke and a × that clears it. *Tag* lists tags with their run counts
//! (*No tags yet* until tasks have tags) and matches any tag chosen.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, OutsidePress, Placement},
    events::TextInputEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::AgentProvider,
  id::ProjectId,
  usage::{FilterOptions, UsageFilter, toggle},
};

use super::{
  card::text_link,
  format,
  state::{Controls, Dashboard},
};
use crate::{
  theme::{ExtraRadius, ExtraShadow, ExtraSpacing},
  ui::{
    field::text_input,
    icon::{Icon, IconSize, icon},
    link::Press,
    menu_option::menu_option,
    pressable::{pressable, surface},
  },
};

/// A chip's height (`C2pTYF`) and a menu's width.
const CHIP_HEIGHT: f32 = 30.0;
const MENU_WIDTH: f32 = 280.0;
/// A list longer than this gets a search field.
const SEARCH_FROM: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Facet {
  Project,
  Cli,
  Model,
  Profile,
  Pipeline,
  Tag,
}

impl Facet {
  const fn key(self) -> &'static str {
    match self {
      Self::Project => "usage.filters.project",
      Self::Cli => "usage.filters.cli",
      Self::Model => "usage.filters.model",
      Self::Profile => "usage.filters.profile",
      Self::Pipeline => "usage.filters.pipeline",
      Self::Tag => "usage.filters.tag",
    }
  }

  const fn id(self) -> &'static str {
    match self {
      Self::Project => "usage-filter-project",
      Self::Cli => "usage-filter-cli",
      Self::Model => "usage-filter-model",
      Self::Profile => "usage-filter-profile",
      Self::Pipeline => "usage-filter-pipeline",
      Self::Tag => "usage-filter-tag",
    }
  }
}

/// A value a filter can choose.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
  Project(ProjectId),
  Cli(AgentProvider),
  Model(Arc<str>),
  Profile(Arc<str>),
  Pipeline(Arc<str>),
  Tag(Arc<str>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OpenFacet(Option<Facet>);

impl lurq::app::component::DevtoolsInspectable for OpenFacet {}

#[derive(Clone)]
pub struct FilterProps {
  pub controls: Signal<Controls>,
  pub dashboard: Option<Arc<Dashboard>>,
}

impl PartialEq for FilterProps {
  fn eq(&self, other: &Self) -> bool {
    self.dashboard == other.dashboard
  }
}

impl lurq::app::component::DevtoolsInspectable for FilterProps {}

pub struct FilterChips {
  open: Signal<OpenFacet>,
  show: Signal<bool>,
  anchors: [ElementRef; 6],
  query: Signal<String>,
}

impl Component for FilterChips {
  type Props = FilterProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(OpenFacet::default()),
      show: ctx.signal(false),
      anchors: std::array::from_fn(|_| ElementRef::default()),
      query: ctx.signal(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FilterProps>().clone();
    let filter = props.controls.get().filter;
    let options = props.dashboard.as_ref().map_or_else(
      || Arc::new(FilterOptions::default()),
      |dashboard| Arc::clone(&dashboard.options),
    );
    let all = props
      .dashboard
      .as_ref()
      .is_some_and(|dashboard| dashboard.all_projects());
    let facets = [
      Facet::Project,
      Facet::Cli,
      Facet::Model,
      Facet::Profile,
      Facet::Pipeline,
      Facet::Tag,
    ];
    let chips = facets
      .into_iter()
      .enumerate()
      .filter(|(_, facet)| all || *facet != Facet::Project)
      .map(|(index, facet)| self.chip(ctx, (facet, index), &filter, &options, &props.controls));
    let controls = props.controls.clone();
    let clear = (!filter.is_empty()).then(|| {
      text_link(
        "usage-filters-clear",
        &ctx.t("usage.filters.clear"),
        Press::action(move || controls.update(|controls| controls.filter = UsageFilter::default())),
      )
    });
    let mut row = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(chips)
      .with_children(clear);
    if let Some(facet) = self.open.get().0 {
      let index = facets.iter().position(|candidate| *candidate == facet).unwrap_or(0);
      let menu = self.menu(ctx, facet, &filter, &options, &props.controls);
      row = row.child(
        Popup::new(self.anchors[index].clone(), menu)
          .open(self.show.clone())
          .placement(Placement::BottomStart)
          .outside_press(OutsidePress::PassThrough),
      );
    }
    row
  }
}

impl FilterChips {
  /// A chip: the facet's name and what it chose; a × clears an active one.
  fn chip(
    &self,
    ctx: &Ctx,
    (facet, index): (Facet, usize),
    filter: &UsageFilter,
    options: &FilterOptions,
    controls: &Signal<Controls>,
  ) -> Element {
    let chosen = chosen(filter, facet);
    let total = values(ctx, facet, options).len();
    let value = match chosen.as_slice() {
      [] => ctx.t("usage.filters.all"),
      [only] => name(ctx, only, options),
      many => ctx.t_args(
        "usage.filters.someOf",
        [
          ("count", many.len().to_string().as_str()),
          ("total", total.to_string().as_str()),
        ],
      ),
    };
    let active = !chosen.is_empty();
    let (open, show, query) = (self.open.clone(), self.show.clone(), self.query.clone());
    let label = pressable(surface().id(facet.id()).height(Dimension::Px(CHIP_HEIGHT)), move || {
      query.set(String::new());
      open.set(OpenFacet(Some(facet)));
      show.set(true);
    })
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t(facet.key()))
            .variant(TypographyStyle::Caption)
            .nowrap(),
        )
        .child(Text::new(&value).variant(TypographyStyle::Link).nowrap())
        .with_children((!active).then(|| icon(Icon::ChevronDown, IconSize::Small, PaletteColor::TextSecondary))),
    );
    let controls = controls.clone();
    let clear = active.then(|| {
      Element::from(
        pressable(surface().id(format!("{}-clear", facet.id())), move || {
          controls.update(|controls| clear_facet(&mut controls.filter, facet));
        })
        .align_items(Alignment::Center)
        .child(icon(Icon::X, IconSize::Small, PaletteColor::TextPrimary)),
      )
    });
    let (fill, stroke) = if active {
      (PaletteColor::AccentMuted, PaletteColor::BorderStrong)
    } else {
      (PaletteColor::SurfacePanel, PaletteColor::Border)
    };
    Row::new()
      .ref_element(self.anchors[index].clone())
      .height(Dimension::Px(CHIP_HEIGHT))
      .padding_horizontal(ExtraSpacing::Inline.role())
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .background(fill)
      .border_inside(BorderSize::Sm, stroke)
      .rounded(RadiusSize::Md)
      .describe("active", active.to_string())
      .child(label)
      .with_children(clear)
      .into()
  }

  /// The facet's menu: its values to check, searchable when the list is long.
  fn menu(
    &self,
    ctx: &Ctx,
    facet: Facet,
    filter: &UsageFilter,
    options: &FilterOptions,
    controls: &Signal<Controls>,
  ) -> Element {
    let all = values(ctx, facet, options);
    let searchable = facet == Facet::Model || all.len() > SEARCH_FROM;
    let query = self.query.get().to_lowercase();
    let picked = chosen(filter, facet);
    let rows: Vec<Element> = all
      .into_iter()
      .filter(|(_, label)| query.is_empty() || label.to_lowercase().contains(&query))
      .map(|(value, label)| {
        let (controls, checked) = (controls.clone(), picked.contains(&value));
        menu_option(&label, checked, true, move || {
          controls.update(|controls| toggle_value(&mut controls.filter, value.clone()));
        })
      })
      .collect();
    let empty = rows.is_empty().then(|| {
      let key = match facet {
        Facet::Tag if options.tags.is_empty() => "usage.filters.noTags",
        _ if !query.is_empty() => "usage.filters.noMatch",
        _ => "usage.filters.nothing",
      };
      Element::from(Text::new(&ctx.t(key)).variant(TypographyStyle::Caption))
    });
    let search = searchable.then(|| {
      let input = self.query.clone();
      Element::from(
        text_input(
          ctx,
          "usage-filter-search",
          self.query.clone(),
          false,
          TypographyStyle::Body,
        )
        .placeholder(&ctx.t("usage.filters.search"))
        .on_input(move |event: TextInputEvent| input.set(event.new_value)),
      )
    });
    Column::new()
      .id("usage-filter-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .padding(ExtraSpacing::SelectMenu.role())
      .spacing(ExtraSpacing::Hair.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::SelectMenu.role())
      .box_shadow(ExtraShadow::Popover.role())
      .with_children(search)
      .with_children(rows)
      .with_children(empty)
      .into()
  }
}

/// What the facet offers, with each value's label; tags with their run counts.
fn values(ctx: &Ctx, facet: Facet, options: &FilterOptions) -> Vec<(Value, Arc<str>)> {
  let texts = |values: &[Arc<str>], value: fn(Arc<str>) -> Value| {
    values.iter().map(|text| (value(text.clone()), text.clone())).collect()
  };
  match facet {
    Facet::Project => options
      .projects
      .iter()
      .map(|(id, name)| (Value::Project(*id), name.clone()))
      .collect(),
    Facet::Cli => options
      .clis
      .iter()
      .map(|cli| (Value::Cli(*cli), format::cli_name(ctx, *cli)))
      .collect(),
    Facet::Model => texts(&options.models, Value::Model),
    Facet::Profile => texts(&options.profiles, Value::Profile),
    Facet::Pipeline => texts(&options.pipelines, Value::Pipeline),
    Facet::Tag => options
      .tags
      .iter()
      .map(|(tag, runs)| {
        let label = ctx.t_args(
          "usage.filters.tagRuns",
          [("tag", &**tag), ("count", runs.to_string().as_str())],
        );
        (Value::Tag(tag.clone()), label)
      })
      .collect(),
  }
}

/// A chosen value's name in a chip.
fn name(ctx: &Ctx, value: &Value, options: &FilterOptions) -> Arc<str> {
  match value {
    Value::Project(id) => options
      .projects
      .iter()
      .find(|(project, _)| project == id)
      .map_or_else(|| ctx.t("usage.unknown"), |(_, name)| name.clone()),
    Value::Cli(cli) => format::cli_name(ctx, *cli),
    Value::Model(text) | Value::Profile(text) | Value::Pipeline(text) | Value::Tag(text) => text.clone(),
  }
}

fn chosen(filter: &UsageFilter, facet: Facet) -> Vec<Value> {
  let texts = |values: &[Arc<str>], value: fn(Arc<str>) -> Value| values.iter().cloned().map(value).collect();
  match facet {
    Facet::Project => filter.projects.iter().copied().map(Value::Project).collect(),
    Facet::Cli => filter.clis.iter().copied().map(Value::Cli).collect(),
    Facet::Model => texts(&filter.models, Value::Model),
    Facet::Profile => texts(&filter.profiles, Value::Profile),
    Facet::Pipeline => texts(&filter.pipelines, Value::Pipeline),
    Facet::Tag => texts(&filter.tags, Value::Tag),
  }
}

fn clear_facet(filter: &mut UsageFilter, facet: Facet) {
  match facet {
    Facet::Project => filter.projects.clear(),
    Facet::Cli => filter.clis.clear(),
    Facet::Model => filter.models.clear(),
    Facet::Profile => filter.profiles.clear(),
    Facet::Pipeline => filter.pipelines.clear(),
    Facet::Tag => filter.tags.clear(),
  }
}

fn toggle_value(filter: &mut UsageFilter, value: Value) {
  match value {
    Value::Project(id) => toggle(&mut filter.projects, id),
    Value::Cli(cli) => toggle(&mut filter.clis, cli),
    Value::Model(model) => toggle(&mut filter.models, model),
    Value::Profile(profile) => toggle(&mut filter.profiles, profile),
    Value::Pipeline(pipeline) => toggle(&mut filter.pipelines, pipeline),
    Value::Tag(tag) => toggle(&mut filter.tags, tag),
  }
}

/// The active filters named (*CLI: Claude, Pipeline: Docs update*), for *No usage matches these
/// filters* (`YxAed`).
pub fn active_filters(ctx: &Ctx, dashboard: &Dashboard) -> String {
  let separator = ctx.t("usage.listSeparator");
  let named: Vec<String> = [
    Facet::Project,
    Facet::Cli,
    Facet::Model,
    Facet::Profile,
    Facet::Pipeline,
    Facet::Tag,
  ]
  .into_iter()
  .filter_map(|facet| {
    let chosen = chosen(&dashboard.filter, facet);
    (!chosen.is_empty()).then(|| {
      let names: Vec<Arc<str>> = chosen
        .iter()
        .map(|value| name(ctx, value, &dashboard.options))
        .collect();
      ctx
        .t_args(
          "usage.filters.named",
          [
            ("facet", &*ctx.t(facet.key())),
            ("values", names.join(&separator).as_str()),
          ],
        )
        .to_string()
    })
  })
  .collect();
  named.join(&separator)
}
