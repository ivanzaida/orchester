//! The list's Filter (`Filter` and `Filter menu` `G4mD2U` in `02 Filtered` `MwxZG`): a ghost button that opens a 260
//! wide menu, `surface_raised` in a `border_strong` stroke, lifted by the `lg` shadow, with a search over its options
//! and one group per facet the workspace filters by in memory (`browse::Facets`): Pipeline, Priority, and Module and
//! Tag where the project has any. Each option has a checkbox; a task is kept when it has any chosen value of every
//! facet that has one. While something is chosen the button reads `Filter · 2` on `accent_muted`, and each chosen value
//! is a removable chip under the views (`Applied filters` `kmQsW`), ended by *Clear*.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ModuleId, ResourceId, TagId},
  task::organization::Priority,
};
use orchester_workspace::tasks::{browse::Facets, data::ProjectTasks};

use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    filter::{FilterChip, filter_group, filter_menu_panel, filter_option},
    icon::Icon,
    menu::menu,
    status::priority_look,
  },
};

/// The menu's width (`G4mD2U`) and the most its options take before they scroll.
const MENU_WIDTH: f32 = 260.0;
const OPTIONS_MAX_HEIGHT: f32 = 420.0;

/// One value a facet can be filtered by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
  Pipeline(Option<ResourceId>),
  Priority(Priority),
  Module(Option<ModuleId>),
  Tag(TagId),
}

impl Value {
  fn chosen(self, facets: &Facets) -> bool {
    match self {
      Self::Pipeline(pipeline) => facets.pipelines.contains(&pipeline),
      Self::Priority(priority) => facets.priorities.contains(&priority),
      Self::Module(module) => facets.modules.contains(&module),
      Self::Tag(tag) => facets.tags.contains(&tag),
    }
  }

  /// Chooses it, or lets it go when it is chosen.
  fn toggle(self, facets: &mut Facets) {
    fn flip<T: PartialEq>(values: &mut Vec<T>, value: T) {
      match values.iter().position(|held| *held == value) {
        Some(index) => {
          values.remove(index);
        }
        None => values.push(value),
      }
    }
    match self {
      Self::Pipeline(pipeline) => flip(&mut facets.pipelines, pipeline),
      Self::Priority(priority) => flip(&mut facets.priorities, priority),
      Self::Module(module) => flip(&mut facets.modules, module),
      Self::Tag(tag) => flip(&mut facets.tags, tag),
    }
  }
}

/// A facet: its name for element ids, its icon, the i18n key of its name, and its values with their words.
#[derive(Clone, Debug, PartialEq)]
struct Facet {
  name: &'static str,
  glyph: Icon,
  key: &'static str,
  values: Vec<(Value, Arc<str>)>,
}

/// The facets of a project's tasks, as the menu offers them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Options(Vec<Facet>);

impl Options {
  /// Pipeline (the published ones and none) and Priority always; Module and Tag where the project has any, so no
  /// facet keeps every task whatever is chosen.
  pub(super) fn of(ctx: &Ctx, tasks: &ProjectTasks) -> Self {
    let mut pipelines: Vec<(Value, Arc<str>)> = tasks
      .pipelines
      .iter()
      .map(|pipeline| {
        let words = ctx.t_args(
          "tasks.pipelineVersion",
          [
            ("name", pipeline.name.to_string()),
            ("version", pipeline.version.get().to_string()),
          ],
        );
        (Value::Pipeline(Some(pipeline.id)), words)
      })
      .collect();
    pipelines.push((Value::Pipeline(None), ctx.t("tasks.filter.noPipeline")));
    let priorities = Priority::ALL
      .into_iter()
      .map(|priority| (Value::Priority(priority), ctx.t(priority_look(priority).1)))
      .collect();
    let mut facets = vec![
      facet(("pipeline", Icon::Workflow, "tasks.filter.pipeline"), pipelines),
      facet(("priority", Icon::SignalHigh, "tasks.filter.priority"), priorities),
    ];
    if !tasks.modules.is_empty() {
      let mut modules: Vec<(Value, Arc<str>)> = tasks
        .modules
        .iter()
        .map(|module| (Value::Module(Some(module.id())), Arc::from(module.name().as_str())))
        .collect();
      modules.push((Value::Module(None), ctx.t("tasks.filter.noModule")));
      facets.push(facet(("module", Icon::Target, "tasks.filter.module"), modules));
    }
    if !tasks.tags.is_empty() {
      let tags = tasks
        .tags
        .iter()
        .map(|tag| (Value::Tag(tag.id()), Arc::from(tag.name().as_str())))
        .collect();
      facets.push(facet(("tag", Icon::Tag, "tasks.filter.tag"), tags));
    }
    Self(facets)
  }

  /// `chosen` without the values the menu no longer offers (a pipeline, module or tag the project dropped), which
  /// would filter with no chip to let them go.
  pub(super) fn offered(&self, chosen: &Facets) -> Facets {
    let mut kept = Facets::default();
    for (value, _) in self.0.iter().flat_map(|facet| &facet.values) {
      if value.chosen(chosen) {
        value.toggle(&mut kept);
      }
    }
    kept
  }
}

