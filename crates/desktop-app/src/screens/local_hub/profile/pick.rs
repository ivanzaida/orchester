//! One select row of a local profile's panel (`Agent CLI`, `Model`, `Reasoning effort` in `Profile` `Gk3js` of L1): a
//! `surface_input` box with a chevron, in place of the value. Choosing another option asks `on_pick` for it once and
//! shows it at once; the panel holds it. The select stays mounted for as long as the panel shows it: once no choice is
//! being checked it shows what the panel holds, so a choice that was refused, or that *Cancel* dropped, is put back.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::SelectOption,
  core::{Ref, Signal},
  node::Element,
};

use crate::ui::field::SelectField;

/// What a choice does: gets the option's value.
pub(in crate::screens) type Pick = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub(in crate::screens) struct Props {
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
  /// It cannot be changed now: a save is under way, or nothing of its choices applies.
  pub locked: bool,
  /// A choice is being checked: the select keeps showing what the person chose until the panel holds it.
  pub choosing: bool,
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
      self.choosing,
    ) == (
      &other.id,
      &other.name,
      &other.value,
      &other.options,
      &other.placeholder,
      other.mono,
      other.locked,
      other.choosing,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// The select of `props`, mounted once for as long as the panel shows it.
pub(in crate::screens) fn pick(ctx: &mut Ctx, props: Props) -> Element {
  let key = props.id.clone();
  ctx.mount_keyed::<Select>(&key, props)
}

struct Select {
  /// What the select shows: the person's choice, then what the panel holds.
  value: Signal<String>,
  /// What the panel holds, as the select last heard it: a value other than it is the person's choice.
  held: Ref<String>,
  /// What a choice does, as the last render made it.
  on_pick: Ref<Pick>,
}

impl Component for Select {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let value = ctx.signal(props.value.clone());
    let (held, on_pick) = (ctx.create_ref(props.value), ctx.create_ref(props.on_pick));
    let (seen, pick) = (held.clone(), on_pick.clone());
    ctx.watch(&value, move |picked| {
      if *picked != seen.get() {
        (pick.get())(picked.clone());
      }
    });
    Self { value, held, on_pick }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.on_pick.set(props.on_pick.clone());
    self.held.set(props.value.clone());
    // Once no choice is checked, the select shows what the panel holds: the choice it took, or what it kept.
    if !props.choosing && self.value.get_untracked() != props.value {
      self.value.set(props.value.clone());
    }
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
