//! A tool's page (`04b` `nuoSy`): the crumb, head, draft bar and tabs, then the tab with the tool's project and access
//! beside it. A command tool saves as a draft on its own once its fields check out; a change that widens what it
//! may do asks at its field first (*Allow this command tool?*), and publishing asks again only for what was not
//! allowed yet. Questions open in place, never in a modal.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_store::SharedStanding;
pub(super) use orchester_workspace::tools::editor::next_version;
use orchester_workspace::{
  agents::data::VersionState,
  messages::Message,
  sharing::team::TeamConfiguration,
  tasks::outside::Outside,
  tools::{
    data::ToolEntry,
    editor::{Edit, EditorData},
    form::{CommandErrors, CommandText},
  },
};

use super::{
  fields::Fields,
  head, inspector, words,
  work::{self, Work},
};
use crate::{
  navigation::Navigation,
  project::{ShownProject, own::OwnChanges},
  routes::{ProjectPage, Route, ToolTab},
  screens::hub::{self, page_head::More},
  theme::HubSpacing,
  ui::{icon::Icon, inspector::note, message::message_text},
};

/// Reads the tools again and starts the page over from what is saved.
#[derive(Clone)]
pub struct Reload {
  pub revision: Signal<u64>,
  /// The revision the page started over at: the page waits for a read at least this new.
  pub reset_at: Signal<u64>,
}
impl Reload {
  pub fn run(&self) {
    let next = self.revision.get_untracked() + 1;
    self.revision.set(next);
    self.reset_at.set(next);
  }
}

