//! The chosen step's column beside the flow (`Step` `AL0M6` in `IsU5u`): its number and name, which is edited in
//! place, *Duplicate step* and *Remove step*, asked in place of the actions (`03b` `g6Ecy`); then its Step, Flow
//! and Gate and access groups. Its Permissions, Sources and Instructions open in the column under `← 03 Implement`
//! (`T46Jq`, `z4g9jL`, `PexNQ`). A viewed version's step reads, never edits (`AaaH0`).
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Spacer, Text},
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  parts,
  state::{StepPage, View},
  step_fields::{Fields, Props},
};
use crate::{
  theme::PipelinesText,
  ui::{
    button::{ActionButton, ButtonKind},
    field::FieldInput,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// The name field's width (`Step name` `C78A4`).
const NAME_WIDTH: f32 = 260.0;

#[derive(Clone, PartialEq)]
pub struct ColumnProps {
  pub step: Props,
  pub page: Option<StepPage>,
}
impl DevtoolsInspectable for ColumnProps {}

pub struct StepColumn {
  name: Signal<String>,
  removing: Signal<bool>,
  /// Whether the step is locked, as last shown.
  locked: Ref<bool>,
}
impl Component for StepColumn {
  type Props = ColumnProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ColumnProps>().step.clone();
    let editing = props.state.get();
    let name = editing
      .inputs
      .get(&format!("{}:step-name", props.step.key))
      .filter(|_| editing.viewing.is_none())
      .cloned()
      .unwrap_or_else(|| props.step.name.as_str().to_owned());
    let column = Self {
      name: ctx.signal(name),
      removing: ctx.signal(false),
      locked: ctx.create_ref(props.locked || props.viewing),
    };
    column.follow_name(ctx, &props);
    column
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ColumnProps { step: props, page } = ctx.props::<ColumnProps>().clone();
    self.locked.set(props.locked || props.viewing);
    let number = number(&props);
    let column = Column::new()
      .id("pipeline-step-column")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg);
    let Some(page) = page else {
      let heading = self.heading(ctx, &props, &number);
      let fields = ctx.mount_keyed::<Fields>("step-fields", props.clone());
      let access = super::step_access::group(ctx, &props);
      return column
        .child(heading)
        .child(fields)
        .child(access)
        .with_children(executor_error(ctx, &props));
    };
    let back_words = format!("{number} {}", props.step.name);
    // The Sources page draws its own head: its counts wait for the project's sources.
    if page == StepPage::Sources {
      return column.child(super::sources::step_section(ctx, &props, back_words));
    }
    let views = props.state.get().views;
    let key = props.step.key.clone();
    let back = parts::back_link(
      "step-page-back",
      &back_words,
      Press::run(move || views.choose(View::Step(key.clone()))),
    );
    let (title, meta) = page_words(ctx, &props, page);
    let column = column.child(parts::sub_head("step-page-title", back, &title, meta.as_deref()));
    match page {
      StepPage::Permissions => column.with_children(super::step_permissions::page(ctx, &props)),
      StepPage::Sources | StepPage::Instructions => {
        column.child(ctx.mount_keyed::<super::step_instructions::Instructions>(
          &format!("step-instructions-{}", props.step.key),
          super::step_instructions::InstructionsProps(props.clone()),
        ))
      }
    }
  }
}

impl StepColumn {
  /// Takes what is typed into the step's name into the draft, once per column, unless it is locked now. The step's
  /// key is part of the column's mount key, so it holds for the column's life.
  fn follow_name(&self, ctx: &mut Ctx, props: &Props) {
    let (state, key, locked) = (props.state.clone(), props.step.key.clone(), self.locked.clone());
    ctx.watch(&self.name, move |text| {
      if !locked.get() {
        state.update(|s| {
          let id = format!("{key}:step-name");
          s.inputs.insert(id.clone(), text.clone());
          s.field(
            &id,
            orchester_workspace::pipelines::fields::parse_name(text),
            |s, value| s.change(&key, |step| step.name = value),
          );
        });
      }
    });
  }

