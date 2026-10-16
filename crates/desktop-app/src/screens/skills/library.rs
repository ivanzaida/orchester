//! Library Syllb and empty states WMT1W/F8mJNd. The filter tabs are the route's ([`SkillsFilter`]).
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, VirtualizedList},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InstructionScope;

use super::{PageProps, guidance, repository, state::Panel, words};
use crate::{
  routes::SkillsFilter,
  theme::{ConnectionsSpacing, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, marker},
    inspector::note,
    skill_row,
    tabs::{PageTab, counted, page_tabs},
  },
};
/// Tv2Ei columns and its two-line row.
const ROW_HEIGHT: f32 = skill_row::ROW_HEIGHT;
const HOW_WIDTH: f32 = 300.0;
/// F8mJNd's first-skill card; sources grow the found variant from its content.
const EMPTY_HEIGHT: f32 = 216.0;
pub struct Library;
impl Component for Library {
  type Props = PageProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<PageProps>().clone();
    let header = header(ctx, &p);
    let page = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .left(ConnectionsSpacing::Page.role())
          .right(ConnectionsSpacing::Page.role()),
      )
      .spacing(if p.data.listed.entries.is_empty() {
        crate::theme::skills::section_spacing()
      } else {
        crate::theme::skills::cell_spacing()
      })
      .child(header);
    let content: Element = if p.data.listed.entries.is_empty() {
      let empty = empty(ctx, &p);
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(lurq::layout::Alignment::Start)
        .spacing(SpacingSize::Lg)
        .child(empty)
        .child(
          Column::new()
            .width(Dimension::Px(HOW_WIDTH))
            .spacing(crate::theme::skills::cell_spacing())
            .child(guidance::how(ctx, true))
            .child(ownership(ctx, &p, true))
            .child(repository::found(ctx, &p)),
        )
        .into()
    } else {
      let tabs = self.tabs(ctx, &p);
      let table = self.table(ctx, &p);
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(crate::theme::skills::cell_spacing())
        .child(tabs)
        .child(table)
        .child(guidance::how(ctx, false))
        .child(ownership(ctx, &p, false))
        .child(repository::found(ctx, &p))
        .into()
    };
    ScrollVertical::new(page.child(content))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
  }
}
fn accepts(filter: SkillsFilter, scope: &InstructionScope) -> bool {
  match filter {
    SkillsFilter::All => true,
    SkillsFilter::Project => matches!(scope, InstructionScope::Project),
    SkillsFilter::Roles => matches!(scope, InstructionScope::AgentRole { .. }),
    SkillsFilter::Pipelines => matches!(scope, InstructionScope::Pipeline { .. } | InstructionScope::Step { .. }),
  }
}
impl Library {
  fn tabs(&self, ctx: &mut Ctx, p: &PageProps) -> Element {
    let tabs = [
      (SkillsFilter::All, "skills.tabs.all"),
      (SkillsFilter::Project, "skills.tabs.project"),
      (SkillsFilter::Roles, "skills.tabs.roles"),
      (SkillsFilter::Pipelines, "skills.tabs.pipelines"),
    ]
    .map(|(filter, key)| {
      let panel = p.view.panel.clone();
      let count = p
        .data
        .listed
        .entries
        .iter()
        .filter(|e| accepts(filter, e.body.scope()))
        .count();
      PageTab::new(
        format!("skills-tab-{filter:?}"),
        counted(ctx, &ctx.t(key), count),
        p.filter == filter,
        move || panel.set(Panel::Library(filter)),
      )
    });
    page_tabs(ctx, "skills-tabs", tabs).into()
  }
  fn table(&self, ctx: &mut Ctx, p: &PageProps) -> Element {
    let entries = p
      .data
      .listed
      .entries
      .iter()
      .filter(|e| accepts(p.filter, e.body.scope()))
      .cloned()
      .collect::<Vec<_>>();
    let headings = [
      "skills.columns.skill",
      "skills.columns.files",
      "skills.columns.version",
      "skills.columns.scope",
      "skills.columns.published",
    ]
    .map(|k| ctx.t(k));
    let count = entries.len();
    let data = p.data.clone();
    let view = p.view.clone();
    let rows = VirtualizedList::new(ctx, entries)
      .list_key("skill-rows")
      .height(Dimension::Px(ROW_HEIGHT * count.min(6) as f32))
      .width(Dimension::Pct(100.0))
      .mount_keyed::<SkillRow, _, _, _>(
        |e| e.id.to_string(),
        move |e| RowProps {
          entry: e.clone(),
          data: data.clone(),
          view: view.clone(),
        },
      );
    let drafts = p.data.listed.entries.iter().filter(|e| e.state.has_draft()).count();
    Column::new()
      .id("skills-list")
      .width(Dimension::Pct(100.0))
      .child(skill_row::header("skills-list-header", &headings))
      .child(rows)
      .with_children((count == 0).then(|| Text::new(&ctx.t("skills.noMatches")).variant(TypographyStyle::Description)))
      .child(
        Text::new(&ctx.t_args(
          "skills.count",
          [
            ("count", p.data.listed.entries.len().to_string()),
            ("drafts", drafts.to_string()),
          ],
        ))
        .variant(ExtraTypography::Note.role())
        .padding(SpacingSize::Sm),
      )
      .into()
  }
}
fn header(ctx: &Ctx, p: &PageProps) -> Row {
  let import = p.view.panel.clone();
  let new = p.view.panel.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(SpacingSize::Xs))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("skills.title")).variant(ExtraTypography::Display.role()))
        .child(Text::new(&ctx.t("skills.description")).variant(crate::theme::skills::library_lead())),
    )
    .child(
      ActionButton::new("skills-import-open", ButtonKind::Secondary, ctx.t("skills.import"))
        .icon(Icon::Download)
        .on_click(move || import.set(Panel::Import)),
    )
    .child(
      ActionButton::new("skills-new", ButtonKind::Primary, ctx.t("skills.new"))
        .icon(Icon::Plus)
        .on_click(move || new.set(Panel::New)),
    )
}
fn empty(ctx: &mut Ctx, p: &PageProps) -> Column {
  let found = !p.data.repository.items.is_empty();
  let mut card = Column::new()
    .id("skills-empty")
    .min_height(Dimension::Px(if found { 0.0 } else { EMPTY_HEIGHT }))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Md)
    .padding(ExtraSpacing::Roomy.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(crate::theme::ExtraRadius::Tile.role())
    .child(marker(Icon::GraduationCap, 36.0))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t_args(
            "skills.emptyTitle",
            [("project", p.project.open.project.name().as_str())],
          ))
          .width(Dimension::Pct(100.0))
          .variant(ExtraTypography::Strong.role()),
        )
        .child(
          Text::new(&ctx.t_args(
            if found {
              "skills.emptyDetail"
            } else {
              "skills.emptyFirstDetail"
            },
            [("project", p.project.open.project.name().as_str())],
          ))
          .width(Dimension::Pct(100.0))
          .variant(TypographyStyle::Description),
        ),
    );
  if found {
    card = card.child(repository::inventory(ctx, p));
  }
  card.child(empty_actions(ctx, p))
}
fn ownership(ctx: &Ctx, p: &PageProps, empty: bool) -> Element {
  note(
    "skills-ownership",
    Icon::FolderGit2,
    &ctx.t_args(
      if empty && p.data.repository.items.is_empty() {
        "skills.noRepository"
      } else if empty {
        "skills.emptyOwnership"
      } else {
        "skills.ownership"
      },
      [("project", p.project.open.project.name().as_str())],
    ),
  )
  .into()
}
#[derive(Clone)]
struct RowProps {
  entry: orchester_workspace::skills::data::SkillEntry,
  data: orchester_workspace::skills::editor::EditorData,
  view: super::state::View,
}
impl PartialEq for RowProps {
  fn eq(&self, o: &Self) -> bool {
    self.entry == o.entry && self.data == o.data
  }
}
impl lurq::app::component::DevtoolsInspectable for RowProps {}
struct SkillRow;
impl Component for SkillRow {
  type Props = RowProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<RowProps>().clone();
    let entry = p.entry;
    let panel = p.view.panel.clone();
    let id = entry.id;
    let published = entry.last_published.map_or_else(
      || ctx.t("skills.never"),
      |time| {
        crate::ui::message::message_text(
          ctx,
          &orchester_workspace::display::elapsed(orchester_core::time::Timestamp::now(), time),
        )
      },
    );
    let version_mark = if entry.state.has_draft() {
      Icon::FilePenLine
    } else {
      Icon::CircleCheck
    };
    let scope_mark = match entry.body.scope() {
      InstructionScope::Project => Icon::FolderGit2,
      InstructionScope::AgentRole { .. } => Icon::Bot,
      InstructionScope::Pipeline { .. } => Icon::Workflow,
      InstructionScope::Step { .. } => Icon::GitCommitHorizontal,
    };
    skill_row::SkillRow::new(
      format!("skill-row-{id}"),
      entry.name.clone(),
      entry.body.description(),
      [
        words::files(ctx, entry.body.files().len()),
        words::version(ctx, entry.state),
        words::scope(ctx, &p.data, entry.body.scope()),
        published,
      ],
      move || panel.set(Panel::Edit(id)),
    )
    .marks(version_mark, scope_mark)
  }
}
fn empty_actions(ctx: &Ctx, p: &PageProps) -> Row {
  let found = !p.data.repository.items.is_empty();
  let new = p.view.panel.clone();
  let import = p.view.panel.clone();
  let mut actions = Row::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
  if found {
    actions = actions.child(
      ActionButton::new("skills-empty-import", ButtonKind::Primary, ctx.t("skills.import"))
        .icon(Icon::Download)
        .on_click(move || import.set(Panel::Import)),
    );
  }
  actions = actions.child(
    ActionButton::new(
      "skills-empty-new",
      if found {
        ButtonKind::Secondary
      } else {
        ButtonKind::Primary
      },
      ctx.t("skills.new"),
    )
    .icon(Icon::Plus)
    .on_click(move || new.set(Panel::New)),
  );
  if !found {
    actions = actions.child(
      Text::new(&ctx.t_args(
        "skills.emptyNewNote",
        [("project", p.project.open.project.name().as_str())],
      ))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .variant(ExtraTypography::Note.role()),
    );
  }
  actions
}