#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  pub data: EditorData,
  pub entry: Option<ToolEntry>,
  pub revision: Signal<u64>,
  pub reload: Reload,
  /// Where a save records its change of the tools.
  pub tools: OwnChanges,
  pub read_revision: u64,
  pub page: ProjectPage,
}
impl PartialEq for EditorProps {
  fn eq(&self, o: &Self) -> bool {
    self.project == o.project
      && self.data == o.data
      && self.entry == o.entry
      && self.page == o.page
      && self.read_revision == o.read_revision
  }
}
impl DevtoolsInspectable for EditorProps {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Asking(pub Option<Edit>);
impl DevtoolsInspectable for Asking {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Baseline(pub Option<ToolEntry>);
impl DevtoolsInspectable for Baseline {}
/// The widenings the person allowed on this page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct AllowedHere(pub Vec<Message>);
impl DevtoolsInspectable for AllowedHere {}
/// The tool the last save sent, so a failed save is not sent again on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Sent(pub Option<orchester_core::resource::Tool>);
impl DevtoolsInspectable for Sent {}
/// A question about the tool itself, asked in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ask {
  Nothing,
  Discard,
  Remove,
}
impl DevtoolsInspectable for Ask {}
pub struct ToolEditor {
  pub(super) action: Work,
  pub(super) fields: Fields,
  pub(super) baseline: Signal<Baseline>,
  pub(super) asking: Signal<Asking>,
  pub(super) busy: Signal<bool>,
  pub(super) allowed: Signal<AllowedHere>,
  pub(super) sent: Signal<Sent>,
  /// The write under way replaces the draft with the latest version: the page starts over once it ends.
  pub(super) restart: Signal<bool>,
  pub(super) ask: Signal<Ask>,
  more: Signal<bool>,
  anchor: ElementRef,
}
impl Component for ToolEditor {
  type Props = EditorProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let text = props
      .entry
      .as_ref()
      .and_then(|e| CommandText::of(&e.tool))
      .unwrap_or_else(CommandText::new_tool);
    let name = props.entry.as_ref().map_or("", |e| &*e.name);
    let editor = Self {
      action: work::action(ctx),
      fields: Fields::new(ctx, name, &text),
      baseline: ctx.signal(Baseline(props.entry.clone())),
      asking: ctx.signal(Asking(None)),
      busy: ctx.signal(false),
      allowed: ctx.signal(AllowedHere::default()),
      sent: ctx.signal(Sent::default()),
      restart: ctx.signal(false),
      ask: ctx.signal(Ask::Nothing),
      more: ctx.signal(false),
      anchor: ctx.element_ref(),
    };
    let action = editor.action.clone();
    let (revision, busy, restart) = (props.revision.clone(), editor.busy.clone(), editor.restart.clone());
    let (tools, reload) = (props.tools.clone(), props.reload.clone());
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    ctx.watch(&action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      busy.set(false);
      if let Some(saved) = state.data.filter(|_| state.is_fulfilled()) {
        // The save recorded its change; this screen reads the tools again once, on its revision.
        tools.saw(saved.1);
        let tab = match navigation.route_now() {
          Route::Project(_, ProjectPage::Tool(_, tab)) => tab,
          _ => ToolTab::Configuration,
        };
        navigation.replace(Route::Project(project, ProjectPage::Tool(saved.0, tab)));
        // A draft started again from the latest version starts the page over from it.
        if restart.get_untracked() {
          restart.set(false);
          reload.run();
        } else {
          revision.update(|r| *r += 1);
        }
      }
    });
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<EditorProps>().clone();
    self.adopt(&p);
    let baseline = self.baseline.get().0;
    let (checked, checking) = super::validation::checked(ctx, &self.fields, baseline.as_ref(), &p);
    let unchanged = baseline.as_ref().is_some_and(|e| {
      let same_text = CommandText::of(&e.tool).is_some_and(|text| text == self.fields.text());
      same_text || (!checking && matches!(&checked, Ok(Edit::Save { tool, .. }) if **tool == e.tool))
    });
    let outside = baseline != p.entry;
    let busy = self.busy.get();
    let read_only = baseline.as_ref().is_some_and(|e| CommandText::of(&e.tool).is_none());
    let team = hub::team::of(&p.project);
    // A member of a shared project reads the team's tool: nothing of it is saved or removed here.
    let may_change = hub::team::may_change(team.as_ref());
    let widening = match &checked {
      Ok(edit) if baseline.is_some() && !unchanged && !checking && !outside && !read_only && may_change => {
        self.autosave(&p, edit)
      }
      _ => None,
    };
    let errors = checked.as_ref().err().cloned().unwrap_or_default();
    let title = baseline.as_ref().map_or(ctx.t("tools.newTitle"), |e| e.name.clone());
    let more = baseline.as_ref().filter(|_| may_change).map(|_| self.more_menu());
    let standing = team
      .as_ref()
      .zip(baseline.as_ref())
      .and_then(|(team, e)| Some((team, team.standing(e.id)?, e)));
    let shared = standing.map(|(team, standing, e)| {
      hub::team::head_line(ctx, "tool-team-state", team, standing, hub::team::draft_on(e.state))
    });
    let versions = baseline
      .as_ref()
      .and_then(|e| hub::team::member_versions(ctx, "tool", (&p.project, team.as_ref()), e.id));
    let mut pane = Column::new()
      .id("tool-editor")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(head::crumb(ctx, &p))
      .child(head::head(
        ctx,
        &p,
        (baseline.as_ref(), &title),
        more,
        (shared, versions),
      ))
      .with_children(self.remove_question(ctx, &p, baseline.as_ref()));
    pane = match (self.discard_question(ctx, &p, baseline.as_ref()), self.asking.get().0) {
      _ if !may_change => pane.with_children(team.as_ref().map(|team| hub::team::line(ctx, "tool-team-line", team))),
      (Some(question), _) => pane.child(question),
      (None, Some(edit)) => pane.child(self.confirm(ctx, &p, edit)),
      (None, None) => match waiting(ctx, &p, standing.map(|(team, standing, _)| (team, standing))) {
        Some(waiting) => pane.child(waiting),
        None => {
          let status = words::editor_state(ctx, baseline.as_ref(), unchanged, busy).0;
          let actions = self.bar_actions(ctx, &p, (&checked, checking), (unchanged, outside));
          pane.child(head::bar(
            ctx,
            (baseline.as_ref(), team.is_some()),
            (status, unchanged),
            actions,
          ))
        }
      },
    };
    pane = pane
      .with_children(self.failure(ctx))
      .child(head::tabs(ctx, &p))
      .child(self.body(
        ctx,
        &p,
        &errors,
        (busy || self.asking.get().0.is_some() || !may_change, read_only),
        widening,
      ));
    hub::item_body("tool-page", pane, Some(inspector::inspector(ctx, &p).into()))
  }
}

/// In a shared project, a version of the tool published here that waits, in the draft bar's place (02b, 02c); after
/// *Open my draft on vN* the editor reads the saved tool again.
fn waiting(ctx: &mut Ctx, p: &EditorProps, standing: Option<(&TeamConfiguration, &SharedStanding)>) -> Option<Element> {
  let reload = p.reload.clone();
  let reread: hub::team::Reread = Arc::new(move || reload.run());
  hub::team::waiting_bar(ctx, &p.project, standing?, ("tool", Outside::Tools), reread)
}

