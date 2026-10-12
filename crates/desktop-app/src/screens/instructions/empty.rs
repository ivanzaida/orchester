//! Empty project B5A4Ux, including real repository findings T1w9U.
use super::state::{Selection, Tab, View};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    state::EmptyState,
  },
};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::{
  background,
  messages::Message,
  skills::repository::{self, RepositoryScan},
};
// Empty state sG2xi and its top inset within the Instructions body.
const EMPTY_WIDTH: f32 = 600.0;
const FOUND_PATH_WIDTH: f32 = 200.0;
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub selection: Selection,
  pub all_empty: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.all_empty == other.all_empty
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, Debug, PartialEq, Eq)]
enum ReadError {
  Files,
  Background(Message),
}
impl DevtoolsInspectable for ReadError {}
pub struct Empty;
impl Component for Empty {
  type Props = Props;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let read = ctx
      .future(root, |root| async move {
        background::run(move || repository::scan(&root))
          .await
          .map_err(ReadError::Background)?
          .map_err(|error| {
            tracing::error!(%error,"instruction repository findings could not be read");
            ReadError::Files
          })
      })
      .state()
      .get();
    let (new, import) = (props.selection.clone(), props.selection.clone());
    let tab = match props.selection.view.get() {
      View::Rule(tab, _) | View::New(tab) => tab,
      View::Import => Tab::Project,
    };
    let title = if props.all_empty {
      ctx.t_args(
        "instructions.emptyTitle",
        [("project", props.project.open.project.name().as_str())],
      )
    } else {
      ctx.t_args(
        "instructions.emptyScope",
        [
          ("project", props.project.open.project.name().as_str()),
          ("scope", &ctx.t(tab.key())),
        ],
      )
    };
    let mut empty = EmptyState::new(
      "instructions-empty",
      Icon::BookOpen,
      title,
      ctx.t("instructions.emptyDetail"),
    )
    .wide()
    .unframed()
    .note(ctx.t("instructions.emptyTabDetail"))
    .action(
      ActionButton::new(
        "instructions-empty-import",
        ButtonKind::Primary,
        ctx.t("instructions.import"),
      )
      .icon(Icon::FolderOpen)
      .on_click(move || import.open(View::Import)),
    )
    .action(
      ActionButton::new(
        "instructions-empty-new",
        ButtonKind::Secondary,
        ctx.t("instructions.new"),
      )
      .icon(Icon::Plus)
      .on_click(move || new.open(View::New(tab))),
    );
    if let Some(scan) = read.data.as_ref().filter(|scan| !scan.items.is_empty()) {
      empty = empty.child(findings(ctx, scan));
    } else if read.is_rejected() {
      empty = empty.child(field_error(
        "instructions-repository-error",
        &ctx.t("instructions.importReadFailed"),
      ));
    }
    ScrollVertical::new(
      Column::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .padding_top(crate::theme::instructions::empty_top())
        .child(Column::new().width(Dimension::Px(EMPTY_WIDTH)).child(empty)),
    )
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
  }
}
fn finding(_: &Ctx, source: &str, detail: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .padding_top(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(source)
        .variant(TypographyStyle::Mono)
        .width(Dimension::Px(FOUND_PATH_WIDTH)),
    )
    .child(Text::new(detail).variant(TypographyStyle::Description).flex(1.0))
    .into()
}
fn findings(ctx: &Ctx, scan: &RepositoryScan) -> Element {
  let mut found = Column::new()
    .id("instructions-repository-findings")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .rounded(lurq::app::theme::RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&ctx.t_args("instructions.found", [("count", scan.items.len().to_string())]))
        .variant(ExtraTypography::Strong.role()),
    );
  for item in scan
    .items
    .iter()
    .filter(|item| item.kind == orchester_core::agent_files::import::ItemKind::Instruction)
  {
    found = found.child(finding(ctx, &item.source, &super::import::item_detail(ctx, item)));
  }
  for folder in orchester_core::agent_files::import::SKILL_FOLDERS {
    let count = scan
      .items
      .iter()
      .filter(|item| item.source.starts_with(&format!("{folder}/")))
      .count();
    if count > 0 {
      found = found.child(finding(
        ctx,
        folder,
        &ctx.t_args("instructions.foundSkills", [("count", count.to_string())]),
      ));
    }
  }
  found
    .child(Text::new(&ctx.t("instructions.importDraftsOnly")).variant(TypographyStyle::Caption))
    .into()
}
