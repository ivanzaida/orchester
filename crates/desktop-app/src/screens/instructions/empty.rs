//! Empty project B5A4Ux, including real repository findings T1w9U.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::{
  background,
  instructions::repository::{self, RepositoryScan},
  messages::Message,
};

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
// Empty state sG2xi and its top inset within the Instructions body.
const EMPTY_WIDTH: f32 = 600.0;
const FOUND_PATH_WIDTH: f32 = 200.0;
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub selection: Selection,
  pub all_empty: bool,
  /// The scope tab shown.
  pub tab: Tab,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.all_empty == other.all_empty && self.tab == other.tab
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
    let tab = props.tab;
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
      .on_click(move || new.open(View::new_rule(tab))),
    );
    if let Some(scan) = read.data.as_ref().filter(|scan| !scan.items.is_empty()) {
      empty = empty.child(findings(ctx, scan, &props.project.open.repo_root.to_string_lossy()));
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
fn finding(glyph: Icon, source: &str, detail: &str) -> Element {
  Row::new()
    .id(format!("instructions-finding-{source}"))
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Loose.role())
    .padding(lurq::node::padding::Padding::vertical(SpacingSize::Sm))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crate::ui::icon::icon(
      glyph,
      crate::theme::IconSize::Base,
      PaletteColor::TextMuted,
    ))
    .child(
      Text::new(source)
        .variant(crate::theme::instructions::discovery_path())
        .width(Dimension::Px(FOUND_PATH_WIDTH)),
    )
    .child(
      Text::new(detail)
        .variant(crate::theme::instructions::discovery_detail())
        .flex(1.0),
    )
    .into()
}
fn findings(ctx: &mut Ctx, scan: &RepositoryScan, root: &str) -> Element {
  let mut found = Column::new()
    .id("instructions-repository-findings")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .rounded(lurq::app::theme::RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .padding(
      lurq::node::padding::Padding::new()
        .top(SpacingSize::Md)
        .left(SpacingSize::Lg)
        .right(SpacingSize::Lg)
        .bottom(SpacingSize::Sm),
    )
    .spacing(ExtraSpacing::Snug.role())
    .child(findings_head(ctx, scan.items.len(), root));
  for item in scan
    .items
    .iter()
    .filter(|item| item.kind == orchester_core::agent_files::import::ItemKind::Instruction)
  {
    found = found.child(finding(
      Icon::FileText,
      &item.source,
      &super::import::item_detail(ctx, item),
    ));
  }
  for folder in orchester_core::agent_files::import::SKILL_FOLDERS {
    let count = scan
      .items
      .iter()
      .filter(|item| item.source.starts_with(&format!("{folder}/")))
      .count();
    if count > 0 {
      found = found.child(finding(
        Icon::Folder,
        folder,
        &ctx.t_args("instructions.foundSkills", [("count", count.to_string())]),
      ));
    }
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(crate::theme::instructions::discovery_offset())
    .child(found.child(findings_foot(ctx)))
    .into()
}
fn findings_head(ctx: &mut Ctx, count: usize, root: &str) -> Element {
  let path = crate::ui::tooltip::Truncated::new(
    "instructions-repository-path",
    root,
    crate::theme::instructions::variable_value(),
  )
  .mono()
  .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .padding_bottom(ExtraSpacing::Snug.role())
    .child(crate::ui::icon::icon(
      Icon::FolderGit2,
      crate::theme::IconSize::Md,
      PaletteColor::TextSecondary,
    ))
    .child(
      Text::new(&ctx.t_args("instructions.found", [("count", count.to_string())]))
        .variant(ExtraTypography::Strong.role()),
    )
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .justify(lurq::layout::layout_kind::Justify::End)
        .child(path),
    )
    .into()
}
fn findings_foot(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Snug.role())
    .padding_top(SpacingSize::Sm)
    .padding_bottom(SpacingSize::Xs)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(crate::ui::icon::icon(
      Icon::ShieldCheck,
      crate::theme::IconSize::Sm,
      PaletteColor::TextMuted,
    ))
    .child(Text::new(&ctx.t("instructions.importDraftsOnly")).variant(crate::theme::instructions::discovery_note()))
    .into()
}
