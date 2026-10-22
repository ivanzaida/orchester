//! The search and filters of a Memory screen's records: *Search text, keys and tags…* and the *Type*, *Status* and
//! *Author* filters on the task's tab, in its toolbar with the count and *New record* (`Memory toolbar` `yFMam`); on
//! the project's page the *Type*, *Author* and *Tag* filters in every status tab, whose tab is the status
//! (`Toolbar` `kjxv9`). A filter that chooses something says what (*Type · Decisions*, *Status · All*).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Row,
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::MemoryKind;
use orchester_workspace::memory::listing::{AuthorFilter, Filters, KIND_ORDER, StatusFilter};

use super::{state::MemoryState, words};
use crate::ui::{
  field::FieldInput,
  filter::{FilterButton, filter_menu_panel},
  menu::{MenuItem, menu},
};

/// The search field's width: the task's tab (`pYCO5`) and the project's page (`ITShi`).
const TASK_SEARCH_WIDTH: f32 = 280.0;
const PROJECT_SEARCH_WIDTH: f32 = 260.0;
/// A filter's menu (`tEymU`).
const MENU_WIDTH: f32 = 220.0;

#[derive(Clone, PartialEq)]
pub struct FiltersProps {
  /// The project's page: *Tag* too, and its defaults' plain names (*Type*, *Author*).
  pub project: bool,
  /// The tags of the records listed, for *Tag*.
  pub tags: Arc<[String]>,
}

impl DevtoolsInspectable for FiltersProps {}

