//! Select resources (06.04 `W4zGKx`, body `sxeK1`): the project to import from and this project,
//! the kind tabs, the search, and the source's pipelines, agent profiles, instructions and tools.
//! Secrets are not imported: a secret's value stays in its own project.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Select, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::import::{CatalogResource, Choices, Plan};
use orchester_store::RegisteredProject;

use super::{
  BUTTON_HEIGHT, ImportDialog, ImportProps, Picked, Source, SourceChoice, Stage, beside_gutter, body, footer_bar,
  labels,
  resource_row::{PickValue, ResourceRow, ResourceRowProps},
};
use crate::{
  import::data::{Catalogs, Sources},
  messages::Message,
  navigation::import::ImportTab,
  theme::{ExtraSpacing, gutter_scrollbar},
  ui::{
    button::ActionButton,
    field::{ValueTone, field_error, select_style, styled_input, value_box},
    icon::{Icon, IconSize, icon},
    message_text,
    tab::Tab,
  },
};

/// Source select, tab row and search heights, and the tabs' widths (`CkA0k`, `RDOlV`, `QSqPt`).
const SOURCE_HEIGHT: f32 = 40.0;
const TABS_HEIGHT: f32 = 38.0;
const SEARCH_HEIGHT: f32 = 36.0;
const TAB_WIDTHS: [f32; 6] = [48.0, 94.0, 78.0, 116.0, 68.0, 82.0];

