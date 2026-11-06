//! The tab's groups (`Pane` `iq9oK`, 28 apart): Services, Stacks and Test suites, each its heading with its count right
//! after it (`Head` `tfePU`) over its rows, then Flaky tests once there is a suite (`Flaky tests` `rduQx`). A service's
//! row is its name in Geist Mono with its driver's chip, what it needs and owns, and at its end whether its start asks
//! (`postgres` `aQS35`); a stack's its start order and the slots it takes (`cloud` `UzOND`); a suite's its layer's
//! chip, its report, needs and timeout (`lint` `Vw3cP`). Each row opens its page. With no service yet the Services
//! group is the empty card (`Empty` `Hck5v`), the others an empty row (`MUfXK`, `BkjpN`), and a note says what agents
//! are told meanwhile (`Agents` `oBjF8`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::TestSuite,
  service::{Service, Stack},
};
use orchester_workspace::agent_testing::{data::Entry, listing::ProjectServices};

use super::{
  super::{
    TabProps,
    kit::{Group, SettingRow},
  },
  words,
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{ProjectPage, Route, ServicesPage},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, TagChip, badge},
    icon::{Icon, icon},
    state::EmptyState,
  },
};

/// What opens the pages under the tab.
struct Pages {
  navigation: Navigation,
  project: orchester_core::id::ProjectId,
}

impl Pages {
  fn to(&self, page: ServicesPage) -> Open {
    self
      .navigation
      .to(Route::Project(self.project, ProjectPage::Services(page)))
  }

  /// A button of the tab, `kind`, that opens `page`.
  fn button(
    &self,
    ctx: &Ctx,
    (id, kind): (&str, ButtonKind),
    (glyph, key): (Icon, &str),
    page: ServicesPage,
  ) -> Element {
    ActionButton::new(id.to_owned(), kind, ctx.t(key))
      .icon(glyph)
      .opens(self.to(page))
      .into()
  }
}

/// The tab's groups for what the project defines.
pub(super) fn groups(ctx: &mut Ctx, props: &TabProps, listed: &ProjectServices) -> Element {
  let pages = Pages {
    navigation: Navigation::of(ctx),
    project: props.project.open.project.id(),
  };
  let bodies: Vec<Service> = listed.services.iter().map(|entry| entry.body.clone()).collect();
  let flaky = (!listed.suites.is_empty()).then(|| flaky(ctx, &pages));
  let note = listed.services.is_empty().then(|| agents_note(ctx));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(services(ctx, &pages, &listed.services))
    .child(stacks(ctx, &pages, &listed.stacks, &bodies))
    .child(suites(ctx, &pages, listed))
    .with_children(flaky)
    .with_children(note)
    .into()
}

/// A group's count right after its heading (`Count` `n3kni`).
fn count(id: &str, count: usize) -> Text {
  Text::new(&count.to_string())
    .id(format!("{id}-count"))
    .variant(ProjectSettingsText::Hint.role())
}

fn chevron() -> Text {
  icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted)
}

fn services(ctx: &mut Ctx, pages: &Pages, services: &[Entry<Service>]) -> Element {
  const ID: &str = "settings-services";
  let group = Group::new(ID, ctx.t("project_settings.services.services")).beside(count(ID, services.len()));
  // In the empty card Import draft is the one action the card is about; over the rows it steps back.
  let kind = if services.is_empty() {
    ButtonKind::Primary
  } else {
    ButtonKind::Ghost
  };
  let import = pages.button(
    ctx,
    ("settings-services-import", kind),
    (Icon::FileInput, "project_settings.services.importDraft"),
    ServicesPage::Import,
  );
  let new = pages.button(
    ctx,
    ("settings-services-new", ButtonKind::Secondary),
    (Icon::Plus, "project_settings.services.newService"),
    ServicesPage::NewService,
  );
  if services.is_empty() {
    let empty = EmptyState::new(
      "settings-services-empty",
      Icon::Server,
      ctx.t("project_settings.services.empty.title"),
      ctx.t("project_settings.services.empty.explanation"),
    )
    .wide()
    .action(import)
    .action(new);
    return group.after(empty).into();
  }
  let rows: Vec<Element> = services.iter().map(|entry| service_row(ctx, pages, entry)).collect();
  group.end(import).end(new).rows(rows).into()
}

fn service_row(ctx: &mut Ctx, pages: &Pages, entry: &Entry<Service>) -> Element {
  let id = format!("settings-service-{}", entry.name);
  // A start asks a person until services have approval rules of their own; a service never published cannot start.
  let (key, tone) = match entry.state.published() {
    Some(_) => ("project_settings.services.approval.asks", BadgeTone::Neutral),
    None => ("project_settings.services.approval.notPublished", BadgeTone::Warning),
  };
  SettingRow::new(id.clone(), entry.name.clone())
    .mono_label()
    .beside(TagChip::new(
      format!("{id}-driver"),
      words::driver(ctx, entry.body.ui_driver()),
    ))
    .hint(words::service_hint(ctx, &entry.body))
    .control(badge(&format!("{id}-approval"), tone, &ctx.t(key)))
    .control(chevron())
    .opens(pages.to(ServicesPage::Service(entry.id)))
    .render(ctx)
}

