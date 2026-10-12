//! Source-backed prompt control (kov6Z). Its keyed host retains this controller across renders.
use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  components::{InputEditError, InputTokens, TextInput, TextInputController},
  core::Signal,
  layout::text_style::TextStyle,
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditFeedback {
  Clear,
  Prevented,
  Failed(Arc<InputEditError>),
}
impl DevtoolsInspectable for EditFeedback {}

#[derive(Clone)]
pub struct PromptInput {
  source: Signal<String>,
  pub controller: TextInputController,
  return_focus: Signal<bool>,
  pub feedback: Signal<EditFeedback>,
}
impl PromptInput {
  pub fn new(ctx: &mut Ctx, source: Signal<String>) -> Self {
    let feedback = ctx.signal(EditFeedback::Clear);
    let changed = feedback.clone();
    ctx.watch(&source, move |_| changed.set(EditFeedback::Clear));
    Self {
      source,
      controller: TextInputController::new(),
      return_focus: ctx.signal(false),
      feedback,
    }
  }
  pub fn field(&self, ctx: &Ctx, style: TextStyle, tokens: InputTokens) -> TextInput {
    let field = TextInput::styled(self.source.clone(), style)
      .controller(self.controller.clone())
      .tokens(tokens)
      .multiline();
    if self.return_focus.get() {
      self.return_focus.set(false);
      self.controller.focus(ctx);
    }
    field
  }
  /// The caller handles errors and prevented edits; accepted insertion is one normal undo step.
  pub fn insert(&self, text: &str) -> Result<bool, InputEditError> {
    let accepted = self.controller.replace_selection(text)?;
    if accepted {
      self.return_focus.set(true);
    }
    Ok(accepted)
  }
  /// Surface the exact typed failure at the control's UI boundary.
  pub fn report(&self, result: Result<bool, InputEditError>) {
    self.feedback.set(match result {
      Ok(true) => EditFeedback::Clear,
      Ok(false) => EditFeedback::Prevented,
      Err(error) => EditFeedback::Failed(Arc::new(error)),
    });
  }
}
