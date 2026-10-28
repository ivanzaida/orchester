//! The top of a tool's page (`04b` `nuoSy`): the crumb back to Tools, the head with the tool's name, kind, standing
//! and callers and the … menu with Remove, the draft bar, and the page's tabs — Configuration, Inputs & outputs,
//! Access and Usage log.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::{agents::data::VersionState, tools::data::ToolEntry};

use super::{editor::EditorProps, words};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, ToolTab},
  screens::hub::{
    draft_bar::{DraftBar, Standing},
    page_head::{self, More},
  },
  ui::{
    icon::Icon,
    tabs::{PageTab, page_tabs},
  },
};

pub(super) fn crumb(ctx: &mut Ctx, p: &EditorProps) -> Element {
  let tools = Navigation::of(ctx).to(Route::Project(p.project.open.project.id(), ProjectPage::Tools));
  let new = ctx.t("tools.newTitle");
  page_head::crumb(
    "tool-crumbs",
    (&ctx.t("hub.tabs.tools"), tools),
    p.entry.is_none().then_some(&*new),
  )
}

/// The head: `title`, then the kind, whether it runs and how many steps call it.
pub(super) fn head(
  ctx: &mut Ctx,
  p: &EditorProps,
  (entry, title): (Option<&ToolEntry>, &str),
  more: Option<More>,
) -> Element {
  let (glyph, sub) = match entry {
    Some(entry) => {
      let (kind, glyph) = words::kind(&entry.tool);
      let standing = ctx.t(if !entry.runnable() {
        "tools.unavailable"
      } else if entry.state.published().is_some() {
        "tools.ready"
      } else {
        "tools.draft"
      });
      let used = super::words::used_by(ctx, p.data.listed.used_by(entry.id).count());
      let sub = ctx.t_args(
        "tools.pageSub",
        [
          ("kind", ctx.t(kind).to_string()),
          ("standing", standing.to_string()),
          ("used", used.to_string()),
        ],
      );
      (glyph, sub)
    }
    None => (Icon::Terminal, ctx.t("tools.newDetail")),
  };
  page_head::head(ctx, "tool", (glyph, title, &sub), more)
}

/// What the bar says about the tool now, with its actions.
pub(super) fn bar(
  ctx: &Ctx,
  entry: Option<&ToolEntry>,
  (status, unchanged): (Arc<str>, bool),
  actions: Vec<Element>,
) -> Element {
  let standing = match entry.map(|e| e.state) {
    Some(VersionState::Published(_)) if unchanged => Standing::Published,
    Some(VersionState::StaleDraft { .. }) => Standing::Attention,
    _ => Standing::Draft,
  };
  let sub = match entry.map(|e| (e.state, super::editor::next_version(Some(e)))) {
    None => ctx.t("tools.barSubNew"),
    Some((VersionState::Published(_), _)) => ctx.t("tools.barSubPublished"),
    Some((_, Some(next))) => ctx.t_args("tools.barSubDraft", [("version", next.get().to_string())]),
    Some((_, None)) => ctx.t("tools.versionLimit"),
  };
  let mut bar = DraftBar::new("tool-draft-bar", standing, status).sub(sub);
  for action in actions {
    bar = bar.action(action);
  }
  bar.into()
}

pub(super) fn tabs(ctx: &mut Ctx, p: &EditorProps) -> Element {
  let current = match p.page {
    ProjectPage::Tool(_, tab) => tab,
    _ => ToolTab::Configuration,
  };
  let project = p.project.open.project.id();
  let nav = Navigation::of(ctx);
  let tabs = ToolTab::ALL
    .into_iter()
    .map(|tab| {
      let label = ctx.t(&format!("tools.tabs.{}", tab.name()));
      let key = format!("tool-tab-{}", tab.name());
      let Some(id) = p.entry.as_ref().map(|e| e.id) else {
        if tab != ToolTab::Configuration {
          return PageTab::unavailable(key, label, ctx.t("tools.saveFirst"));
        }
        return PageTab::to(key, label, true, nav.to(Route::Project(project, ProjectPage::NewTool)));
      };
      let item = PageTab::to(
        key,
        label,
        tab == current,
        nav.to(Route::Project(project, ProjectPage::Tool(id, tab))),
      );
      match tab {
        ToolTab::UsageLog => item.explanation(ctx.t("tools.locked")),
        _ => item,
      }
    })
    .collect::<Vec<_>>();
  page_tabs(ctx, "tool-tabs", tabs).into()
}