  /// The number, the name and the step's actions, or the question that removes it (`Step heading` `o9vgM`).
  fn heading(&self, ctx: &mut Ctx, props: &Props, number: &str) -> Element {
    let name: Element = if props.locked || props.viewing {
      Text::new(props.step.name.as_str())
        .id("step-name")
        .variant(lurq::app::theme::TypographyStyle::Title)
        .into()
    } else {
      let mut field = FieldInput::new("step-name", ctx.t("pipelines.stepName"), self.name.clone())
        .text_style(lurq::app::theme::TypographyStyle::Title)
        .width(Dimension::Px(NAME_WIDTH));
      if let Some(error) = props.state.get().errors.get(&format!("{}:step-name", props.step.key)) {
        field = field.error(message_text(ctx, error));
      }
      field.render(ctx)
    };
    let row = Row::new()
      .id("step-heading")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(number).variant(PipelinesText::StepNumber.role()))
      .child(name)
      .child(Spacer::new().flex(1.0));
    if props.viewing {
      return row.into();
    }
    if self.removing.get() {
      return Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(row)
        .child(self.remove_question(ctx, props))
        .into();
    }
    row.with_children(self.actions(ctx, props)).into()
  }

  fn actions(&self, ctx: &Ctx, props: &Props) -> [ActionButton; 2] {
    let (state, key) = (props.state.clone(), props.step.key.clone());
    let views = state.get_untracked().views;
    let duplicate = ActionButton::new("step-duplicate", ButtonKind::Ghost, ctx.t("pipelines.duplicate"))
      .icon(crate::ui::icon::Icon::Copy)
      .on_click(move || {
        let mut copy = None;
        state.update(|s| copy = s.edit.duplicate_step(&key));
        if let Some(copy) = copy {
          views.choose(View::Step(copy));
        }
      });
    let removing = self.removing.clone();
    let remove = ActionButton::new("step-remove", ButtonKind::DangerGhost, ctx.t("pipelines.remove"))
      .icon(crate::ui::icon::Icon::Trash2)
      .on_click(move || removing.set(true));
    if props.locked {
      [duplicate.disabled(), remove.disabled()]
    } else {
      [duplicate, remove]
    }
  }

  /// *Remove this step?*, asked in place (`Notice · warning` in `g6Ecy`).
  fn remove_question(&self, ctx: &Ctx, props: &Props) -> Element {
    let (state, key) = (props.state.clone(), props.step.key.clone());
    let views = state.get_untracked().views;
    let confirm =
      ActionButton::new("step-remove-confirm", ButtonKind::Danger, ctx.t("pipelines.remove")).on_click(move || {
        state.update(|s| {
          s.edit.remove_step(&key);
          let prefix = format!("{key}:");
          s.errors.retain(|id, _| !id.starts_with(&prefix));
          s.inputs.retain(|id, _| !id.starts_with(&prefix));
        });
        views.close();
      });
    let keep = self.removing.clone();
    Notice::new("step-remove-question", NoticeTone::Warning, ctx.t("pipelines.removeTitle"))
      .stacked()
      .body(ctx.t("pipelines.removeHint"))
      .action(if props.locked { confirm.disabled() } else { confirm })
      .action(
        ActionButton::new("step-keep", ButtonKind::Ghost, ctx.t("pipelines.keep")).on_click(move || keep.set(false)),
      )
      .into()
  }
}

/// The step's number among the steps, as the flow shows it (`03`).
fn number(props: &Props) -> String {
  let position = props
    .state
    .get()
    .shown()
    .steps()
    .iter()
    .position(|s| s.key == props.step.key)
    .map_or(0, |n| n + 1);
  format!("{position:02}")
}

/// A page's title and what it says of the step.
fn page_words(ctx: &Ctx, props: &Props, page: StepPage) -> (std::sync::Arc<str>, Option<String>) {
  match page {
    StepPage::Permissions => (
      ctx.t("pipelines.permissions"),
      Some(super::step_access::permissions_hint(ctx, props)),
    ),
    StepPage::Sources => (ctx.t("pipelines.sources"), None),
    StepPage::Instructions => (ctx.t("pipelines.instructions"), None),
  }
}

/// Why the step's executor cannot run it, when no notice already says so.
fn executor_error(ctx: &Ctx, props: &Props) -> Option<Element> {
  let editing = props.state.get();
  let error = orchester_workspace::pipelines::narrowing::check_step(&props.step, &editing.opened.assignables).err()?;
  if super::step_executor::draft_profile(props).is_some() || super::step_executor::unavailable_tool(props).is_some() {
    return None;
  }
  Some(crate::ui::field::field_error("step-executor-error", &message_text(ctx, &error)).into())
}