fn stacks(ctx: &mut Ctx, pages: &Pages, stacks: &[Entry<Stack>], services: &[Service]) -> Element {
  const ID: &str = "settings-stacks";
  let mut group = Group::new(ID, ctx.t("project_settings.services.stacks")).beside(count(ID, stacks.len()));
  // A stack groups services, so it can be made once there is one.
  if !services.is_empty() {
    group = group.end(pages.button(
      ctx,
      ("settings-stacks-new", ButtonKind::Secondary),
      (Icon::Plus, "project_settings.services.newStack"),
      ServicesPage::NewStack,
    ));
  }
  if stacks.is_empty() {
    let empty = SettingRow::new(
      "settings-stacks-empty",
      ctx.t("project_settings.services.stacksEmpty.title"),
    )
    .hint(ctx.t("project_settings.services.stacksEmpty.detail"))
    .render(ctx);
    return group.row(empty).into();
  }
  let rows: Vec<Element> = stacks
    .iter()
    .map(|entry| {
      let id = format!("settings-stack-{}", entry.name);
      SettingRow::new(id.clone(), entry.name.clone())
        .mono_label()
        .hint(words::stack_hint(ctx, &entry.body, services))
        .control(
          Text::new(&words::slots(ctx, &entry.body, services))
            .id(format!("{id}-slots"))
            .variant(ProjectSettingsText::Value.role())
            .nowrap(),
        )
        .control(chevron())
        .opens(pages.to(ServicesPage::Stack(entry.id)))
        .render(ctx)
    })
    .collect();
  group.rows(rows).into()
}

fn suites(ctx: &mut Ctx, pages: &Pages, listed: &ProjectServices) -> Element {
  const ID: &str = "settings-suites";
  let suites = &listed.suites;
  let group = Group::new(ID, ctx.t("project_settings.services.suites")).beside(count(ID, suites.len()));
  let new = pages.button(
    ctx,
    ("settings-suites-new", ButtonKind::Secondary),
    (Icon::Plus, "project_settings.services.newSuite"),
    ServicesPage::NewSuite,
  );
  if suites.is_empty() {
    let empty = SettingRow::new(
      "settings-suites-empty",
      ctx.t("project_settings.services.suitesEmpty.title"),
    )
    .hint(ctx.t("project_settings.services.suitesEmpty.detail"))
    .control(new)
    .render(ctx);
    return group.row(empty).into();
  }
  let note = Text::new(&ctx.t("project_settings.services.suitesNote"))
    .id("settings-suites-note")
    .variant(ExtraTypography::Note.role())
    .nowrap();
  let rows: Vec<Element> = suites
    .iter()
    .map(|entry| suite_row(ctx, pages, entry, listed))
    .collect();
  group.end(note).end(new).rows(rows).into()
}

fn suite_row(ctx: &mut Ctx, pages: &Pages, entry: &Entry<TestSuite>, listed: &ProjectServices) -> Element {
  let id = format!("settings-suite-{}", entry.name);
  SettingRow::new(id.clone(), entry.name.clone())
    .mono_label()
    .beside(TagChip::new(
      format!("{id}-layer"),
      words::layer(ctx, entry.body.layer()),
    ))
    .hint(words::suite_hint(ctx, &entry.body, &listed.stacks))
    .control(chevron())
    .opens(pages.to(ServicesPage::Suite(entry.id)))
    .render(ctx)
}

/// Flaky tests (`Quarantine` `TUVz1`): the entry to the quarantined tests' page.
fn flaky(ctx: &mut Ctx, pages: &Pages) -> Element {
  let row = SettingRow::new(
    "settings-flaky-quarantine",
    ctx.t("project_settings.services.quarantine"),
  )
  .info(ctx.t("project_settings.services.quarantineInfo"))
  .hint(ctx.t("project_settings.services.quarantineHint"))
  .control(chevron())
  .opens(pages.to(ServicesPage::Flaky))
  .render(ctx);
  Group::new("settings-flaky", ctx.t("project_settings.services.flaky"))
    .row(row)
    .into()
}

/// What agents are told while the project has no service (`Agents` `oBjF8`): a 13 `bot` and the words in 12
/// `text_muted`, 8 apart.
fn agents_note(ctx: &Ctx) -> Element {
  Row::new()
    .id("settings-services-agents")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Bot, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("project_settings.services.agentsNote"))
        .id("settings-services-agents-text")
        .variant(ProjectSettingsText::Hint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
