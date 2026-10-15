//! Version/path/staging choices remain candidates until the distinct reviewed Save action.
use lurq::{
  app::{
    ctx::Ctx,
    events::input_source,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::integration::local::{DraftChoice, LocalPath, LocalPhase, Resolution};
use orchester_workspace::tasks::local_integration::{ConflictView, Preview};

use super::{Props, Resolver};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::{FieldInput, SelectField},
};
impl Resolver {
  pub(super) fn choice_controls(&self, ctx: &mut Ctx, props: &Props, file: &ConflictView, needs_save: bool) -> Element {
    let dirty = self.editor.text.get() != self.editor.baseline_text();
    let destination = FieldInput::new(
      "local-destination",
      ctx.t("local_integration.destination"),
      self.editor.destination.clone(),
    )
    .mono();
    let destination = if props.busy { destination.locked() } else { destination };
    let destination = if props.view.editor_policy.sensitive(&self.editor.destination.get()) {
      Element::from(Text::new(&ctx.t("local_integration.sensitiveDraft")).variant(TypographyStyle::Description))
    } else {
      destination.render(ctx)
    };
    let mut choices = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(self.candidate_button(
        ctx,
        props,
        file,
        ("local-use-current", "local_integration.useCurrent"),
        (DraftChoice::Current, dirty),
      ))
      .child(self.candidate_button(
        ctx,
        props,
        file,
        ("local-use-incoming", "local_integration.useIncoming"),
        (DraftChoice::Incoming, dirty),
      ))
      .child(self.candidate_button(
        ctx,
        props,
        file,
        ("local-delete", "local_integration.delete"),
        (DraftChoice::Delete, dirty),
      ))
      .child(destination);
    choices = choices.with_children(self.destination_controls(ctx, props, file));
    choices = choices
      .with_children(self.selected_notice(ctx))
      .with_children(self.placement_control(ctx, props));
    if needs_save {
      choices = choices.child(self.save_button(
        ctx,
        props,
        file,
        ("local-save-result", "local_integration.save"),
        (Resolution::Text(String::new()), props.busy),
      ));
    }
    choices.into()
  }

  fn selected_notice(&self, ctx: &Ctx) -> Option<Element> {
    let selected = self.editor.current().choice();
    if selected == DraftChoice::Text {
      return None;
    }
    {
      let label = ctx.t(match selected {
        DraftChoice::Current => "local_integration.useCurrent",
        DraftChoice::Incoming => "local_integration.useIncoming",
        DraftChoice::Delete => "local_integration.delete",
        DraftChoice::KeepBoth => "local_integration.keepBoth",
        DraftChoice::IncomingAtDestination => "local_integration.saveAtPath",
        DraftChoice::Text => "local_integration.result",
      });
      Some(Element::from(
        Text::new(&ctx.t_args("local_integration.candidateChoice", [("choice", label.as_ref())]))
          .id("local-candidate-choice")
          .variant(TypographyStyle::Description),
      ))
    }
  }

  fn placement_control(&self, ctx: &mut Ctx, props: &Props) -> Option<Element> {
    if !props
      .view
      .operation
      .as_ref()
      .is_some_and(|op| op.phase() == LocalPhase::RestoreConflicts)
    {
      return None;
    }
    {
      let staging = SelectField::new(
        "local-staging-choice",
        ctx.t("local_integration.placement"),
        if props.busy {
          Signal::new(self.editor.placement.get())
        } else {
          self.editor.placement.clone()
        },
        vec![
          SelectOption::new(String::from("unstaged"), ctx.t("local_integration.leaveUnstaged")),
          SelectOption::new(String::from("stage"), ctx.t("local_integration.stage")),
        ],
      );
      let staging = if props.busy { staging.locked() } else { staging };
      Some(staging.render(ctx))
    }
  }

  fn candidate_button(
    &self,
    ctx: &Ctx,
    props: &Props,
    file: &ConflictView,
    (id, label): (&str, &str),
    (choice, dirty): (DraftChoice, bool),
  ) -> ActionButton {
    let button = ActionButton::new(id, ButtonKind::Secondary, ctx.t(label));
    if props.busy || dirty {
      return button.disabled();
    }
    let editor = self.editor.clone();
    let gate = self.gate.clone();
    let work = props.work.clone();
    let refusal = super::super::work::Request {
      task: props.task.clone(),
      action: super::super::work::Action::AuthorityRefused,
      stop: props.stop.clone(),
      input: None,
      generation: props.view.operation.as_ref().map(|operation| operation.version()),
    };
    let preview = match choice {
      DraftChoice::Current => Some(file.current.clone()),
      DraftChoice::Incoming => Some(file.incoming.clone()),
      _ => None,
    };
    button.on_click(move || {
      if gate.local_input(input_source()).allowed() {
        if let Some(Preview::Text(text)) = &preview {
          editor.text.set(text.clone());
        }
        editor.select(choice);
      } else {
        work.run_if_idle(refusal.clone());
      }
    })
  }

  fn destination_controls(&self, ctx: &Ctx, props: &Props, file: &ConflictView) -> Vec<Element> {
    let dirty = self.editor.text.get() != self.editor.baseline_text();
    if props.view.editor_policy.sensitive(&self.editor.destination.get()) {
      return Vec::new();
    }
    match LocalPath::new(&self.editor.destination.get()) {
      Ok(_) => vec![
        self
          .candidate_button(
            ctx,
            props,
            file,
            ("local-keep-both", "local_integration.keepBoth"),
            (DraftChoice::KeepBoth, dirty),
          )
          .into(),
        self
          .candidate_button(
            ctx,
            props,
            file,
            ("local-save-at-path", "local_integration.saveAtPath"),
            (DraftChoice::IncomingAtDestination, dirty),
          )
          .into(),
      ],
      Err(_) => vec![
        ActionButton::new(
          "local-keep-both",
          ButtonKind::Secondary,
          ctx.t("local_integration.keepBoth"),
        )
        .disabled()
        .into(),
        ActionButton::new(
          "local-save-at-path",
          ButtonKind::Secondary,
          ctx.t("local_integration.saveAtPath"),
        )
        .disabled()
        .into(),
      ],
    }
  }
}
