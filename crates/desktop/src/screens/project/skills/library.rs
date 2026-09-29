//! Skills library (`qO1Az`, built on 03.04) and its empty state (`YRaLK`, built on 03.A): the
//! project's skills with their files, version, scope and last publish, filtered by scope, and how
//! skills reach agents. Without skills, what a skill is and, when the repository gives its agents
//! files, what Import from repository would make of them.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Select, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::InstructionScope;

use super::{
  super::{ProjectNav, page, repository_import::copy as found_copy},
  row::{column_headings, skill_row},
};
use crate::{
  background,
  messages::Message,
  skills::{
    data::{ProjectSkills, SkillEntry},
    repository::{RepositoryScan, scan},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::select_style,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::ownership_note,
    panel::RecoveryPanel,
    summary_step::summary_step,
  },
  workspace::OpenProject,
};

/// Filter, side panel, empty state icon and card sizes (`AIUvA`, `BFLW0`, `yDzft`, `mzYus`).
const FILTER_WIDTH: f32 = 150.0;
const FILTER_HEIGHT: f32 = 34.0;
const SIDE_PANEL_WIDTH: f32 = 268.0;
const EMPTY_ICON_SIZE: f32 = 48.0;
const EMPTY_WIDTH: f32 = 640.0;

/// Which skills the library lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScopeFilter {
  #[default]
  All,
  Project,
  Roles,
  PipelinesAndSteps,
}

impl lurq::app::component::DevtoolsInspectable for ScopeFilter {}

impl ScopeFilter {
  pub const ALL: [Self; 4] = [Self::All, Self::Project, Self::Roles, Self::PipelinesAndSteps];

  pub fn admits(self, scope: &InstructionScope) -> bool {
    matches!(
      (self, scope),
      (Self::All, _)
        | (Self::Project, InstructionScope::Project)
        | (Self::Roles, InstructionScope::AgentRole { .. })
        | (
          Self::PipelinesAndSteps,
          InstructionScope::Pipeline { .. } | InstructionScope::Step { .. }
        )
    )
  }

  pub const fn key(self) -> &'static str {
    match self {
      Self::All => "skills.filter.all",
      Self::Project => "skills.filter.project",
      Self::Roles => "skills.filter.roles",
      Self::PipelinesAndSteps => "skills.filter.pipelines",
    }
  }
}

#[derive(Clone)]
pub struct LibraryProps {
  pub project: OpenProject,
  /// `None` while the skills are read, or when they could not be (`error`).
  pub data: Option<ProjectSkills>,
  pub error: Option<Message>,
  /// Whether the New skill dialog is open.
  pub creating: Signal<bool>,
}

impl PartialEq for LibraryProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.data == other.data && self.error == other.error
  }
}

impl lurq::app::component::DevtoolsInspectable for LibraryProps {}

pub struct Library {
  filter: Signal<ScopeFilter>,
  nav: ProjectNav,
}

impl Component for Library {
  type Props = LibraryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      filter: ctx.signal(ScopeFilter::All),
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LibraryProps>().clone();
    let empty = props.data.as_ref().is_some_and(|data| data.entries.is_empty());
    let found = self.found(ctx, &props.project, empty);
    let project = props.project.project.name().as_str();
    let content = match (&props.data, &props.error) {
      (_, Some(error)) => RecoveryPanel::new(
        Icon::AlertCircle,
        &ctx.t("skills.library.loadFailed"),
        &message_text(ctx, error),
      )
      .into(),
      (Some(_), None) if empty => self.empty(ctx, project, found.as_ref(), &props.creating),
      (Some(data), None) => self.list(ctx, data, project),
      (None, None) => Column::new().into(),
    };
    let body = Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .padding_horizontal(SpacingSize::Section)
      .padding_bottom(SpacingSize::Section)
      .spacing(SpacingSize::Section)
      .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(content))
      .child(versions_panel(ctx, empty));
    Column::new()
      .id("skills-library")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(
        &ctx.t_args("skills.breadcrumb", [("project", project)]),
        self.topbar_actions(ctx, &props.creating),
      ))
      .child(heading(ctx))
      .child(body)
  }
}

