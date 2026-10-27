//! What a skill's page asks and tells in place: publishing (*Publish v4?*, which only the person answers), leaving
//! with unsaved changes, Discard draft in place of the draft bar, Remove under the head, a write that failed with its
//! retry, and reloading the saved draft.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::{ResourceKind, Skill};
use orchester_workspace::{skills::data::SkillEntry, tasks::outside::Outside};

use super::{
  super::{
    words,
    work::{self, Change},
  },
  Ask, Editor, Props,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::hub::{
    discard,
    question::{Question, Weight},
    removal,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

impl Editor {
  pub(super) fn question(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>) -> Element {
    let (keep, confirm) = (self.asking.clone(), self.asking.clone());
    let action = self.action.clone();
    let open = p.page.project.open.clone();
    let entry = entry.cloned();
    let submitted = self.submitted.clone();
    let operation = self.operation.clone();
    let gate = ctx.use_context::<InputGate>();
    let version = entry.as_ref().map_or(1, |e| words::next(e.state)).to_string();
    Question::new(
      "skill-confirm",
      Weight::Neutral,
      (ctx.t("skills.publishQuestion"), ctx.t("skills.publishReason")),
      (
        (Icon::Upload, ctx.t_args("skills.publish", [("version", version)])),
        ctx.t("skills.keepDraft"),
      ),
    )
    .on_confirm(false, move || {
      // A published skill reaches agents' prompts (ORC-30): an MCP client publishes it only under the person's
      // consent, as for an instruction.
      if !input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        tracing::warn!("synthetic input confirmed publishing a skill; not taken");
        return;
      }
      if let Some(entry) = &entry {
        operation.set(work::Operation::Publish);
        submitted.set(true);
        confirm.set(false);
        action.run((open.clone(), Change::Publish(entry.clone())));
      }
    })
    .on_keep(move || keep.set(false))
    .into()
  }

  /// *Leave with unsaved changes?*, the wording instructions and skills share: Keep editing is its action.
  pub(super) fn leave_question(&self, ctx: &mut Ctx, busy: bool) -> Option<Element> {
    if !self.leave.requested() {
      return None;
    }
    let (keep, leave) = (self.leave.clone(), self.leave.clone());
    let nav = crate::navigation::Navigation::of(ctx);
    Some(
      Question::new(
        "skill-leave-question",
        Weight::Neutral,
        (ctx.t("hub.leave.title"), ctx.t("hub.leave.sentence")),
        ((Icon::Pencil, ctx.t("hub.leave.keep")), ctx.t("hub.leave.leave")),
      )
      // Keep editing is the question's action; Leave editor is the way out.
      .button_ids("skill-leave-keep", "skill-leave-confirm")
      .on_confirm(false, move || keep.keep_editing())
      .on_keep(move || {
        if !busy {
          leave.leave(&nav);
        }
      })
      .into(),
    )
  }

  /// Discard draft, asked in place of the draft bar; the page then shows the published version again.
  pub(super) fn discard_question(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>) -> Option<Element> {
    let entry = entry.filter(|_| self.ask.get() == Ask::Discard)?;
    let published = entry.state.published().map_or(0, |v| v.get());
    let (root, id, expected) = (p.page.project.open.repo_root.clone(), entry.id, entry.body.clone());
    let (keep, done, reset) = (self.ask.clone(), self.ask.clone(), p.page.view.reset.clone());
    Some(discard::question(
      ctx,
      discard::DiscardProps {
        project: p.page.project.clone(),
        versions: (words::next(entry.state), published),
        discard: Arc::new(move || orchester_workspace::removal::discard_in::<Skill>(&root, id, &expected)),
        change: Outside::Skills,
        on_keep: Arc::new(move || keep.set(Ask::Nothing)),
        on_discarded: Arc::new(move || {
          done.set(Ask::Nothing);
          // The page reads the skill again and starts over from what is saved.
          reset.update(|r| *r += 1);
        }),
      },
    ))
  }

  /// Remove, asked under the head; the list opens once it is removed.
  pub(super) fn remove_question(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>) -> Option<Element> {
    let entry = entry.filter(|_| self.ask.get() == Ask::Remove)?;
    let (keep, dirty, panel) = (self.ask.clone(), self.dirty.clone(), p.page.view.panel.clone());
    Some(removal::question(
      ctx,
      removal::RemoveProps {
        project: p.page.project.clone(),
        resource: entry.id,
        kind: ResourceKind::Skill,
        name: entry.name.clone(),
        on_keep: Arc::new(move || keep.set(Ask::Nothing)),
        on_removed: Arc::new(move || {
          dirty.set(false);
          panel.close(panel.library());
        }),
      },
    ))
  }

  pub(super) fn failure(&self, ctx: &Ctx, p: &Props) -> Option<Element> {
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
    let body = self.form_state(p).0;
    let (action, before, name) = (self.action.clone(), baseline, self.name.get());
    let open = p.page.project.open.clone();
    let (submitted, operation, asking) = (self.submitted.clone(), self.operation.clone(), self.asking.clone());
    let (choosing, authorship) = (self.choosing_file.clone(), self.authorship.clone());
    let new = before.is_none();
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
            by_automation: authorship.retrying(),
          },
        ));
      }
    });
    let request = p.page.view.reload_requested.clone();
    let reload = ActionButton::new("skill-reload", ButtonKind::Secondary, ctx.t("skills.reload"))
      .icon(Icon::RotateCcw)
      .on_click(move || request.set(true));
    let held = p.page.reloading || self.choosing_file.get();
    let reload = if held { reload.disabled() } else { reload };
    let retry = if p.page.reloading || (new && self.choosing_file.get()) {
      retry.disabled()
    } else {
      retry
    };
    Some(
      // What was typed stays: the `danger` icon only, no red fill (`SsC16`).
      Notice::new("skill-save-error", NoticeTone::Info, title)
        .icon(Icon::CircleAlert)
        .mark(lurq::app::theme::PaletteColor::Danger)
        .body(message_text(ctx, error))
        .action(retry)
        .action(reload)
        .into(),
    )
  }

  pub(super) fn reload_question(&self, ctx: &Ctx, p: &Props, busy: bool) -> Option<Element> {
    if !p.page.view.reload_requested.get() {
      return None;
    }
    let keep = p.page.view.reload_requested.clone();
    let (confirm, reset) = (keep.clone(), p.page.view.reset.clone());
    let held = busy || self.choosing_file.get();
    Some(
      Question::new(
        "skill-reload-question",
        Weight::Loses,
        (ctx.t("skills.reloadQuestion"), ctx.t("skills.reloadDetail")),
        ((Icon::RotateCcw, ctx.t("skills.reloadSaved")), ctx.t("skills.keepEditing")),
      )
      .button_ids("skill-reload-confirm", "skill-reload-keep")
      .on_confirm(held, move || {
        confirm.set(false);
        reset.update(|r| *r += 1);
      })
      .on_keep(move || keep.set(false))
      .into(),
    )
  }
}
