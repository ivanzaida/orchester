//! A skill's page (`03b Skill` `QbDWX`), one page under the hub's tabs: the crumb, head and draft bar, then the skill's
//! name, scope and description, its body and its reference files, with where it loads, its versions, origin and
//! active runs beside them. An edit saves the draft on its own; publishing asks first. Questions open in place, never
//! in a modal.

mod asks;
mod files;
mod form;
mod header;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::{InstructionScope, Skill, SkillFile};
use orchester_workspace::{messages::Message, skills::data::SkillEntry, tasks::outside::Outside};

use super::{
  PageProps, inspector,
  work::{self, Change, Saved, Work},
};
use crate::{screens::hub, theme::HubSpacing};
/// A field's width in the *Skill* rows (`zalHn`, `MdHMb`) and the description's least height, one line (`P1kVO`).
const FIELD_WIDTH: f32 = 280.0;
const DESCRIPTION_HEIGHT: f32 = 39.0;
/// The body's editor (`Area` `IaAfU`); its lines scroll inside it.
const BODY_HEIGHT: f32 = 130.0;
/// The page's main column (`Pane` `FgUP6`) and its side (`Side` `jGRYJ`).
const MAIN_WIDTH: f32 = 680.0;
const SIDE_WIDTH: f32 = 424.0;
#[derive(Clone)]
pub struct Props {
  pub page: PageProps,
  pub entry: Option<SkillEntry>,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.page == o.page && self.entry == o.entry
  }
}
impl DevtoolsInspectable for Props {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files(pub Vec<SkillFile>);
impl DevtoolsInspectable for Files {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Baseline(Option<SkillEntry>);
impl DevtoolsInspectable for Baseline {}
/// A question about the skill itself, asked in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
  Nothing,
  Discard,
  Remove,
}
impl DevtoolsInspectable for Ask {}
pub struct Editor {
  name: Signal<String>,
  description: Signal<String>,
  body: Signal<String>,
  scope: Signal<orchester_workspace::instructions::form::Picked<InstructionScope>>,
  pub files: Signal<Files>,
  choosing_file: Signal<bool>,
  baseline: Signal<Baseline>,
  action: Work,
  operation: Signal<work::Operation>,
  asking: Signal<bool>,
  submitted: Signal<bool>,
  dirty: Signal<bool>,
  leave: crate::navigation::guard::LeaveGuard,
  authorship: work::Authorship,
  ask: Signal<Ask>,
  more: Signal<bool>,
  anchor: ElementRef,
}
impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let p = ctx.props::<Props>().clone();
    let entry = p.entry.as_ref();
    let scope = orchester_workspace::instructions::form::Picked(
      entry.map_or(InstructionScope::Project, |e| e.body.scope().clone()),
    );
    let dirty = ctx.signal(false);
    let edits = dirty.clone();
    let leave = crate::navigation::Navigation::of(ctx).guard(move || edits.get_untracked());
    let editor = Self {
      name: ctx.signal(entry.map_or(String::new(), |e| e.name.to_string())),
      description: ctx.signal(entry.map_or(String::new(), |e| e.body.description().to_owned())),
      body: ctx.signal(entry.map_or(String::new(), |e| e.body.body().to_owned())),
      scope: ctx.signal(scope),
      files: ctx.signal(Files(entry.map_or(Vec::new(), |e| e.body.files().to_vec()))),
      choosing_file: ctx.signal(false),
      dirty,
      leave,
      baseline: ctx.signal(Baseline(p.entry.clone())),
      action: work::action(ctx),
      operation: ctx.signal(work::Operation::Save),
      asking: ctx.signal(false),
      submitted: ctx.signal(false),
      authorship: work::Authorship::default(),
      ask: ctx.signal(Ask::Nothing),
      more: ctx.signal(false),
      anchor: ctx.element_ref(),
    };
    editor.watch_edits(ctx);
    editor.watch_writes(ctx, &p);
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<Props>().clone();
    let baseline = self.baseline.get().0;
    let (parsed, edits) = self.form_state(&p);
    let state = self.action.state().get();
    let dirty = parsed.as_ref().is_ok_and(|body| {
      baseline
        .as_ref()
        .is_none_or(|e| e.body != *body || e.name.as_ref() != self.name.get().trim())
    });
    if self.asking.get() && (dirty || parsed.is_err() || p.page.reloading) {
      self.asking.set(false);
    }
    // A member of a shared project reads the team's skill: nothing of it is saved here.
    let may_change = hub::team::may_change(hub::team::of(&p.page.project).as_ref());
    if baseline.is_some()
      && dirty
      && may_change
      && !state.is_pending()
      && state.error.is_none()
      && !self.submitted.get()
      && !self.asking.get()
      && !p.page.reloading
    {
      self.save(&p, parsed.as_ref().expect("dirty means parsing passed").clone());
    }
    let busy = self.submitted.get() || state.is_pending() || p.page.reloading;
    let dirty_or_invalid = edits || busy;
    if self.dirty.get_untracked() != dirty_or_invalid {
      self.dirty.set(dirty_or_invalid);
    }
    let title = baseline
      .as_ref()
      .map_or_else(|| ctx.t("skills.new"), |e| e.name.clone());
    let changed = (dirty || parsed.is_err(), busy);
    // The crumb, the head and the draft bar sit closer than the sections under them (`Top` `KVCXI`).
    let mut top = Column::new()
      .id("skill-top")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(self.crumb(ctx, &p, baseline.as_ref()))
      .child(self.page_head(ctx, &p, baseline.as_ref(), &title))
      .with_children(self.leave_question(ctx, busy))
      .with_children(self.remove_question(ctx, &p, baseline.as_ref()));
    top = match (self.discard_question(ctx, &p, baseline.as_ref()), self.asking.get()) {
      (Some(question), _) => top.child(question),
      (None, true) => top.child(self.question(ctx, &p, baseline.as_ref())),
      (None, false) => top.child(self.bar(ctx, &p, baseline.as_ref(), changed)),
    };
    let form = self.form(ctx, &p, parsed.as_ref().err(), busy);
    let pane = Column::new()
      .id("skill-editor")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(top)
      .with_children(self.failure(ctx, &p))
      .with_children(self.reload_question(ctx, &p, busy))
      .with_children(form.into_iter().map(|section| -> Element {
        if may_change {
          section
        } else {
          Column::new()
            .width(Dimension::Pct(100.0))
            .pointer_events_none()
            .child(section)
            .into()
        }
      }));
    let side = inspector::panel(ctx, &p.page, baseline.as_ref());
    hub::page_body("skill-page", (pane.into(), MAIN_WIDTH), (side, SIDE_WIDTH))
  }
}
impl Editor {
  /// Signals change while lurq delivers the input that changed them, so a client's edit is told from a person's.
  fn watch_edits(&self, ctx: &mut Ctx) {
    let authorship = self.authorship.clone();
    ctx.watch(&self.name, move |_| authorship.edited());
    let authorship = self.authorship.clone();
    ctx.watch(&self.description, move |_| authorship.edited());
    let authorship = self.authorship.clone();
    ctx.watch(&self.body, move |_| authorship.edited());
    let authorship = self.authorship.clone();
    ctx.watch(&self.scope, move |_| authorship.edited());
    let authorship = self.authorship.clone();
    ctx.watch(&self.files, move |_| authorship.edited());
  }
  fn watch_writes(&self, ctx: &mut Ctx, p: &Props) {
    let baseline = self.baseline.clone();
    let (revision, skills) = (p.page.view.revision.clone(), p.page.view.skills.clone());
    let changes = crate::services::Services::of(ctx).changes;
    let submitted = self.submitted.clone();
    let dirty_state = self.dirty.clone();
    let navigation = p.page.view.panel.navigation.clone();
    let project = p.page.view.panel.project;
    let creating = p.entry.is_none();
    ctx.watch(&self.action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      submitted.set(false);
      if state.is_fulfilled()
        && let Some(Saved(entry)) = state.data.as_ref()
      {
        baseline.set(Baseline(Some(entry.clone())));
        // Every other screen that shows the skills reads them again; this one reads them again once.
        skills.record(&changes, project, Outside::Skills);
        revision.update(|r| *r += 1);
        if creating {
          dirty_state.set(false);
          navigation.replace(crate::routes::Route::Project(
            project,
            crate::routes::ProjectPage::Skill(entry.id),
          ));
        }
      }
    });
  }
  fn form_state(&self, p: &Props) -> (Result<Skill, Message>, bool) {
    let form = orchester_workspace::skills::editor::SkillForm {
      name: &self.name.get(),
      description: &self.description.get(),
      body: &self.body.get(),
      files: self.files.get().0,
      scope: self.scope.get().0,
    };
    let baseline = self.baseline.get().0;
    let edits = form.has_edits(baseline.as_ref());
    (form.skill(&p.page.data, baseline.as_ref().map(|e| e.id)), edits)
  }
  fn save(&self, p: &Props, body: Skill) {
    self.operation.set(work::Operation::Save);
    self.submitted.set(true);
    self.action.run((
      p.page.project.open.clone(),
      Change::Save {
        before: self.baseline.get().0,
        name: self.name.get(),
        body,
        by_automation: self.authorship.saving(),
      },
    ));
  }
}