impl Library {
  /// What the repository gives its agents, read only while the project has no skills.
  fn found(&self, ctx: &mut Ctx, open: &OpenProject, empty: bool) -> Option<RepositoryScan> {
    let load = ctx.future((open.repo_root.clone(), empty), |(repo_root, empty)| async move {
      if !empty {
        return Ok(None);
      }
      background::run(move || scan(&repo_root))
        .await?
        .map(Some)
        .map_err(|error| {
          tracing::warn!(%error, "cannot read the repository's agent files");
          Message::new("skills.import.readFailedTitle")
        })
    });
    load.state().get().data.flatten()
  }

  /// Import from repository and New skill (`ap50g`).
  fn topbar_actions(&self, ctx: &Ctx, creating: &Signal<bool>) -> Vec<Element> {
    let (nav, creating) = (self.nav.clone(), creating.clone());
    vec![
      ActionButton::new(ctx.t("skills.import.action"))
        .icon(Icon::FolderInput)
        .id("skills-import")
        .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
        .on_click(move || nav.show_repository_import())
        .render(ctx),
      ActionButton::new(ctx.t("skills.new"))
        .primary()
        .icon(Icon::Plus)
        .id("skills-new")
        .height(Dimension::Px(page::TOPBAR_BUTTON_HEIGHT))
        .on_click(move || creating.set(true))
        .render(ctx),
    ]
  }

  /// Filter, counts, column headings, rows and the ownership note (`bjvCA`).
  fn list(&self, ctx: &Ctx, data: &ProjectSkills, project: &str) -> Element {
    let filter = self.filter.get();
    let shown: Vec<&SkillEntry> = data
      .entries
      .iter()
      .filter(|entry| filter.admits(entry.body.scope()))
      .collect();
    let with_drafts = data.entries.iter().filter(|entry| entry.state.has_draft()).count();
    let counts = Message::new(if data.entries.len() == 1 {
      "skills.library.counts.one"
    } else {
      "skills.library.counts.other"
    })
    .with("count", data.entries.len().to_string())
    .with("drafts", with_drafts.to_string());
    let options: Vec<(ScopeFilter, Arc<str>)> = ScopeFilter::ALL
      .into_iter()
      .map(|filter| (filter, ctx.t(filter.key())))
      .collect();
    let select = Select::new(self.filter.clone())
      .options(options)
      .width(Dimension::Px(FILTER_WIDTH))
      .height(Dimension::Px(FILTER_HEIGHT))
      .style(select_style(ctx, FILTER_HEIGHT));
    let filters = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FILTER_HEIGHT))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(Element::from(select).id("skills-filter"))
      .child(Spacer::new().flex(1.0))
      .child(Text::new(&message_text(ctx, &counts)).variant(TypographyStyle::Caption));
    let rows = Column::new()
      .width(Dimension::Pct(100.0))
      .child(column_headings(ctx))
      .with_children(shown.into_iter().map(|entry| skill_row(ctx, entry, data, &self.nav)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(SpacingSize::Md)
      .child(filters)
      .child(ScrollVertical::new(rows).width(Dimension::Pct(100.0)).flex(1.0))
      .child(ownership_note(
        &ctx.t_args("skills.library.ownership", [("project", project)]),
      ))
      .into()
  }

  /// No skills yet (`a7s7S`): what a skill is, what the repository has, and the two ways to begin.
  fn empty(&self, ctx: &Ctx, project: &str, found: Option<&RepositoryScan>, creating: &Signal<bool>) -> Element {
    let icon_surface = Row::new()
      .size(Dimension::Px(EMPTY_ICON_SIZE), Dimension::Px(EMPTY_ICON_SIZE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .rounded(RadiusSize::Lg)
      .child(icon(Icon::Sparkles, IconSize::Large, PaletteColor::TextPrimary));
    let explanation = Column::new()
      .width(Dimension::Px(EMPTY_WIDTH))
      .spacing(ExtraSpacing::Inline.role())
      .child(
        Text::new(&ctx.t_args("skills.empty.title", [("project", project)]))
          .variant(ExtraTypography::DialogTitle.role()),
      )
      .child(Text::new(&ctx.t("skills.empty.description")).variant(ExtraTypography::Summary.role()));
    let found = found.filter(|scan| !scan.items.is_empty());
    let (nav, creating) = (self.nav.clone(), creating.clone());
    let import = ActionButton::new(ctx.t("skills.import.action"))
      .icon(Icon::FolderInput)
      .id("skills-empty-import")
      .on_click(move || nav.show_repository_import());
    let new = ActionButton::new(ctx.t("skills.new"))
      .icon(Icon::Plus)
      .id("skills-empty-new")
      .on_click(move || creating.set(true));
    // The repository's own files are the quicker start when there are any.
    let (import, new) = if found.is_some() {
      (import.primary(), new)
    } else {
      (import, new.primary())
    };
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_top(SpacingSize::Sm)
      .spacing(SpacingSize::Lg)
      .child(icon_surface)
      .child(explanation);
    if let Some(found) = found {
      column = column.child(found_card(ctx, found));
    }
    let actions = Row::new()
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center);
    let actions = if found.is_some() {
      actions.child(import.render(ctx)).child(new.render(ctx))
    } else {
      actions.child(new.render(ctx)).child(import.render(ctx))
    };
    column.child(actions).into()
  }
}

/// Page heading (`m0NWP`).
fn heading(ctx: &Ctx) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("skills.title")).variant(ExtraTypography::ScreenTitle.role()))
    .child(
      Text::new(&ctx.t("skills.description"))
        .variant(TypographyStyle::Body)
        .color(PaletteColor::TextSecondary),
    )
    .into()
}

