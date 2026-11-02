//! The two answer rows of 06b, retaining the existing wider-approval confirmation.
//! A covers error means a wider answer is not offered; `.ok()` omits that answer and its confirmation, as 06b does.
//! A hold error disables an action rather than sending it.
use lurq::app::events::KeyboardEvent;

use super::*;

pub(in super::super) fn answers(ctx: &mut Ctx, answering: &Answering<'_>, hint: &Signal<Option<Arc<str>>>) -> Element {
  let run = answering.run.to_string();
  let similar = similar_covers(ctx, answering, &run);
  let everything = everything_covers(ctx, answering, &run);
  let focus = [
    answering.once.clone(),
    ElementRef::new(),
    ElementRef::new(),
    ElementRef::new(),
    ElementRef::new(),
  ];
  let once = button(
    answering,
    Given::ApproveOnce,
    "approve",
    ctx.t("cards.approval.inline.once"),
  )
  .key("1")
  .element_ref(answering.once.clone());
  let once_hint = hint.clone();
  let once_words = ctx.t("cards.approval.inline.onceCovers");
  let allow = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(once.on_focus(move || once_hint.set(Some(once_words.clone()))))
    .with_children(wider_button(
      ctx,
      answering,
      Given::ApproveSimilar,
      similar,
      hint,
      &focus[1],
    ))
    .with_children(wider_button(
      ctx,
      answering,
      Given::ApproveEverything,
      everything,
      hint,
      &focus[2],
    ));
  actions(ctx, answering, hint, allow, focus)
}

fn actions(
  ctx: &mut Ctx,
  answering: &Answering<'_>,
  hint: &Signal<Option<Arc<str>>>,
  allow: Row,
  focus: [ElementRef; 5],
) -> Element {
  let id = answering.id;
  let label = ctx.t_args(
    "cards.approval.inline.deny",
    [("agent", answering.request.agent.as_str())],
  );
  let deny = if answering.request.native.is_some() {
    button(answering, Given::Deny, "deny", ctx.t("cards.approval.deny")).key("4")
  } else {
    message_button(answering, label).key("4")
  };
  let deny_hint = hint.clone();
  let deny = deny.element_ref(focus[3].clone()).on_focus(move || deny_hint.set(None));
  let more_hint = hint.clone();
  let (more, toggle) = (answering.more.clone(), answering.more.clone());
  let open = more.get();
  let more = ActionButton::new(
    format!("{id}-more-answers"),
    ButtonKind::Secondary,
    ctx.t("cards.approval.inline.more"),
  )
  .icon(if open { Icon::ChevronUp } else { Icon::ChevronDown })
  .key("5")
  .in_form()
  .element_ref(focus[4].clone())
  .on_focus(move || more_hint.set(None))
  .on_click(move || toggle.update(|open| *open = !*open));
  let row = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(deny)
    .child(more);
  let confirmation = confirmation(ctx, answering);
  let keys = shortcuts(ctx, answering);
  let column = Column::new()
    .id(format!("{id}-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .on_key_down(move |event: KeyboardEvent| {
      if focus.iter().any(ElementRef::focused) {
        press_digit(&keys, &event);
      }
    })
    .child(allow)
    .child(row)
    .with_children(hint.get().map(|words| note(&format!("{id}-consequence"), &words)))
    .with_children(held_line(ctx, answering))
    .with_children(confirmation);
  if !open {
    return column.into();
  }
  column
    .with_children(project_covers(ctx, answering.request).map(|covers| {
      tile(
        ctx,
        answering,
        (Given::ApproveProject, Icon::ShieldCheck, "cards.approval.project"),
        Ok(covers),
      )
    }))
    .with_children(
      answering
        .request
        .native
        .is_none()
        .then(|| button(answering, Given::Deny, "deny", ctx.t("cards.approval.deny"))),
    )
    .child(automation(ctx, id, answering.shown))
    .into()
}

fn confirmation(ctx: &mut Ctx, answering: &Answering<'_>) -> Option<Element> {
  let run = answering.run.to_string();
  let asked = match answering.asking.get() {
    Some(Given::ApproveSimilar) => similar_covers(ctx, answering, &run)
      .ok()
      .map(|covers| (Given::ApproveSimilar, covers)),
    Some(Given::ApproveEverything) => everything_covers(ctx, answering, &run)
      .ok()
      .map(|covers| (Given::ApproveEverything, covers)),
    Some(Given::ApproveProject) => project_covers(ctx, answering.request).map(|covers| (Given::ApproveProject, covers)),
    _ => None,
  };
  asked.and_then(|(given, covers)| confirm_wider(ctx, answering, given, covers))
}

