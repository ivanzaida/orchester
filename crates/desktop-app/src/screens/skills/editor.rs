//! Skill editor gJurA and the save/publish states bQqDC; no modal.

mod form;
mod header;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{InstructionScope, Skill, SkillFile};
use orchester_workspace::{messages::Message, skills::data::SkillEntry, tasks::outside::Outside};

use super::{
  PageProps, inspector, words,
  work::{self, Change, Saved, Work},
};
use crate::{
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};
// gJurA: sidebar a52AMN, form xF9B8 and scope cc82c.
const INSPECTOR_WIDTH: f32 = 340.0;
const SCOPE_WIDTH: f32 = 380.0;
const HEADER_HEIGHT: f32 = 52.0;
const FOOTER_HEIGHT: f32 = 44.0;
const DESCRIPTION_HEIGHT: f32 = 64.0;
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
    };
    let baseline = editor.baseline.clone();
    let (revision, skills) = (p.page.view.revision.clone(), p.page.view.skills.clone());
    let changes = crate::services::Services::of(ctx).changes;
    let submitted = editor.submitted.clone();
    let dirty_state = editor.dirty.clone();
    let navigation = p.page.view.panel.navigation.clone();
    let project = p.page.view.panel.project;
    let creating = p.entry.is_none();
    let action = editor.action.clone();
    ctx.watch(&action.state(), move |state| {
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
    if baseline.is_some()
      && dirty
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
    let header = self.header(ctx, &p, baseline.as_ref(), &title, (dirty || parsed.is_err(), busy));
    let form = self.form(ctx, &p, parsed.as_ref().err());
    let details = inspector::panel(
      ctx,
      &p.page,
      baseline.as_ref(),
      self.files.clone(),
      self.choosing_file.clone(),
      busy,
    );
    let leave = self.leave_question(ctx, busy);
    let mut frame = Column::new()
      .id("skill-editor")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header)
      .with_children(leave);
    frame = frame.with_children(self.failure(ctx, &p));
    frame = frame.with_children(self.reload_question(ctx, &p, busy));
    frame = frame.child(
      Row::new()
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .width(Dimension::Pct(100.0))
        .child(
          Column::new()
            .flex(1.0)
            .height(Dimension::Pct(100.0))
            .min_width(Dimension::Px(0.0))
            .child(form)
            .child(self.footer(ctx, baseline.as_ref(), (busy, p.page.reloading))),
        )
        .child(
          ScrollVertical::new(details)
            .width(Dimension::Px(INSPECTOR_WIDTH))
            .height(Dimension::Pct(100.0))
            .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border)),
        ),
    );
    if self.asking.get() {
      frame = frame.child(self.question(ctx, &p, baseline.as_ref()));
    }
    frame
  }
}
impl Editor {
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
      },
    ));
  }
  fn question(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>) -> Element {
    let keep = self.asking.clone();
    let confirm = self.asking.clone();
    let action = self.action.clone();
    let open = p.page.project.open.clone();
    let entry = entry.cloned();
    let submitted = self.submitted.clone();
    let operation = self.operation.clone();
    Notice::new("skill-confirm", NoticeTone::Warning, ctx.t("skills.publishQuestion"))
      .body(ctx.t("skills.publishReason"))
      .action(
        ActionButton::new("skill-confirm-keep", ButtonKind::Secondary, ctx.t("skills.keepDraft"))
          .on_click(move || keep.set(false)),
      )
      .action(
        ActionButton::new(
          "skill-confirm-confirm",
          ButtonKind::Primary,
          ctx.t_args(
            "skills.publish",
            [(
              "version",
              entry.as_ref().map_or(1, |e| words::next(e.state)).to_string(),
            )],
          ),
        )
        .on_click(move || {
          if let Some(entry) = &entry {
            operation.set(work::Operation::Publish);
            submitted.set(true);
            confirm.set(false);
            action.run((open.clone(), Change::Publish(entry.clone())));
          }
        }),
      )
      .into()
  }
}
impl Editor {
  fn leave_question(&self, ctx: &mut Ctx, busy: bool) -> Option<Element> {
    if !self.leave.requested() {
      return None;
    }
    let keep = self.leave.clone();
    let leave = self.leave.clone();
    let nav = crate::navigation::Navigation::of(ctx);
    let button = ActionButton::new("skill-leave-confirm", ButtonKind::Primary, ctx.t("skills.leave"))
      .on_click(move || leave.leave(&nav));
    Some(
      Notice::new(
        "skill-leave-question",
        NoticeTone::Warning,
        ctx.t("skills.leaveQuestion"),
      )
      .body(ctx.t("skills.leaveDetail"))
      .action(if busy { button.disabled() } else { button })
      .action(
        ActionButton::new("skill-leave-keep", ButtonKind::Secondary, ctx.t("skills.keepEditing"))
          .on_click(move || keep.keep_editing()),
      )
      .into(),
    )
  }
}
impl Editor {
  fn failure(&self, ctx: &Ctx, p: &Props) -> Option<Element> {
    let state = self.action.state().get();
    let error = state.error.as_ref().filter(|_| !state.is_pending())?;
    let baseline = self.baseline.get().0;
    let title = match (self.operation.get(), baseline.as_ref()) {
      (work::Operation::Publish, Some(entry)) => ctx.t_args(
        "skills.publishFailed",
        [
          ("name", entry.name.to_string()),
          ("version", words::next(entry.state).to_string()),
        ],
      ),
      _ => ctx.t("skills.notSaved"),
    };
    let parsed = self.form_state(p).0;
    let action = self.action.clone();
    let before = baseline;
    let name = self.name.get();
    let body = parsed;
    let open = p.page.project.open.clone();
    let submitted = self.submitted.clone();
    let operation = self.operation.clone();
    let asking = self.asking.clone();
    let choosing = self.choosing_file.clone();
    let retry = ActionButton::new("skill-retry", ButtonKind::Secondary, ctx.t("skills.retry")).on_click(move || {
      if submitted.get_untracked() || (before.is_none() && choosing.get_untracked()) {
        return;
      }
      if operation.get_untracked() == work::Operation::Publish {
        asking.set(true);
        return;
      }
      if let Ok(body) = &body {
        submitted.set(true);
        action.run((
          open.clone(),
          Change::Save {
            before: before.clone(),
            name: name.clone(),
            body: body.clone(),
          },
        ));
      }
    });
    let request = p.page.view.reload_requested.clone();
    let reload = ActionButton::new("skill-reload", ButtonKind::Secondary, ctx.t("skills.reload"))
      .on_click(move || request.set(true));
    let reload = if p.page.reloading || self.choosing_file.get() {
      reload.disabled()
    } else {
      reload
    };
    let retry = if p.page.reloading || (self.baseline.get().0.is_none() && self.choosing_file.get()) {
      retry.disabled()
    } else {
      retry
    };
    Some(
      Notice::new("skill-save-error", NoticeTone::Danger, title)
        .body(message_text(ctx, error))
        .action(retry)
        .action(reload)
        .into(),
    )
  }
}
impl Editor {
  fn footer(&self, ctx: &Ctx, entry: Option<&SkillEntry>, (busy, reloading): (bool, bool)) -> Row {
    let effect = entry.and_then(|e| e.state.published()).map_or_else(
      || ctx.t("skills.publishFirst"),
      |v| ctx.t_args("skills.publishEffect", [("version", v.get().to_string())]),
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FOOTER_HEIGHT))
      .align_items(lurq::layout::Alignment::Center)
      .padding(Padding::horizontal(crate::theme::skills::form_inset()))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Text::new(&effect).variant(TypographyStyle::Description))
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&if reloading {
          ctx.t("skills.reading")
        } else if busy && self.operation.get() == work::Operation::Publish {
          ctx.t("skills.publishing")
        } else if busy {
          ctx.t("skills.saving")
        } else {
          entry.map_or_else(|| ctx.t("skills.newNote"), |e| words::saved_at(ctx, e))
        })
        .variant(ExtraTypography::Note.role()),
      )
  }
}

impl Editor {
  fn reload_question(&self, ctx: &Ctx, p: &Props, busy: bool) -> Option<Element> {
    if !p.page.view.reload_requested.get() {
      return None;
    }
    let keep = p.page.view.reload_requested.clone();
    let confirm = keep.clone();
    let reset = p.page.view.reset.clone();
    let button =
      ActionButton::new("skill-reload-confirm", ButtonKind::Primary, ctx.t("skills.reloadSaved")).on_click(move || {
        confirm.set(false);
        reset.update(|r| *r += 1);
      });
    Some(
      Notice::new(
        "skill-reload-question",
        NoticeTone::Warning,
        ctx.t("skills.reloadQuestion"),
      )
      .body(ctx.t("skills.reloadDetail"))
      .action(if busy || self.choosing_file.get() {
        button.disabled()
      } else {
        button
      })
      .action(
        ActionButton::new("skill-reload-keep", ButtonKind::Secondary, ctx.t("skills.keepEditing"))
          .on_click(move || keep.set(false)),
      )
      .into(),
    )
  }
}