/// Found in this repository (`mzYus`): each file or folder and what it becomes.
fn found_card(ctx: &Ctx, found: &RepositoryScan) -> Element {
  let count = found.items.len();
  let count = if count == 1 {
    ctx.t("skills.empty.foundCount.one")
  } else {
    ctx.t_args("skills.empty.foundCount.other", [("count", count.to_string().as_str())])
  };
  let header = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::FolderGit, IconSize::Option, PaletteColor::TextPrimary))
    .child(
      Text::new(&ctx.t("skills.empty.found"))
        .variant(TypographyStyle::Label)
        .flex(1.0),
    )
    .child(Text::new(&count).variant(TypographyStyle::Caption));
  let items = Column::new()
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Tight.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(
      found
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| summary_step(index + 1, &item.source, &found_copy::card_line(ctx, item))),
    );
  Column::new()
    .id("skills-found")
    .width(Dimension::Px(EMPTY_WIDTH))
    .padding(ExtraSpacing::Step.role())
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(header)
    .child(items)
    .child(Text::new(&ctx.t("skills.empty.foundResult")).variant(ExtraTypography::Detail.role()))
    .into()
}

/// How skills reach agents (`BFLW0`); the hint at its foot differs without skills (`fzZHu`).
fn versions_panel(ctx: &Ctx, empty: bool) -> Element {
  let item = |glyph: Icon, title: &str, detail: &str| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(icon(glyph, IconSize::Option, PaletteColor::TextSecondary))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Tight.role())
          .child(
            Text::new(&ctx.t(title))
              .variant(TypographyStyle::FieldLabel)
              .color(PaletteColor::TextPrimary),
          )
          .child(Text::new(&ctx.t(detail)).variant(ExtraTypography::Detail.role())),
      )
  };
  let hint = if empty {
    "skills.versions.hintEmpty"
  } else {
    "skills.versions.hint"
  };
  Column::new()
    .width(Dimension::Px(SIDE_PANEL_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding_left(SpacingSize::Lg)
    .spacing(ExtraSpacing::Step.role())
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(&ctx.t("skills.versions.title")).variant(ExtraTypography::SectionHeading.role()))
    .child(item(
      Icon::Crosshair,
      "skills.versions.scopeTitle",
      "skills.versions.scopeDetail",
    ))
    .child(item(
      Icon::Upload,
      "skills.versions.publishTitle",
      "skills.versions.publishDetail",
    ))
    .child(item(
      Icon::Pin,
      "skills.versions.keepTitle",
      "skills.versions.keepDetail",
    ))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t(hint)).variant(ExtraTypography::Detail.role()))
    .into()
}
