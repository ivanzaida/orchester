//! The toolbar's filters (`Filters` `N20RN`, `Usage · interactions` `p732q`): a button per filter
//! (*Account*, *Model*, *Agent profile*, *Pipeline*, *Tag*) that says how much it chooses (*Account · 2 of 6*)
//! and opens its menu of checkboxes, with a search where the list is long; *Account*'s groups the connected
//! accounts by CLI, each with its series' swatch, then *No account* (`EU0ex`). Then a chip per chosen value,
//! from a menu, a *Where tokens go* row or a link (a CLI, a task, a run or a step a link may add too), and
//! *Clear filters*. What a menu offers and what a choice does is `orchester_workspace::usage`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::usage::{AccountLabel, Dashboard, Facet, FilterOption, ValueLabel};

use super::{
  Requery,
  series::{account_name, account_text, cli_heading, series_swatch},
  words,
};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    filter::{FilterChip, filter_group, filter_menu_panel, filter_option, filter_option_explained},
    icon::{Icon, icon},
    menu::separator,
    state::loading,
  },
};

/// A menu's width (`CLI menu` `tEymU`), with a search (`Model menu` `dcY35`), and *Account*'s (`JoCR3`).
const MENU_WIDTH: f32 = 220.0;
const ACCOUNT_MENU_WIDTH: f32 = 260.0;
const SEARCH_MENU_WIDTH: f32 = 240.0;
/// A list longer than this gets a search; *Model* always has one (`Model filter · search in a long list`).
const SEARCH_FROM: usize = 8;

/// The words of a filter's name.
pub fn facet_name(ctx: &Ctx, facet: Facet) -> Arc<str> {
  ctx.t(match facet {
    Facet::Project => "usage.filters.project",
    Facet::Cli => "usage.filters.cli",
    Facet::Model => "usage.filters.model",
    Facet::Profile => "usage.filters.profile",
    Facet::Pipeline => "usage.filters.pipeline",
    Facet::Tag => "usage.filters.tag",
    Facet::Task => "usage.filters.task",
    Facet::Run => "usage.filters.run",
    Facet::Step => "usage.filters.step",
    Facet::Account => "usage.filters.account",
  })
}

/// A filter's element id: `usage-filter-model`.
pub const fn facet_id(facet: Facet) -> &'static str {
  match facet {
    Facet::Project => "usage-filter-project",
    Facet::Cli => "usage-filter-cli",
    Facet::Model => "usage-filter-model",
    Facet::Profile => "usage-filter-profile",
    Facet::Pipeline => "usage-filter-pipeline",
    Facet::Tag => "usage-filter-tag",
    Facet::Task => "usage-filter-task",
    Facet::Run => "usage-filter-run",
    Facet::Step => "usage-filter-step",
    Facet::Account => "usage-filter-account",
  }
}

/// How a value reads.
pub fn value_text(ctx: &Ctx, label: &ValueLabel) -> Arc<str> {
  match label {
    ValueLabel::Cli(cli) => words::cli_name(ctx, *cli),
    ValueLabel::Name(name) | ValueLabel::Unknown(name) => name.clone(),
    ValueLabel::Pipeline { name, version } => words::pipeline(ctx, name, *version),
    ValueLabel::Run { task, number } => words::run_label(ctx, task, number.get()),
    ValueLabel::Account { cli, name } => account_text(
      ctx,
      &AccountLabel::Known {
        cli: *cli,
        name: name.clone(),
      },
    ),
    ValueLabel::NoAccount => ctx.t("usage.accounts.none"),
  }
}

/// A filter button's words and whether it chooses anything: *Model*, or *Model · 1 of 5*.
pub fn button_label(ctx: &Ctx, dashboard: Option<&Dashboard>, facet: Facet) -> (Arc<str>, bool) {
  let name = facet_name(ctx, facet);
  let Some(dashboard) = dashboard else {
    return (name, false);
  };
  let chosen = dashboard.chosen(facet).len();
  if chosen == 0 {
    return (name, false);
  }
  let total = dashboard.options(facet).len().max(chosen);
  let label = ctx.t_args(
    "usage.filters.someOf",
    [
      ("filter", &*name),
      ("count", chosen.to_string().as_str()),
      ("total", total.to_string().as_str()),
    ],
  );
  (label, true)
}

/// The menu of `facet`: its values to check, searchable when the list is long, or what it reads.
pub fn menu(
  ctx: &mut Ctx,
  dashboard: Option<&Dashboard>,
  facet: Facet,
  search: &Signal<String>,
  requery: &Requery,
) -> Element {
  let id = format!("{}-menu", facet_id(facet));
  let Some(dashboard) = dashboard else {
    return filter_menu_panel(&id, MENU_WIDTH)
      .child(loading(&format!("{id}-reading"), &ctx.t("usage.filters.reading")).padding(SpacingSize::Sm))
      .into();
  };
  if facet == Facet::Account {
    return account_menu(ctx, dashboard, &id, requery);
  }
  let options = dashboard.options(facet);
  let searchable = facet == Facet::Model || options.len() > SEARCH_FROM;
  let query = search.get().to_lowercase();
  let rows: Vec<Element> = options
    .iter()
    .enumerate()
    .filter(|(_, option)| query.is_empty() || value_text(ctx, &option.label).to_lowercase().contains(&query))
    .map(|(index, option)| option_row(ctx, dashboard, (facet, index), option, requery))
    .collect();
  let empty = rows.is_empty().then(|| empty_line(ctx, &id, facet, options.is_empty()));
  let field = searchable.then(|| {
    FieldInput::new(format!("{id}-search"), ctx.t("usage.filters.search"), search.clone())
      .search()
      .placeholder(ctx.t("usage.filters.search"))
      .render(ctx)
  });
  let width = if searchable { SEARCH_MENU_WIDTH } else { MENU_WIDTH };
  filter_menu_panel(&id, width)
    .with_children(field)
    .with_children(rows)
    .with_children(empty)
    .into()
}