impl ToolEditor {
  /// The ⋯ menu of a saved tool, whose Remove asks first.
  fn more_menu(&self) -> More {
    let (ask, open) = (self.ask.clone(), self.more.clone());
    More {
      open: self.more.clone(),
      anchor: self.anchor.clone(),
      remove: Arc::new(move || {
        open.set(false);
        ask.set(Ask::Remove);
      }),
    }
  }

  /// A read that shows what this page itself wrote, or the same tool in another standing, becomes the page's
  /// baseline; any other change waits for the person's Reload.
  fn adopt(&self, p: &EditorProps) {
    let baseline = self.baseline.get_untracked().0;
    let Some(entry) = p.entry.as_ref().filter(|entry| Some(*entry) != baseline.as_ref()) else {
      return;
    };
    let ours = baseline
      .as_ref()
      .is_some_and(|b| b.id == entry.id && b.tool == entry.tool)
      || self.sent.get_untracked().0.as_ref() == Some(&entry.tool);
    if ours && !self.busy.get_untracked() {
      self.baseline.set(Baseline(Some(entry.clone())));
    }
  }

  /// Saves `edit` when it changes the draft, unless it widens what the tool may do beyond what the person allowed
  /// here: then the reasons, for the question at the field.
  fn autosave(&self, p: &EditorProps, edit: &Edit) -> Option<(Edit, Vec<Message>)> {
    let Edit::Save { tool, .. } = edit else {
      return None;
    };
    if self.busy.get() || self.asking.get().0.is_some() || self.sent.get().0.as_ref() == Some(&**tool) {
      return None;
    }
    let reasons = edit.reasons(None);
    let allowed = self.allowed.get().0;
    if reasons.iter().any(|reason| !allowed.contains(reason)) {
      return Some((edit.clone(), reasons));
    }
    self.starter(p).run(edit.clone(), reasons);
    None
  }

  fn body(
    &self,
    ctx: &mut Ctx,
    p: &EditorProps,
    errors: &CommandErrors,
    (locked, read_only): (bool, bool),
    widening: Option<(Edit, Vec<Message>)>,
  ) -> Element {
    let tab = match p.page {
      ProjectPage::Tool(_, tab) => tab,
      _ => ToolTab::Configuration,
    };
    let baseline = self.baseline.get().0;
    let mut main = Column::new()
      .id("tool-tab-body")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .with_children(self.outside_change(ctx, p));
    if read_only {
      return main.child(super::editor_asks::read_only(ctx, p)).into();
    }
    main = match (tab, baseline.as_ref()) {
      (ToolTab::Configuration, _) | (_, None) => main
        .with_children(self.stale(ctx, p, locked))
        .child(self.fields.render(ctx, errors, baseline.is_some(), locked))
        .with_children(widening.map(|(edit, reasons)| self.widening(ctx, edit, reasons))),
      (ToolTab::InputsOutputs, Some(entry)) => main.child(super::inputs::inputs(ctx, entry)),
      (ToolTab::Access, _) => main.child(super::access::access(ctx, p)),
      (ToolTab::UsageLog, _) => main.child(
        crate::ui::state::EmptyState::new(
          "tool-usage-unavailable",
          Icon::History,
          ctx.t("tools.usageTitle"),
          ctx.t("tools.usageDetail"),
        )
        .wide(),
      ),
    };
    main
      .child(note("tool-execution", Icon::TriangleAlert, &ctx.t("tools.execution")))
      .into()
  }

  /// A failed write, with what it said; an edit sends the save again.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    let state = self.action.state().get();
    let error = state.error.as_ref().filter(|_| !state.is_pending())?;
    Some(
      // What was typed stays: the `danger` icon only, no red fill (`SsC16`).
      crate::ui::notice::Notice::new(
        "tool-save-error",
        crate::ui::notice::NoticeTone::Info,
        ctx.t("tools.saveFailed"),
      )
      .icon(Icon::CircleAlert)
      .mark(lurq::app::theme::PaletteColor::Danger)
      .body(message_text(ctx, error))
      .into(),
    )
  }

  pub(super) fn stale_state(&self) -> Option<(ToolEntry, VersionState)> {
    let entry = self.baseline.get().0?;
    matches!(entry.state, VersionState::StaleDraft { .. }).then(|| (entry.clone(), entry.state))
  }
}