fn message_button(answering: &Answering<'_>, label: Arc<str>) -> ActionButton {
  let button = ActionButton::new(format!("{}-deny-message", answering.id), ButtonKind::Secondary, label)
    .icon(Icon::PencilLine)
    .in_form();
  match action(answering, Given::DenyWithMessage) {
    Some(press) => button.on_click(move || press()),
    None => button.disabled(),
  }
}

fn button(answering: &Answering<'_>, given: Given, name: &str, label: Arc<str>) -> ActionButton {
  let kind = if given == Given::ApproveOnce {
    ButtonKind::Primary
  } else {
    ButtonKind::Secondary
  };
  let button = ActionButton::new(format!("{}-{name}", answering.id), kind, label).in_form();
  match action(answering, given) {
    Some(press) => button.on_click(move || press()),
    None if answering.sending() == Some(given) => button.loading(),
    None => button.disabled(),
  }
}

fn wider_button(
  ctx: &Ctx,
  answering: &Answering<'_>,
  given: Given,
  covers: Result<Arc<str>, Arc<str>>,
  hint: &Signal<Option<Arc<str>>>,
  focus: &ElementRef,
) -> Option<Element> {
  let covers = covers.ok()?;
  let (key, number) = if given == Given::ApproveSimilar {
    ("cards.approval.inline.similar", "2")
  } else {
    ("cards.approval.inline.everything", "3")
  };
  let hint = hint.clone();
  Some(
    button(answering, given, wider_name(given), ctx.t(key))
      .key(number)
      .element_ref(focus.clone())
      .on_focus(move || hint.set(Some(covers.clone())))
      .into(),
  )
}

fn everything_covers(ctx: &Ctx, answering: &Answering<'_>, run: &str) -> Result<Arc<str>, Arc<str>> {
  match answering.shown {
    Some(shown) if shown.auto_approving => Err(ctx.t_args("cards.approval.everythingOn", [("run", run)])),
    Some(_) => Ok(ctx.t_args("cards.approval.everythingCovers", [("run", run)])),
    None => Err(ctx.t("cards.approval.unread")),
  }
}

fn action(answering: &Answering<'_>, given: Given) -> Option<Arc<dyn Fn() + Send + Sync>> {
  let open = answering.hold(given).ok()?;
  match given {
    Given::ApproveSimilar | Given::ApproveEverything | Given::ApproveProject => {
      let asking = answering.asking.clone();
      Some(Arc::new(move || asking.set(Some(given))))
    }
    Given::DenyWithMessage if answering.request.native.is_none() => {
      let (editing, focus) = (answering.editing.clone(), answering.focus_field.clone());
      Some(Arc::new(move || {
        focus.store(true, Ordering::Relaxed);
        editing.set(true);
      }))
    }
    Given::DenyWithMessage => None,
    _ => Some(Arc::new(answering.press(given, open))),
  }
}

type Shortcut = Option<Arc<dyn Fn() + Send + Sync>>;

fn shortcuts(ctx: &Ctx, answering: &Answering<'_>) -> [Shortcut; 5] {
  if answering.asking.get().is_some() {
    return std::array::from_fn(|_| None);
  }
  let run = answering.run.to_string();
  let more = answering.more.clone();
  [
    action(answering, Given::ApproveOnce),
    similar_covers(ctx, answering, &run)
      .ok()
      .and_then(|_| action(answering, Given::ApproveSimilar)),
    everything_covers(ctx, answering, &run)
      .ok()
      .and_then(|_| action(answering, Given::ApproveEverything)),
    action(
      answering,
      if answering.request.native.is_some() {
        Given::Deny
      } else {
        Given::DenyWithMessage
      },
    ),
    Some(Arc::new(move || more.update(|open| *open = !*open))),
  ]
}

fn press_digit(keys: &[Shortcut; 5], event: &KeyboardEvent) {
  if event.text_input_focused || event.composing || event.ctrl || event.alt || event.meta || event.shift {
    return;
  }
  let index = match event.key.as_str() {
    "1" => 0,
    "2" => 1,
    "3" => 2,
    "4" => 3,
    "5" => 4,
    _ => return,
  };
  if let Some(press) = &keys[index] {
    event.prevent_default();
    press();
  }
}