fn option_row(
  ctx: &Ctx,
  dashboard: &Dashboard,
  (facet, index): (Facet, usize),
  option: &FilterOption,
  requery: &Requery,
) -> Element {
  let label = match option.runs {
    Some(runs) => ctx.t_args(
      "usage.filters.tagRuns",
      [
        ("tag", &*value_text(ctx, &option.label)),
        ("count", runs.to_string().as_str()),
      ],
    ),
    None => value_text(ctx, &option.label),
  };
  let swatch = option.series.map(series_swatch);
  let (requery, query) = (requery.clone(), dashboard.toggled(facet, &option.value));
  filter_option(
    format!("{}-option-{index}", facet_id(facet)),
    (&label, option.chosen, facet == Facet::Model),
    swatch,
    move || requery.show(query.clone()),
  )
}

/// *Account*'s menu (`Account menu` `JoCR3`): each CLI's heading over its accounts, by name as Settings
/// shows them, each with its series' swatch; then, past a line, *No account* and what it holds.
fn account_menu(ctx: &mut Ctx, dashboard: &Dashboard, id: &str, requery: &Requery) -> Element {
  let mut rows: Vec<Element> = Vec::new();
  let mut heading = None;
  for (index, option) in dashboard.options(Facet::Account).into_iter().enumerate() {
    let (requery, query) = (requery.clone(), dashboard.toggled(Facet::Account, &option.value));
    let choose = move || requery.show(query.clone());
    let option_id = format!("{}-option-{index}", facet_id(Facet::Account));
    let swatch = option.series.map(series_swatch);
    match &option.label {
      ValueLabel::Account { cli, name } => {
        if heading != Some(*cli) {
          heading = Some(*cli);
          rows.push(filter_group(
            format!("{id}-group-{}", cli.as_str()),
            Icon::SquareTerminal,
            &cli_heading(ctx, *cli),
          ));
        }
        let label = account_name(ctx, name.as_ref());
        rows.push(filter_option(option_id, (&label, option.chosen, false), swatch, choose));
      }
      ValueLabel::NoAccount => {
        rows.push(separator(ctx));
        rows.push(filter_option_explained(
          option_id,
          (
            &ctx.t("usage.accounts.none"),
            &ctx.t("usage.accounts.noneDetail"),
            option.chosen,
          ),
          swatch,
          choose,
        ));
      }
      label => {
        let label = value_text(ctx, label);
        rows.push(filter_option(option_id, (&label, option.chosen, false), swatch, choose));
      }
    }
  }
  filter_menu_panel(id, ACCOUNT_MENU_WIDTH).with_children(rows).into()
}

/// What a menu says when it lists nothing: *No tags yet* for a project whose tasks have no tags
/// (`Tag menu` `jN6vp`), no match for a search, or nothing in the period.
fn empty_line(ctx: &Ctx, id: &str, facet: Facet, none: bool) -> Element {
  let (glyph, key) = match (facet, none) {
    (Facet::Tag, true) => (Icon::Tag, "usage.filters.noTags"),
    (_, true) => (Icon::Search, "usage.filters.nothing"),
    (_, false) => (Icon::Search, "usage.filters.noMatch"),
  };
  Row::new()
    .id(format!("{id}-empty"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// A chip per chosen value of every filter, then *Clear filters*; nothing while nothing is chosen.
pub fn chips(ctx: &Ctx, dashboard: &Dashboard, requery: &Requery) -> Vec<Element> {
  let mut chips: Vec<Element> = Facet::ALL
    .into_iter()
    .flat_map(|facet| {
      dashboard
        .chosen(facet)
        .into_iter()
        .enumerate()
        .map(move |(index, value)| (facet, index, value))
    })
    .map(|(facet, index, (value, label))| {
      let (name, text) = (facet_name(ctx, facet), value_text(ctx, &label));
      let remove = ctx.t_args("usage.filters.remove", [("filter", &*name), ("value", &*text)]);
      let (requery, query) = (requery.clone(), dashboard.without(facet, &value));
      FilterChip::new(
        format!("{}-chip-{index}", facet_id(facet)),
        (name, text),
        remove,
        move || requery.show(query.clone()),
      )
      .into()
    })
    .collect();
  if !chips.is_empty() {
    let (requery, query) = (requery.clone(), dashboard.unfiltered());
    chips.push(
      ActionButton::new("usage-clear-filters", ButtonKind::Ghost, ctx.t("usage.filters.clear"))
        .icon(Icon::X)
        .on_click(move || requery.show(query.clone()))
        .into(),
    );
  }
  chips
}

/// The chosen filters named, for *No usage matches these filters*: *Model: claude-opus-5-5, Pipeline:
/// Quick fix v2*.
pub fn described(ctx: &Ctx, dashboard: &Dashboard) -> String {
  let separator = ctx.t("usage.listSeparator");
  Facet::ALL
    .into_iter()
    .filter_map(|facet| {
      let chosen = dashboard.chosen(facet);
      (!chosen.is_empty()).then(|| {
        let values: Vec<Arc<str>> = chosen.iter().map(|(_, label)| value_text(ctx, label)).collect();
        ctx
          .t_args(
            "usage.filters.named",
            [
              ("filter", &*facet_name(ctx, facet)),
              ("values", values.join(&separator).as_str()),
            ],
          )
          .to_string()
      })
    })
    .collect::<Vec<_>>()
    .join(&separator)
}
