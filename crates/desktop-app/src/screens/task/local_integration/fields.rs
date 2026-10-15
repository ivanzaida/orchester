use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
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
  line_box: f32,
) -> Element {
  if policy.sensitive(&value.get()) {
    return ctx.mount_keyed::<SensitiveField>(
      id,
      SensitiveProps {
        id: id.to_owned(),
        value,
      },
    );
  }
  FieldInput::new(id, ctx.t(label), value)
    .line_box(line_box)
    .strong_stroke()
    .render(ctx)
}

#[derive(Clone)]
struct SensitiveProps {
  id: String,
  value: Signal<String>,
}
impl PartialEq for SensitiveProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}
impl DevtoolsInspectable for SensitiveProps {}
struct SensitiveField {
  refused: Signal<bool>,
  _protected: Option<crate::mcp::input_gate::Protected>,
}
impl Component for SensitiveField {
  type Props = SensitiveProps;
  fn create(ctx: &mut Ctx) -> Self {
    let gate: Option<crate::mcp::input_gate::InputGate> = ctx.use_context();
    Self {
      refused: ctx.signal(false),
      _protected: gate.map(|gate| gate.protect(crate::mcp::input_gate::Guarded::LocalIntegrationQuestion)),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SensitiveProps>().clone();
    let id = props.id;
    let value = props.value;
    let refused = self.refused.clone();
    let notice = Notice::new(
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
        } else {
          refused.set(true);
        }
      }),
    );
    if self.refused.get() {
      notice.body(ctx.t("local_integration.error.metadata"))
    } else {
      notice
    }
  }
}
