//! One select row of a local profile's panel (`Agent CLI`, `Model`, `Reasoning effort` in `Profile` `Gk3js` of L1): a
//! `surface_input` box with a chevron, in place of the value. Choosing another option asks `on_pick` for it once; the
//! panel saves it into the draft and reads the hub again, and the select remounts on what was stored: it is keyed by
//! its value and the panel's count of settled edits, so a choice that was refused is put back too.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::SelectOption,
  core::Signal,
  node::Element,
};

use crate::ui::field::SelectField;

/// What a choice does: gets the option's value.
pub(super) type Pick = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub(super) struct Props {
  /// The select's element id.
  pub id: String,
  /// What tools call the select: its row's label.
  pub name: String,
  /// The value of the option the profile has now.
  pub value: String,
  /// Each option's value and words.
  pub options: Vec<(String, String)>,
  /// What the box says while `value` is none of the options.
  pub placeholder: Option<String>,
  /// The chosen option's words are an identifier, in Geist Mono.
  pub mono: bool,
  /// It cannot be changed now: an edit is under way, or nothing of its choices applies.
  pub locked: bool,
  pub on_pick: Pick,
}

/// The props are equal when they say the same: the handler is made anew on every render.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.id,
      &self.name,
      &self.value,
      &self.options,
      &self.placeholder,
      self.mono,
      self.locked,
    ) == (
      &other.id,
      &other.name,
      &other.value,
      &other.options,
      &other.placeholder,
      other.mono,
      other.locked,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// The select of `props`, remounted when what is stored changes or an edit settles (`settled` counts them).
pub(super) fn pick(ctx: &mut Ctx, props: Props, settled: u64) -> Element {
  let key = format!("{}-{settled}-{}", props.id, props.value);
  ctx.mount_keyed::<Select>(&key, props)
}

struct Select {
  value: Signal<String>,
}

impl Component for Select {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let value = ctx.signal(props.value.clone());
    let (seeded, on_pick) = (props.value, props.on_pick);
    ctx.watch(&value, move |picked| {
      if *picked != seeded {
        on_pick(picked.clone());
      }
    });
    Self { value }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let options = props
      .options
      .into_iter()
      .map(|(value, words)| SelectOption::new(value, words))
      .collect();
    let select = SelectField::new(props.id, props.name, self.value.clone(), options);
    let select = match props.placeholder {
      Some(words) => select.placeholder(words),
      None => select,
    };
    let select = if props.mono { select.mono() } else { select };
    if props.locked { select.locked() } else { select }.render(ctx)
  }
}
