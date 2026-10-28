//! The Instructions tab with no instruction yet (06b `VcE5a`): one quiet row in its box with Import from repository and
//! New instruction, then the repository's real findings (T1w9U); findings that couldn't be read are one row in their
//! box (`SsC16`).
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::{
  background,
  instructions::repository::{self, RepositoryScan},
  messages,
};

use super::state::{Selection, Tab, View};
use crate::{
  project::ShownProject,
  screens::hub::{
    self,
    parts::{Entry, boxed},
  },
  theme::{ExtraSpacing, ExtraTypography, HubSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    failure::{Failure, LastFailure},
    icon::Icon,
  },
};
const FOUND_PATH_WIDTH: f32 = 200.0;
#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub selection: Selection,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}
impl DevtoolsInspectable for Props {}
pub struct Empty {
  /// Counts the person's tries to read the repository's findings again.
  again: Signal<u64>,
  /// Why the findings couldn't be read, kept while they are read again.
  failure: LastFailure,
}
impl Component for Empty {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      again: ctx.signal(0),
      failure: LastFailure::default(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let read = ctx
      .future((root, self.again.get()), |(root, _)| async move {
        background::run(move || repository::scan(&root))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "instruction repository findings could not be read");
            messages::repository_unreadable()
          })
      })
      .state()
      .get();
    let failed = self.failure.follow(&read);
    let mut pane = Column::new()
      .id("instructions-empty-page")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Groups.role())
      .child(boxed("instructions-empty-rows", [empty_row(ctx, &props)]));
    if let Some((error, retrying)) = failed {
      // The findings' box holds one row in place of its rows; the rest of the page works.
      let again = self.again.clone();
      let row = Failure::new(
        "instructions-repository-error",
        ctx.t("instructions.importReadFailed"),
        error,
      )
      .retry(retrying, move || again.update(|n| *n += 1))
      .section(ctx);
      pane = pane.child(boxed("instructions-repository-unread", [row]));
    } else if let Some(scan) = read.data.as_ref().filter(|scan| !scan.items.is_empty()) {
      pane = pane.child(findings(ctx, scan, &props.project.open.repo_root.to_string_lossy()));
    }
    hub::list_body("instructions-empty-list", pane)
  }
}
/// The quiet row: what instructions are, with Import from repository and New instruction (06b).
fn empty_row(ctx: &Ctx, props: &Props) -> Element {
  let (new, import) = (props.selection.clone(), props.selection.clone());
  let project = props.project.open.project.name().as_str();
  Entry::new(
    "instructions-empty",
    ctx.t_args("instructions.emptyTitle", [("project", project)]),
  )
  .tile(Icon::BookOpen)
  .hint(ctx.t("instructions.emptyDetail"))
  .end(
    ActionButton::new(
      "instructions-empty-import",
      ButtonKind::Ghost,
      ctx.t("instructions.import"),
    )
    .icon(Icon::FolderOpen)
    .opens(import.to(View::Import)),
  )
  .end(
    ActionButton::new(
      "instructions-empty-new",
      ButtonKind::Secondary,
      ctx.t("instructions.new"),
    )
    .icon(Icon::Plus)
    .opens(new.to(View::new_rule(Tab::Project))),
  )
  .into()
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
      &crate::screens::hub::import_detail::item_detail(ctx, item),
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
  found.child(findings_foot(ctx)).into()
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
