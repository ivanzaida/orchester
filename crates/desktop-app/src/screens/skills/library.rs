//! Library Syllb and empty states WMT1W/F8mJNd.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text, VirtualizedList},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InstructionScope;

use super::{
  PageProps, repository,
  state::{Filter, Panel},
  words,
};
use crate::{
  theme::{ConnectionsSpacing, ConnectionsText, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::note,
    skill_row,
    tabs::{PageTab, counted, page_tabs},
  },
};
/// Tv2Ei columns and its two-line row.
const ROW_HEIGHT: f32 = skill_row::ROW_HEIGHT;
const HOW_WIDTH: f32 = 300.0;
pub struct Library {
  filter: Signal<Filter>,
}
impl Component for Library {
  type Props = PageProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      filter: ctx.signal(Filter::All),
    }
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
        .spacing(SpacingSize::Lg)
        .child(empty)
        .child(Column::new().width(Dimension::Px(HOW_WIDTH)).child(how(ctx, true)))
        .into()
    } else {
      let tabs = self.tabs(ctx, &p);
      let table = self.table(ctx, &p);
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(crate::theme::skills::cell_spacing())
        .child(tabs)
        .child(table)
        .child(how(ctx, false))
        .into()
    };
    ScrollVertical::new(page.child(content).child(note(
      "skills-ownership",
      Icon::FolderGit2,
      &ctx.t_args(
        if p.data.repository.items.is_empty() {
          "skills.noRepository"
        } else {
          "skills.ownership"
        },
        [("project", p.project.open.project.name().as_str())],
      ),
    )))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
  }
}
fn accepts(filter: Filter, scope: &InstructionScope) -> bool {
  match filter {
    Filter::All => true,
    Filter::Project => matches!(scope, InstructionScope::Project),
    Filter::Roles => matches!(scope, InstructionScope::AgentRole { .. }),
    Filter::Pipelines => matches!(scope, InstructionScope::Pipeline { .. } | InstructionScope::Step { .. }),
  }
}
impl Library {
  fn tabs(&self, ctx: &mut Ctx, p: &PageProps) -> Element {
    let tabs = [
      (Filter::All, "skills.tabs.all"),
      (Filter::Project, "skills.tabs.project"),
      (Filter::Roles, "skills.tabs.roles"),
      (Filter::Pipelines, "skills.tabs.pipelines"),
    ]
    .map(|(filter, key)| {
      let value = self.filter.clone();
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
        self.filter.get() == filter,
        move || value.set(filter),
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
      .filter(|e| accepts(self.filter.get(), e.body.scope()))
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
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("skills.title")).variant(ExtraTypography::Display.role()))
        .child(Text::new(&ctx.t("skills.description")).variant(ConnectionsText::Lead.role())),
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
  let new = p.view.panel.clone();
  let found = repository::found(ctx, p);
  Column::new()
    .id("skills-empty")
    .flex(1.0)
    .spacing(SpacingSize::Md)
    .padding(ExtraSpacing::Roomy.role())
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(crate::theme::ExtraRadius::Tile.role())
    .child(
      Text::new(&ctx.t_args(
        "skills.emptyTitle",
        [("project", p.project.open.project.name().as_str())],
      ))
      .variant(ExtraTypography::Strong.role()),
    )
    .child(Text::new(&ctx.t("skills.emptyDetail")).variant(TypographyStyle::Description))
    .child(found)
    .child(
      ActionButton::new("skills-empty-new", ButtonKind::Primary, ctx.t("skills.new"))
        .icon(Icon::Plus)
        .on_click(move || new.set(Panel::New)),
    )
}
pub fn how(ctx: &Ctx, vertical: bool) -> Element {
  let items = [
    ("skills.howScopeTitle", "skills.howScope"),
    ("skills.howPublishTitle", "skills.howPublish"),
    ("skills.howKeepTitle", "skills.howKeep"),
  ]
  .map(|(title, body)| {
    Column::new()
      .flex(1.0)
      .spacing(ExtraSpacing::Fine.role())
      .child(Text::new(&ctx.t(title)).variant(ExtraTypography::Strong.role()))
      .child(Text::new(&ctx.t(body)).variant(ExtraTypography::Note.role()))
  });
  let content: Element = if vertical {
    Column::new().spacing(SpacingSize::Lg).with_children(items).into()
  } else {
    Row::new().spacing(SpacingSize::Lg).with_children(items).into()
  };
  let title = Text::new(&ctx.t("skills.how")).variant(ExtraTypography::Strong.role());
  if vertical {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfaceRaised)
      .rounded(crate::theme::ExtraRadius::Tile.role())
      .child(title)
      .child(content)
      .into()
  } else {
    // Syllb's how strip places its title in a 170px column.
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Lg)
      .background(PaletteColor::SurfaceRaised)
      .rounded(crate::theme::ExtraRadius::Tile.role())
      .child(Column::new().width(Dimension::Px(170.0)).child(title))
      .child(Column::new().flex(1.0).child(content))
      .into()
  }
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
