//! Skill editor gJurA and the save/publish states bQqDC; no modal.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{InstructionScope, Skill, SkillFile};
use orchester_workspace::{
  messages::Message,
  skills::{data::SkillEntry, editor::publish_reason},
};

use super::{
  PageProps, inspector,
  state::Panel,
  words,
  work::{self, Change, Saved, Work},
};
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{WidenConfirm, WidenProps},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field},
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
      dirty,
      leave,
      baseline: ctx.signal(Baseline(p.entry.clone())),
      action: work::action(ctx),
      operation: ctx.signal(work::Operation::Save),
      asking: ctx.signal(false),
      submitted: ctx.signal(false),
    };
    let baseline = editor.baseline.clone();
    let revision = p.page.view.revision.clone();
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
    if self.asking.get() && (dirty || parsed.is_err()) {
      self.asking.set(false);
    }
    if baseline.is_some()
      && dirty
      && !state.is_pending()
      && state.error.is_none()
      && !self.submitted.get()
      && !self.asking.get()
    {
      self.save(&p, parsed.as_ref().expect("dirty means parsing passed").clone());
    }
    let busy = self.submitted.get() || state.is_pending();
    let dirty_or_invalid = edits || busy;
    if self.dirty.get_untracked() != dirty_or_invalid {
      self.dirty.set(dirty_or_invalid);
    }
    let title = baseline
      .as_ref()
      .map_or_else(|| ctx.t("skills.new"), |e| e.name.clone());
    let header = self.header(ctx, &p, baseline.as_ref(), &title, (dirty || parsed.is_err(), busy));
    let form = self.form(ctx, &p, parsed.as_ref().err());
    let details = inspector::panel(ctx, &p.page, baseline.as_ref(), self.files.clone(), busy);
    let leave = self.leave_question(ctx, busy);
    let mut frame = Column::new()
      .id("skill-editor")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header)
      .with_children(leave);
    frame = frame.with_children(self.failure(ctx, &p));
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
            .child(self.footer(ctx, baseline.as_ref(), busy)),
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
  fn header(
    &self,
    ctx: &mut Ctx,
    p: &Props,
    entry: Option<&SkillEntry>,
    title: &str,
    (dirty, busy): (bool, bool),
  ) -> Row {
    let back = p.page.view.panel.clone();
    let crumbs = breadcrumb(
      ctx,
      "skill-crumb",
      CrumbSeparator::Chevron,
      vec![
        Crumb::link(ctx.t("skills.title"), move || back.set(Panel::Library)),
        Crumb::here(title).mono(),
      ],
    );
    let action = if let Some(entry) = entry {
      let asking = self.asking.clone();
      let button = ActionButton::new(
        "skill-publish",
        ButtonKind::Primary,
        ctx.t_args("skills.publish", [("version", words::next(entry.state).to_string())]),
      );
      if busy || dirty {
        button.unavailable(ctx.t("skills.publishWaiting"))
      } else if let Some(reason) = publish_reason(entry) {
        button.unavailable(message_text(ctx, &reason))
      } else {
        button.on_click(move || asking.set(true))
      }
    } else {
      let body = self.form_state(p).0;
      let editor = self.action.clone();
      let open = p.page.project.open.clone();
      let name = self.name.get();
      let submitted = self.submitted.clone();
      let button = ActionButton::new("skill-create", ButtonKind::Primary, ctx.t("skills.create"));
      if body.is_err() || busy {
        button.disabled()
      } else {
        button.on_click(move || {
          if let Ok(body) = &body {
            if submitted.get_untracked() {
              return;
            }
            submitted.set(true);
            editor.run((
              open.clone(),
              Change::Save {
                before: None,
                name: name.clone(),
                body: body.clone(),
              },
            ));
          }
        })
      }
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .align_items(lurq::layout::Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(crumbs)
      .child(
        Text::new(&if busy {
          ctx.t("skills.saving")
        } else if dirty {
          ctx.t("skills.notSaved")
        } else {
          entry.map_or_else(|| ctx.t("skills.newNote"), |e| words::version(ctx, e.state))
        })
        .id("skill-save-status")
        .variant(TypographyStyle::Description),
      )
      .child(Spacer::new().flex(1.0))
      .child(action)
  }
  fn form(&self, ctx: &mut Ctx, p: &Props, error: Option<&Message>) -> Element {
    let locked = self.baseline.get().0.is_none() && self.submitted.get();
    let name_label = ctx.t("skills.name");
    let mut name = FieldInput::new("skill-name", name_label.clone(), self.name.clone()).mono();
    if let Some(error) = error {
      name = name.error(message_text(ctx, error));
    }
    if locked {
      name = name.locked();
    }
    let name = name.render(ctx);
    let scope_label = ctx.t("skills.scope");
    let scope = ctx.mount_keyed::<super::scope::ScopePicker>(
      "skill-scope-picker",
      super::scope::Props {
        page: p.page.clone(),
        value: self.scope.clone(),
        locked,
      },
    );
    let description_label = ctx.t("skills.descriptionLabel");
    let description = FieldInput::new("skill-description", description_label.clone(), self.description.clone())
      .text_area()
      .area_height(DESCRIPTION_HEIGHT)
      .placeholder(ctx.t("skills.descriptionPlaceholder"));
    let description = if locked { description.locked() } else { description };
    let description = description.render(ctx);
    let body = crate::ui::code_editor::CodeEditor::new("skill-body", ctx.t("skills.body"), self.body.clone())
      .locked(locked)
      .render(ctx);
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .padding(
        Padding::new()
          .left(crate::theme::skills::form_inset())
          .right(crate::theme::skills::form_inset())
          .top(ExtraSpacing::Tabs.role())
          .bottom(SpacingSize::Md),
      )
      .spacing(SpacingSize::Md)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .child(
            field(
              "skill-name-field",
              Some(&name_label),
              name,
              Some(&ctx.t("skills.nameHint")),
            )
            .flex(1.0),
          )
          .child(field("skill-scope-field", Some(&scope_label), scope, None).width(Dimension::Px(SCOPE_WIDTH))),
      )
      .child(field(
        "skill-description-field",
        Some(&description_label),
        description,
        Some(&ctx.t("skills.descriptionHint")),
      ))
      .child(
        Column::new()
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Snug.role())
          .child(
            Row::new()
              .width(Dimension::Pct(100.0))
              .child(Text::new(&ctx.t("skills.body")).variant(TypographyStyle::FieldLabel))
              .child(Spacer::new().flex(1.0))
              .child(Text::new(&ctx.t("skills.bodyFormat")).variant(TypographyStyle::Caption)),
          )
          .child(body),
      )
      .into()
  }
  fn question(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>) -> Element {
    let keep = self.asking.clone();
    let confirm = self.asking.clone();
    let action = self.action.clone();
    let open = p.page.project.open.clone();
    let entry = entry.cloned();
    let submitted = self.submitted.clone();
    let operation = self.operation.clone();
    ctx.mount_keyed::<WidenConfirm>(
      "skill-confirm",
      WidenProps {
        id: "skill-confirm",
        card: false,
        automation: Guarded::WideningQuestion,
        title: ctx.t("skills.publishQuestion"),
        reasons: Arc::from([ctx.t("skills.publishReason")]),
        confirm: ctx.t_args(
          "skills.publish",
          [(
            "version",
            entry.as_ref().map_or(1, |e| words::next(e.state)).to_string(),
          )],
        ),
        keep: ctx.t("skills.keepDraft"),
        on_keep: Arc::new(move || keep.set(false)),
        on_confirm: Arc::new(move || {
          if let Some(entry) = &entry {
            operation.set(work::Operation::Publish);
            submitted.set(true);
            confirm.set(false);
            action.run((open.clone(), Change::Publish(entry.clone())));
          }
        }),
      },
    )
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
    let parsed = self.form_state(p).0;
    let action = self.action.clone();
    let before = baseline;
    let name = self.name.get();
    let body = parsed;
    let open = p.page.project.open.clone();
    let submitted = self.submitted.clone();
    let operation = self.operation.clone();
    let asking = self.asking.clone();
    let retry = ActionButton::new("skill-retry", ButtonKind::Secondary, ctx.t("skills.retry")).on_click(move || {
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
    let reset = p.page.view.reset.clone();
    let reload = ActionButton::new("skill-reload", ButtonKind::Secondary, ctx.t("skills.reload"))
      .on_click(move || reset.update(|r| *r += 1));
    Some(
      Notice::new("skill-save-error", NoticeTone::Danger, ctx.t("skills.notSaved"))
        .body(message_text(ctx, error))
        .action(retry)
        .action(reload)
        .into(),
    )
  }
}
impl Editor {
  fn footer(&self, ctx: &Ctx, entry: Option<&SkillEntry>, busy: bool) -> Row {
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
        Text::new(&if busy {
          ctx.t("skills.saving")
        } else {
          entry.map_or_else(|| ctx.t("skills.newNote"), |e| words::saved_at(ctx, e))
        })
        .variant(ExtraTypography::Note.role()),
      )
  }
}