impl ImportDialog {
  pub(super) fn select_body(
    &self,
    ctx: &mut Ctx,
    props: &ImportProps,
    sources: Result<&Option<Sources>, &Message>,
    chosen: Option<&RegisteredProject>,
    source: &Source<'_>,
  ) -> Element {
    let tab = self.tab(props);
    // The note follows the list, so it scrolls with it.
    let list = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .child(self.list(ctx, tab, sources, chosen, source))
      .child(Text::new(&ctx.t("projectImport.dependencyNote")).variant(TypographyStyle::Caption));
    body()
      .spacing(ExtraSpacing::Step.role())
      .child(beside_gutter(self.projects(ctx, props, sources)))
      .child(beside_gutter(self.tabs(ctx, tab)))
      .child(beside_gutter(self.search_field(ctx)))
      .child(
        ScrollVertical::new(list)
          .scrollbar(gutter_scrollbar())
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
      .into()
  }

  /// Source project and destination (`UYFtb`).
  fn projects(&self, ctx: &Ctx, props: &ImportProps, sources: Result<&Option<Sources>, &Message>) -> Element {
    let control = match sources {
      Ok(Some(Sources(projects))) if !projects.is_empty() => {
        let options: Vec<(SourceChoice, Arc<str>)> = projects
          .iter()
          .map(|project| (SourceChoice(Some(project.id)), project.name.as_str().into()))
          .collect();
        Element::from(
          Select::new(self.source.clone())
            .options(options)
            .width(Dimension::Pct(100.0))
            .height(Dimension::Px(SOURCE_HEIGHT))
            .style(select_style(ctx, SOURCE_HEIGHT)),
        )
        .id("import-source")
      }
      Ok(Some(_)) => value_box(
        ctx,
        "import-source",
        &ctx.t("projectImport.noSources"),
        ValueTone::Unavailable,
        None,
      ),
      Ok(None) | Err(_) => value_box(ctx, "import-source", "", ValueTone::Placeholder, None),
    };
    let destination = ctx.t_args(
      "projectImport.destinationValue",
      [("project", props.project.project.name().as_str())],
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Field.role())
          .child(Text::new(&ctx.t("projectImport.sourceProject")).variant(TypographyStyle::Caption))
          .child(control),
      )
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Inline.role())
          .child(Text::new(&ctx.t("projectImport.destination")).variant(TypographyStyle::Caption))
          .child(
            Text::new(&destination)
              .variant(TypographyStyle::Body)
              .id("import-destination"),
          ),
      )
      .into()
  }

  /// The kind tabs (`RDOlV`), form controls of the dialog; Secrets is shown but not available.
  fn tabs(&self, ctx: &Ctx, shown: ImportTab) -> Element {
    let label = |tab| match tab {
      ImportTab::All => ctx.t("projectImport.tabs.all"),
      ImportTab::Pipelines => ctx.t("projectImport.tabs.pipelines"),
      ImportTab::Agents => ctx.t("projectImport.tabs.agents"),
      ImportTab::Instructions => ctx.t("projectImport.tabs.instructions"),
      ImportTab::Tools => ctx.t("projectImport.tabs.tools"),
    };
    let ids = [
      "import-tab-all",
      "import-tab-pipelines",
      "import-tab-agents",
      "import-tab-instructions",
      "import-tab-tools",
    ];
    let height = Dimension::Px(TABS_HEIGHT);
    let tabs = ImportTab::ALL
      .into_iter()
      .zip(ids)
      .zip(TAB_WIDTHS)
      .map(|((tab, id), width)| {
        let chosen = self.tab.clone();
        Tab::new(id, label(tab), height)
          .in_form()
          .min_width(Dimension::Px(width))
          .selected(tab == shown)
          .on_click(move || chosen.set(Some(tab)))
      });
    let secrets = Tab::new("import-tab-secrets", ctx.t("projectImport.tabs.secrets"), height)
      .min_width(Dimension::Px(TAB_WIDTHS[5]));
    // The tabs' focus rings reach out of the row, above, below and left of All.
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(height)
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .with_children(tabs.chain([secrets]))
      .into()
  }

  /// Search by name (`QSqPt`).
  fn search_field(&self, ctx: &Ctx) -> Element {
    let input = ctx.theme().form().input.clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SEARCH_HEIGHT))
      .padding_custom(input.padding)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(input.background)
      .border(Border::inside(BorderSize::Sm, input.border))
      .rounded(RadiusSize::Md)
      .child(
        styled_input(ctx, self.search.clone(), input.text.typography)
          .id("import-search")
          .single_line()
          .flex(1.0)
          .placeholder(&ctx.t("projectImport.search")),
      )
      .child(icon(Icon::Search, IconSize::Dismiss, PaletteColor::TextSecondary))
      .into()
  }

  /// The source's resources of the tab that match the search (`u7qCg`), or why there are none.
  fn list(
    &self,
    ctx: &mut Ctx,
    tab: ImportTab,
    sources: Result<&Option<Sources>, &Message>,
    chosen: Option<&RegisteredProject>,
    source: &Source<'_>,
  ) -> Element {
    let note = |text: &str| -> Element {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_vertical(SpacingSize::Md)
        .child(Text::new(text).variant(TypographyStyle::Caption))
        .into()
    };
    let catalogs = match (sources, source) {
      (Err(error), _) | (_, Source::Failed(error)) => return field_error(&message_text(ctx, error)),
      (Ok(Some(Sources(projects))), _) if projects.is_empty() => return note(&ctx.t("projectImport.noOtherProjects")),
      (_, Source::Pending) => {
        let reading = chosen.map_or_else(String::new, |project| {
          ctx
            .t_args("projectImport.reading", [("project", project.name.as_str())])
            .to_string()
        });
        return note(&reading);
      }
      (_, Source::Ready(catalogs)) => *catalogs,
    };
    let search = self.search.get().trim().to_lowercase();
    let shown: Vec<&CatalogResource> = catalogs
      .source
      .resources()
      .iter()
      .filter(|entry| tab.kind().is_none_or(|kind| entry.resource().kind() == kind))
      .filter(|entry| search.is_empty() || entry.resource().name().as_str().to_lowercase().contains(&search))
      .collect();
    if shown.is_empty() {
      let project = catalogs.source.project().name().as_str();
      return if catalogs.source.resources().is_empty() {
        note(&ctx.t_args("projectImport.empty", [("project", project)]))
      } else {
        note(&ctx.t("projectImport.noMatch"))
      };
    }
    let rows: Vec<Element> = shown.iter().map(|entry| self.row(ctx, catalogs, entry)).collect();
    Column::new().width(Dimension::Pct(100.0)).with_children(rows).into()
  }

  fn row(&self, ctx: &mut Ctx, catalogs: &Catalogs, entry: &CatalogResource) -> Element {
    let id = entry.resource().id();
    let chosen = self
      .picked
      .get()
      .0
      .iter()
      .find(|(picked, _)| *picked == id)
      .map(|(_, pick)| *pick);
    let pick = chosen.unwrap_or_else(|| entry.default_pick());
    let alone = Plan::new(
      &catalogs.source,
      &catalogs.target,
      &[(id, entry.default_pick())],
      &Choices::default(),
    );
    let props = ResourceRowProps {
      id,
      name: entry.resource().name().as_str().into(),
      description: labels::describe(ctx, &catalogs.source, entry, labels::dependency_count(&alone)),
      picks: entry
        .picks()
        .into_iter()
        .map(|option| (PickValue(option), labels::pick_option(ctx, option)))
        .collect(),
      pick,
      pick_label: labels::pick(ctx, pick),
      selected: chosen.is_some(),
      picked: self.picked.clone(),
    };
    ctx.mount_keyed::<ResourceRow>(&format!("import-resource-{id}"), props)
  }

  /// How many are selected and what they need resolved, Cancel and Resolve dependencies (`qSGs5`).
  pub(super) fn select_footer(&self, ctx: &mut Ctx, source: &Source<'_>) -> Element {
    let Picked(picked) = self.picked.get();
    let summary = match source {
      Source::Ready(catalogs) if !picked.is_empty() => {
        let plan = self.plan(catalogs);
        let selected = ctx.t_args("projectImport.selected", [("count", picked.len().to_string().as_str())]);
        let dependencies = match labels::dependency_count(&plan) {
          0 => ctx.t("projectImport.toResolve.none"),
          1 => ctx.t("projectImport.toResolve.one"),
          count => ctx.t_args("projectImport.toResolve.other", [("count", count.to_string().as_str())]),
        };
        labels::separated(ctx, &[selected, dependencies])
      }
      _ => ctx.t("projectImport.nothingSelected"),
    };
    let close = Self::close(ctx);
    let cancel = ActionButton::new(ctx.t("common.cancel"))
      .icon(Icon::X)
      .id("import-cancel")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(close)
      .render(ctx);
    let stage = self.stage.clone();
    let next = ActionButton::new(ctx.t("projectImport.resolve"))
      .primary()
      .icon(Icon::ArrowRight)
      .id("import-resolve")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(move || stage.set(Stage::Resolve))
      .enabled(matches!(source, Source::Ready(_)) && !picked.is_empty())
      .render(ctx);
    footer_bar(vec![
      Text::new(&summary)
        .variant(TypographyStyle::Caption)
        .id("import-selection")
        .into(),
      Spacer::new().flex(1.0).into(),
      cancel,
      next,
    ])
  }
}
