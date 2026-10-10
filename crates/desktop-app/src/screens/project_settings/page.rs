//! The 720-wide header and scrolled form (rsQ7o, ZZRcn, tqYNS).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{Status, Tab};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, TaskList},
  theme::{ExtraSpacing, ExtraTypography, TaskFormSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    form_section::{Label, Place, side_row},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    tabs::{PageTab, page_tabs},
  },
};
pub const WIDTH: f32 = 720.0;
pub const CONTROL_WIDTH: f32 = 300.0;

pub fn header(
  ctx: &mut Ctx,
  (project, tab): (&ShownProject, Tab),
  selected: &Signal<Tab>,
  status: &Signal<Status>,
) -> Element {
  let save_status = save_status(ctx, status);
  let title = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("project_settings.title")).variant(TypographyStyle::Heading))
    .child(Spacer::new().flex(1.0))
    .child(save_status);
  let tabs = Tab::ALL
    .into_iter()
    .map(|choice| {
      let select = selected.clone();
      let status = status.clone();
      PageTab::new(
        format!("settings-tab-{}", choice.name()),
        ctx.t(choice.key()),
        tab == choice,
        move || {
          select.set(choice);
          status.set(Status::Saved);
        },
      )
    })
    .chain([
      PageTab::unavailable(
        "settings-tab-memory",
        ctx.t("project_settings.memory"),
        ctx.t("project_settings.outOfScope"),
      ),
      PageTab::unavailable(
        "settings-tab-sharing",
        ctx.t("project_settings.sharing"),
        ctx.t("project_settings.outOfScope"),
      ),
    ])
    .collect::<Vec<_>>();
  let tabs = page_tabs(ctx, "project-settings-tabs", tabs);
  let name = project.open.project.name().as_str();
  let content = Column::new()
    .width(Dimension::Px(WIDTH))
    .spacing(SpacingSize::Xs)
    .child(title)
    .child(Text::new(&ctx.t_args(tab.description(), [("project", name)])).variant(ExtraTypography::Prose.role()));
  Column::new()
    .id("project-settings-head")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .padding(
      Padding::new()
        .top(TaskFormSpacing::PageTop.role())
        .left(SpacingSize::Xl)
        .right(SpacingSize::Xl),
    )
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(content)
    .child(Column::new().width(Dimension::Px(WIDTH)).child(tabs))
    .into()
}
pub fn screen(header: Element, content: Element) -> Element {
  Column::new()
    .id("screen-project_settings")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(content)
    .into()
}
pub fn scroll(content: impl Into<Element>) -> Element {
  ScrollVertical::new(
    Column::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .padding(
        Padding::new()
          .top(SpacingSize::Xl)
          .bottom(TaskFormSpacing::PageBottom.role())
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl),
      )
      .child(
        Column::new()
          .width(Dimension::Px(WIDTH))
          .spacing(ExtraSpacing::Roomy.role())
          .child(content),
      ),
  )
  .id("project-settings-scroll")
  .flex(1.0)
  .into()
}
pub fn row(ctx: &Ctx, id: &str, keys: (&str, &str), control: Element, place: Place) -> Element {
  side_row(
    Label::new(id, ctx.t(keys.0)).description(ctx.t(keys.1)),
    control,
    place,
    true,
  )
}
pub fn note(id: &str, words: &str) -> Element {
  Text::new(words)
    .id(id.to_owned())
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0))
    .into()
}
pub fn review_notice(ctx: &mut Ctx, project: &ShownProject, environment: bool) -> Element {
  let nav = Navigation::of(ctx);
  let id = project.open.project.id();
  let keys = if environment {
    (
      "project_settings.environmentChanges",
      "project_settings.environmentChangesDetail",
    )
  } else {
    ("project_settings.changesTitle", "project_settings.changesDetail")
  };
  Notice::new("settings-changes-notice", NoticeTone::Info, ctx.t(keys.0))
    .body(ctx.t(keys.1))
    .action(
      ActionButton::new(
        "settings-review-runs",
        ButtonKind::Secondary,
        ctx.t("project_settings.reviewRuns"),
      )
      .on_click(move || nav.go(Route::Project(id, ProjectPage::Tasks(TaskList::default())))),
    )
    .into()
}
fn save_status(ctx: &Ctx, status: &Signal<Status>) -> Row {
  let state = status.get();
  let key = match state {
    Status::Saved => "project_settings.saved",
    Status::Unsaved => "project_settings.unsaved",
    Status::Invalid => "project_settings.invalid",
    Status::Saving => "project_settings.saving",
    Status::Failed => "project_settings.failed",
  };
  let (glyph, tone) = match state {
    Status::Saved => (Icon::Check, PaletteColor::TextMuted),
    Status::Saving => (Icon::LoaderCircle, PaletteColor::TextSecondary),
    Status::Unsaved => (Icon::CircleAlert, PaletteColor::TextSecondary),
    Status::Invalid | Status::Failed => (Icon::CircleAlert, PaletteColor::Danger),
  };
  Row::new()
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(icon(glyph, crate::theme::IconSize::Sm, tone.clone()))
    .child(
      Text::new(&ctx.t(key))
        .id("project-settings-save-status")
        .variant(TypographyStyle::Caption)
        .color(tone),
    )
}