fn facet((name, glyph, key): (&'static str, Icon, &'static str), values: Vec<(Value, Arc<str>)>) -> Facet {
  Facet {
    name,
    glyph,
    key,
    values,
  }
}

#[derive(Clone)]
pub(super) struct FilterProps {
  pub facets: Signal<Facets>,
  /// `None` until the tasks are read: the button cannot be pressed then.
  pub options: Option<Arc<Options>>,
}

/// Compared by what it shows; the facets by their signal.
impl PartialEq for FilterProps {
  fn eq(&self, other: &Self) -> bool {
    self.facets.id() == other.facets.id() && self.options == other.options
  }
}

impl DevtoolsInspectable for FilterProps {}

pub(super) struct FilterControl {
  open: Signal<bool>,
  anchor: ElementRef,
  /// What the menu's search looks for among the options.
  query: Signal<String>,
}

impl Component for FilterControl {
  type Props = FilterProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
      query: ctx.signal(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FilterProps>().clone();
    let count = props.facets.get().count();
    let label = if count == 0 {
      ctx.t("tasks.filter.button")
    } else {
      ctx.t_args("tasks.filter.applied", [("count", count.to_string())])
    };
    let button = ActionButton::new("tasks-filter", ButtonKind::Ghost, label)
      .icon(Icon::ListFilter)
      .applied(count > 0)
      .element_ref(self.anchor.clone());
    let Some(options) = props.options else {
      return Element::from(Row::new().child(button));
    };
    let open = self.open.clone();
    let button = button.on_click(move || open.update(|open| *open = !*open));
    let popup = self.open.get().then(|| {
      let panel = self.panel(ctx, &options, &props.facets);
      menu(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().child(button).with_children(popup).into()
  }
}

impl FilterControl {
  /// The menu: its search, then each facet with the options the search finds.
  fn panel(&self, ctx: &mut Ctx, options: &Options, facets: &Signal<Facets>) -> Column {
    let query = self.query.get().trim().to_lowercase();
    let chosen = facets.get();
    let mut groups = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role());
    let mut found = false;
    for facet in &options.0 {
      let values: Vec<&(Value, Arc<str>)> = facet
        .values
        .iter()
        .filter(|(_, words)| query.is_empty() || words.to_lowercase().contains(&query))
        .collect();
      if values.is_empty() {
        continue;
      }
      found = true;
      groups = groups.child(filter_group(
        format!("tasks-filter-{}", facet.name),
        facet.glyph,
        &ctx.t(facet.key),
      ));
      for (index, (value, words)) in values.into_iter().enumerate() {
        let (facets, value) = (facets.clone(), *value);
        groups = groups.child(filter_option(
          format!("tasks-filter-{}-{index}", facet.name),
          (words, value.chosen(&chosen), false),
          None,
          move || facets.update(|facets| value.toggle(facets)),
        ));
      }
    }
    let search = FieldInput::new("tasks-filter-search", ctx.t("tasks.filter.search"), self.query.clone())
      .search()
      .placeholder(ctx.t("tasks.filter.search"))
      .render(ctx);
    let panel = filter_menu_panel("tasks-filter-menu", MENU_WIDTH).child(search);
    if !found {
      return panel.child(
        Row::new()
          .padding(Padding::symmetric(
            lurq::app::theme::SpacingSize::Sm,
            ExtraSpacing::Snug.role(),
          ))
          .child(
            Text::new(&ctx.t_args("tasks.filter.noMatch", [("query", self.query.get().trim())]))
              .id("tasks-filter-no-match")
              .variant(TypographyStyle::Description)
              .color(PaletteColor::TextMuted),
          ),
      );
    }
    panel.child(ScrollVertical::new(groups).max_height(Dimension::Px(OPTIONS_MAX_HEIGHT)))
  }
}

/// A chip for each chosen value, which lets it go, and *Clear*, which lets every one go; nothing while none is
/// chosen.
pub(super) fn chips(ctx: &Ctx, facets: &Signal<Facets>, options: &Options) -> Vec<Element> {
  let chosen = facets.get();
  if chosen.is_empty() {
    return Vec::new();
  }
  let mut chips: Vec<Element> = Vec::new();
  for facet in &options.0 {
    let name = ctx.t(facet.key);
    for (index, (value, words)) in facet.values.iter().enumerate() {
      if !value.chosen(&chosen) {
        continue;
      }
      let (facets, value) = (facets.clone(), *value);
      let remove = ctx.t_args("tasks.filter.remove", [("facet", &*name), ("value", &**words)]);
      chips.push(
        FilterChip::new(
          format!("tasks-chip-{}-{index}", facet.name),
          (name.clone(), words.clone()),
          remove,
          move || facets.update(|facets| value.toggle(facets)),
        )
        .into(),
      );
    }
  }
  let facets = facets.clone();
  chips.push(
    ActionButton::new("tasks-filter-clear", ButtonKind::Ghost, ctx.t("tasks.filter.clear"))
      .small()
      .on_click(move || facets.set(Facets::default()))
      .into(),
  );
  chips
}
