//! The card of a target another conversation leads (ORC-778; 05 `EIjDH`, the card `f1J8z`, in `Orchestrator watch ·
//! redesign` `QOPcg`; `orchester_workspace::orchestrator::watch`): a /watch applies what is free and keeps the targets
//! other conversations lead in its line, and the conversation shows one card per such target under it — *Billing is
//! already led in «Billing and invoices»*, since when and what taking it means, *Open that conversation ↗* and *Take
//! it here*. Only a person chooses ([`card`]). Once chosen the card settles into a line in its place (`JZzui` in
//! `DE2n1`): the line the choice kept in the conversation shows there, and nowhere else.

mod card;

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ChatId,
  orchestrator::{
    Chat, ChatEntry, ChatEvent, EntryKind, WatchTarget,
    watch::{LedElsewhere, NamedTarget, WatchCommand, WatchLine},
  },
};
use orchester_workspace::{messages::Message, orchestrator::chat::ChatView};

use self::card::{LedCardView, LedProps};
use super::timeline::TurnEnv;
use crate::{theme::ExtraSpacing, ui::message::message_text};

/// What the person chose on a card: the target taken here from conversation `chat`, or left where `chat` leads it;
/// `title` is that conversation's title when the choice was kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Settled {
  pub(super) taken: bool,
  pub(super) chat: ChatId,
  pub(super) title: String,
}

/// One card: the target another conversation leads, and the person's choice once made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LedCard {
  pub(super) led: LedElsewhere,
  pub(super) settled: Option<Settled>,
}

/// The cards of a /watch's line, in conversation `chat`, with the order of the command that named their targets,
/// which *Take it here* gives the subscription it makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LedCards {
  pub(super) chat: Chat,
  pub(super) order: String,
  pub(super) cards: Vec<LedCard>,
}

/// The cards of `entry` of `shown`, a /watch's line that names targets other conversations lead, or the line of a
/// watch the person asked the Orchestrator for in words; `None` for another entry.
pub(super) fn cards(entry: &ChatEntry, shown: &ChatView) -> Option<LedCards> {
  let EntryKind::Event(ChatEvent::Watch {
    line: WatchLine::Watching {
      led_elsewhere,
      at_request,
      ..
    },
  }) = &entry.kind
  else {
    return None;
  };
  if led_elsewhere.is_empty() {
    return None;
  }
  // A watch asked in words keeps its order in its line; a command is kept just before its line, blank when it gave
  // no order, as the subscription keeps it then.
  let order = at_request.as_ref().map_or_else(
    || {
      shown
        .entries
        .iter()
        .filter(|kept| kept.seq < entry.seq)
        .filter_map(|kept| match &kept.kind {
          EntryKind::Event(ChatEvent::WatchCommand {
            command: WatchCommand::Watch { order, .. },
          }) => Some(order.clone()),
          _ => None,
        })
        .next_back()
        .unwrap_or_default()
    },
    |asked| asked.order.clone(),
  );
  let cards = led_elsewhere
    .iter()
    .map(|led| LedCard {
      led: led.clone(),
      settled: settled(shown, entry.seq, led.target.target),
    })
    .collect();
  Some(LedCards {
    chat: shown.chat.clone(),
    order,
    cards,
  })
}

/// Whether `event` is the line of a choice on a card, which shows in the card's place instead of a row of its own.
pub(super) const fn is_choice(event: &ChatEvent) -> bool {
  matches!(
    event,
    ChatEvent::Watch {
      line: WatchLine::TookOver { .. } | WatchLine::LeftThere { .. },
    }
  )
}

/// The choice kept after entry `after` of `shown` on the card of `target`; a later /watch that shows a card of the
/// same target again leaves this one unchosen.
fn settled(shown: &ChatView, after: i64, target: WatchTarget) -> Option<Settled> {
  for entry in shown.entries.iter().filter(|entry| entry.seq > after) {
    let EntryKind::Event(ChatEvent::Watch { line }) = &entry.kind else {
      continue;
    };
    match line {
      WatchLine::Watching { led_elsewhere, .. } if led_elsewhere.iter().any(|led| led.target.target == target) => {
        return None;
      }
      WatchLine::TookOver {
        target: named,
        from,
        title,
      } if named.target == target => {
        return Some(Settled {
          taken: true,
          chat: *from,
          title: title.clone(),
        });
      }
      WatchLine::LeftThere {
        target: named,
        chat,
        title,
      } if named.target == target => {
        return Some(Settled {
          taken: false,
          chat: *chat,
          title: title.clone(),
        });
      }
      _ => {}
    }
  }
  None
}

/// The cards of `led`, the row keyed `key`, one under another, in the project and conversation of `env`.
pub(super) fn column(ctx: &mut Ctx, key: &str, led: &LedCards, env: &TurnEnv) -> Column {
  let cards: Vec<Element> = led
    .cards
    .iter()
    .enumerate()
    .map(|(index, card)| {
      let id = format!("orchestrator-led-{key}-{index}");
      let props = LedProps {
        id: id.clone(),
        project: env.project,
        root: env.root.clone(),
        chat: led.chat.clone(),
        order: led.order.clone(),
        card: card.clone(),
      };
      ctx.mount_keyed::<LedCardView>(&id, props)
    })
    .collect();
  Column::new()
    .id(format!("orchestrator-led-{key}"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(cards)
}

/// `target` as people read it: its name when the line was written, or the app's words for the tasks without a module
/// and the whole project.
fn target_name(ctx: &Ctx, target: &NamedTarget) -> Arc<str> {
  if let Some(name) = &target.name {
    return Arc::from(name.as_str());
  }
  let key = match target.target {
    WatchTarget::WithoutModule => "orchestrator.watch.target.withoutModule",
    WatchTarget::Project => "orchestrator.watch.target.project",
    WatchTarget::Task(_) | WatchTarget::Module(_) => "orchestrator.watch.target.gone",
  };
  message_text(ctx, &Message::new(key))
}
