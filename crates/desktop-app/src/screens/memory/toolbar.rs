//! The search and filters of a Memory screen's records: *Search text, keys and tags…* and the *Type*, *Status* and
//! *Author* filters on the task's tab, in its toolbar with the count and *New record* (`Memory toolbar` `yFMam`), where
//! a filter that chooses something says what (*Type · Decisions*, *Status · All*); on the project's page one *Filter*
//! whose menu holds the *Type*, *Author* and *Tag* choices (`Search row` `E3atNZ` in 05b), the tab being the status.

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
  button::{ActionButton, ButtonKind},
  field::FieldInput,
  filter::{FilterButton, filter_group, filter_menu_panel},
  icon::Icon,
  menu::{MenuItem, menu},
};

/// The search field's width: the task's tab (`pYCO5`) and the project's page (`fkKXS` in 05b).
const TASK_SEARCH_WIDTH: f32 = 280.0;
const PROJECT_SEARCH_WIDTH: f32 = 360.0;
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
    let row = Row::new()
      .id("memory-filters")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(search.render(ctx));
    if props.project {
      return self.project_filter(ctx, &state, &filters, &props, row);
    }
    let mut row = row;
    let mut popups = Vec::new();
    for (facet, (open, anchor)) in FACETS.into_iter().zip(&self.menus) {
      if facet == Facet::Tag {
        continue;
      }
      let (label, active) = button_label(ctx, &filters, facet);
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
    for item in items(ctx, state, (filters, facet), props, &open) {
      panel = panel.child(item);
    }
    panel.into()
  }

  /// The project's page has one *Filter* (`YNR09` in 05b), its menu the *Type*, *Author* and *Tag* choices under their
  /// names; it is filled while one of them chooses something.
  fn project_filter(
    &self,
    ctx: &mut Ctx,
    state: &MemoryState,
    filters: &Filters,
    props: &FiltersProps,
    row: Row,
  ) -> Row {
    let (open, anchor) = &self.menus[0];
    let active = filters.kind.is_some() || filters.author != AuthorFilter::Any || filters.tag.is_some();
    let toggle = open.clone();
    let button = ActionButton::new(
      "memory-filter",
      if active {
        ButtonKind::Secondary
      } else {
        ButtonKind::Ghost
      },
      ctx.t("memory.filters.name"),
    )
    .icon(Icon::ListFilter)
    .element_ref(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
    let panel = if open.get() {
      let mut panel = filter_menu_panel("memory-filter-menu", MENU_WIDTH);
      for (facet, glyph, name) in [
        (Facet::Kind, Icon::ListFilter, "memory.filters.type.name"),
        (Facet::Author, Icon::User, "memory.filters.author.name"),
        (Facet::Tag, Icon::Tag, "memory.filters.tag.name"),
      ] {
        panel = panel.child(filter_group(format!("{}-group", facet_id(facet)), glyph, &ctx.t(name)));
        for item in items(ctx, state, (filters, facet), props, open) {
          panel = panel.child(item);
        }
      }
      panel.into()
    } else {
      Element::new()
    };
    row.child(button).child(menu(anchor.clone(), open.clone(), panel))
  }
}

/// `facet`'s choices as items of the menu `open` shows, the chosen one checked (`<facet>-menu-<key>`).
fn items(
  ctx: &mut Ctx,
  state: &MemoryState,
  (filters, facet): (&Filters, Facet),
  props: &FiltersProps,
  open: &Signal<bool>,
) -> Vec<Element> {
  let id = format!("{}-menu", facet_id(facet));
  choices(ctx, filters, facet, &props.tags)
    .into_iter()
    .map(|choice| {
      let (shown, change) = (state.filters.clone(), choice.change);
      MenuItem::new(format!("{id}-{}", choice.key), choice.label, move || {
        shown.update(|filters| change(filters));
      })
      .chosen(choice.chosen)
      .render(ctx, open)
    })
    .collect()
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

/// What a filter button of the task's tab says, and whether it chooses something beyond the tab's default.
fn button_label(ctx: &Ctx, filters: &Filters, facet: Facet) -> (Arc<str>, bool) {
  let chosen = |name: &str, value: &str| ctx.t_args("memory.filters.chosen", [("filter", name), ("value", value)]);
  match facet {
    Facet::Kind => match filters.kind {
      Some(kind) => (
        chosen(&ctx.t("memory.filters.type.name"), &words::kind_plural(ctx, kind)),
        true,
      ),
      None => (ctx.t("memory.filters.type.all"), false),
    },
    Facet::Status => {
      let words = words::status_filter(ctx, filters.status);
      if filters.status == StatusFilter::default() {
        (words, false)
      } else {
        (chosen(&ctx.t("memory.filters.status.name"), &words), true)
      }
    }
    Facet::Author => match filters.author {
      AuthorFilter::Any => (words::author_filter(ctx, AuthorFilter::Any), false),
      author => (
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