pub struct FilterBar {
  /// Each filter's menu and its button: type, status, author, tag.
  menus: [(Signal<bool>, ElementRef); 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Facet {
  Kind,
  Status,
  Author,
  Tag,
}

const FACETS: [Facet; 4] = [Facet::Kind, Facet::Status, Facet::Author, Facet::Tag];

impl Component for FilterBar {
  type Props = FiltersProps;

  fn create(ctx: &mut Ctx) -> Self {
    let mut pair = || (ctx.signal(false), ElementRef::default());
    Self {
      menus: [pair(), pair(), pair(), pair()],
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FiltersProps>().clone();
    let state = MemoryState::of(ctx);
    let filters = state.filters.get();
    let search = FieldInput::new("memory-search", ctx.t("memory.toolbar.search"), state.search.clone())
      .search()
      .placeholder(ctx.t("memory.toolbar.search"))
      .clearable(ctx.t("memory.toolbar.clearSearch"))
      .width(Dimension::Px(if props.project {
        PROJECT_SEARCH_WIDTH
      } else {
        TASK_SEARCH_WIDTH
      }));
    let mut row = Row::new()
      .id("memory-filters")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(search.render(ctx));
    let mut popups = Vec::new();
    for (facet, (open, anchor)) in FACETS.into_iter().zip(&self.menus) {
      let shown = match facet {
        Facet::Kind | Facet::Author => true,
        Facet::Status => !props.project,
        Facet::Tag => props.project,
      };
      if !shown {
        continue;
      }
      let (label, active) = button_label(ctx, &filters, facet, props.project);
      row = row.child(FilterButton::new(facet_id(facet), label, open.clone(), anchor.clone()).active(active));
      let panel = if open.get() {
        self.menu(ctx, &state, (&filters, facet), &props)
      } else {
        Element::new()
      };
      popups.push(Element::from(menu(anchor.clone(), open.clone(), panel)));
    }
    row.with_children(popups)
  }
}

impl FilterBar {
  /// The menu of `facet`: its choices, the chosen one checked.
  fn menu(
    &self,
    ctx: &mut Ctx,
    state: &MemoryState,
    (filters, facet): (&Filters, Facet),
    props: &FiltersProps,
  ) -> Element {
    let index = FACETS
      .iter()
      .position(|each| *each == facet)
      .expect("every facet has a menu");
    let open = self.menus[index].0.clone();
    let id = format!("{}-menu", facet_id(facet));
    let mut panel = filter_menu_panel(&id, MENU_WIDTH);
    for choice in choices(ctx, filters, facet, &props.tags) {
      let (shown, change) = (state.filters.clone(), choice.change);
      panel = panel.child(
        MenuItem::new(format!("{id}-{}", choice.key), choice.label, move || {
          shown.update(|filters| change(filters));
        })
        .chosen(choice.chosen)
        .render(ctx, &open),
      );
    }
    panel.into()
  }
}

/// What a choice of a filter's menu does to the filters.
type Choose = Box<dyn Fn(&mut Filters) + Send + Sync>;

/// A choice of a filter's menu: its id's end, its words, whether it is the chosen one, and what it does.
struct Choice {
  key: String,
  label: Arc<str>,
  chosen: bool,
  change: Choose,
}

/// The choices of `facet`'s menu: every value, after *All types* or *Any tag* where the filter can choose none.
fn choices(ctx: &Ctx, filters: &Filters, facet: Facet, tags: &[String]) -> Vec<Choice> {
  let choice = |key: String, label: Arc<str>, chosen: bool, change: Choose| Choice {
    key,
    label,
    chosen,
    change,
  };
  match facet {
    Facet::Kind => std::iter::once(choice(
      "all".to_owned(),
      ctx.t("memory.filters.type.all"),
      filters.kind.is_none(),
      Box::new(|filters| filters.kind = None),
    ))
    .chain(KIND_ORDER.into_iter().map(|kind: MemoryKind| {
      choice(
        kind.name().to_owned(),
        words::kind_plural(ctx, kind),
        filters.kind == Some(kind),
        Box::new(move |filters| filters.kind = Some(kind)),
      )
    }))
    .collect(),
    Facet::Status => StatusFilter::CHOICES
      .into_iter()
      .enumerate()
      .map(|(index, status)| {
        choice(
          index.to_string(),
          words::status_filter(ctx, status),
          filters.status == status,
          Box::new(move |filters| filters.status = status),
        )
      })
      .collect(),
    Facet::Author => AuthorFilter::ALL
      .into_iter()
      .enumerate()
      .map(|(index, author)| {
        choice(
          index.to_string(),
          words::author_filter(ctx, author),
          filters.author == author,
          Box::new(move |filters| filters.author = author),
        )
      })
      .collect(),
    Facet::Tag => std::iter::once(choice(
      "any".to_owned(),
      ctx.t("memory.filters.tag.any"),
      filters.tag.is_none(),
      Box::new(|filters| filters.tag = None),
    ))
    .chain(tags.iter().map(|tag| {
      let chosen = tag.clone();
      choice(
        tag.clone(),
        Arc::from(tag.as_str()),
        filters.tag.as_deref() == Some(tag.as_str()),
        Box::new(move |filters| filters.tag = Some(chosen.clone())),
      )
    }))
    .collect(),
  }
}

fn facet_id(facet: Facet) -> &'static str {
  match facet {
    Facet::Kind => "memory-filter-type",
    Facet::Status => "memory-filter-status",
    Facet::Author => "memory-filter-author",
    Facet::Tag => "memory-filter-tag",
  }
}

/// What `facet`'s button says, and whether it chooses something beyond the screen's default.
fn button_label(ctx: &Ctx, filters: &Filters, facet: Facet, project: bool) -> (Arc<str>, bool) {
  let chosen = |name: &str, value: &str| ctx.t_args("memory.filters.chosen", [("filter", name), ("value", value)]);
  match facet {
    Facet::Kind => match (filters.kind, project) {
      (Some(kind), _) => (
        chosen(&ctx.t("memory.filters.type.name"), &words::kind_plural(ctx, kind)),
        true,
      ),
      (None, true) => (ctx.t("memory.filters.type.name"), false),
      (None, false) => (ctx.t("memory.filters.type.all"), false),
    },
    Facet::Status => {
      let words = words::status_filter(ctx, filters.status);
      if filters.status == StatusFilter::default() {
        (words, false)
      } else {
        (chosen(&ctx.t("memory.filters.status.name"), &words), true)
      }
    }
    Facet::Author => match (filters.author, project) {
      (AuthorFilter::Any, true) => (ctx.t("memory.filters.author.name"), false),
      (AuthorFilter::Any, false) => (words::author_filter(ctx, AuthorFilter::Any), false),
      (author, _) => (
        chosen(&ctx.t("memory.filters.author.name"), &words::author_filter(ctx, author)),
        true,
      ),
    },
    Facet::Tag => match &filters.tag {
      Some(tag) => (chosen(&ctx.t("memory.filters.tag.name"), tag), true),
      None => (ctx.t("memory.filters.tag.name"), false),
    },
  }
}
