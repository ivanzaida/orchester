use lurq::{
  app::{
    ctx::Ctx,
    events::{InputSource, input_source},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::tasks::local_integration::EditorPolicy;

use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::FieldInput,
  notice::{Notice, NoticeTone},
};

pub(super) fn input(
  ctx: &mut Ctx,
  (id, label): (&str, &str),
  value: Signal<String>,
  policy: &EditorPolicy,
  multiline: bool,
) -> Element {
  if policy.sensitive(&value.get()) {
    return Notice::new(
      format!("{id}-sensitive"),
      NoticeTone::Warning,
      ctx.t("local_integration.sensitiveDraft"),
    )
    .action(
      ActionButton::new(
        format!("{id}-clear"),
        ButtonKind::Secondary,
        ctx.t("local_integration.clearSensitiveDraft"),
      )
      .on_click(move || {
        if input_source() != InputSource::Synthetic {
          value.set(String::new());
        }
      }),
    )
    .into();
  }
  let field = FieldInput::new(id, ctx.t(label), value);
  if multiline {
    field.text_area().render(ctx)
  } else {
    field.render(ctx)
  }
}
