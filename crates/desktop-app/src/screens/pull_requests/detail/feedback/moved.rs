//! A pull request that moved on while a new thread was written (`Pull request feedback · PR updated while composing`
//! `JGBXx`): where the chosen lines are at the new head, and the choices before it can be sent (`PR changed`
//! `yz6ql`): use those lines at the new head, show what changed, or choose other lines. A fix is always made on the
//! current head.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  pull_request::feedback::{LineAnchor, Located, locate},
  revision::Revision,
};
use orchester_workspace::display::short_revision;

use super::{Composing, composer::NewProps, words};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// Where the chosen lines are at the pull request's head, which moved on since they were chosen.
pub struct Moved {
  from: Revision,
  to: Revision,
  at: Located,
}

/// Where the lines `props` writes about are at the pull request's head, when it moved on since they were chosen.
pub fn moved(props: &NewProps) -> Option<Moved> {
  let Composing::Lines {
    path,
    side,
    span,
    revision,
  } = &props.composing
  else {
    return None;
  };
  let head = props.env.head.as_ref()?;
  if head == revision {
    return None;
  }
  let anchor = LineAnchor::new(path, *side, *span, props.excerpt.clone()).ok()?;
  let file = props
    .latest
    .as_ref()
    .and_then(|diff| diff.files().iter().find(|file| file.path() == &**path).cloned());
  Some(Moved {
    from: revision.clone(),
    to: head.clone(),
    at: locate(&anchor, file.as_ref()),
  })
}

/// What the composer says of it, with the choices.
pub fn changed(ctx: &Ctx, props: &NewProps, moved: &Moved) -> Element {
  let Composing::Lines { span, .. } = &props.composing else {
    return Element::new();
  };
  let (from, to) = (short_revision(moved.from.as_str()), short_revision(moved.to.as_str()));
  let lines = words::lines(ctx, (None, *span)).to_string();
  let body = match moved.at {
    Located::At(at) => ctx.t_args(
      "pull_requests.feedback.composer.changed.moved",
      [
        ("lines", lines),
        ("moved", words::lines(ctx, (None, at)).to_string()),
        ("head", to.to_owned()),
      ],
    ),
    Located::Outdated => ctx.t_args(
      "pull_requests.feedback.composer.changed.gone",
      [("lines", lines), ("head", to.to_owned())],
    ),
  };
  Column::new()
    .id("feedback-composer-changed")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::GitCompare, IconSize::Base, PaletteColor::TextSecondary))
        .child(
          Text::new(&ctx.t("pull_requests.feedback.composer.changed.title"))
            .variant(TypographyStyle::Label)
            .nowrap(),
        )
        .child(Row::new().flex(1.0))
        .child(
          Text::new(&ctx.t_args(
            "pull_requests.feedback.composer.changed.range",
            [("from", from), ("to", to)],
          ))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
        ),
    )
    .child(
      Text::new(&body)
        .id("feedback-composer-changed-body")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(choices(ctx, props, moved))
    .into()
}

/// *Use lines … at …* when the same code is there, *Show what changed* and *Select other lines*.
fn choices(ctx: &Ctx, props: &NewProps, moved: &Moved) -> Row {
  let state = props.env.state.clone();
  let to = short_revision(moved.to.as_str()).to_owned();
  let use_moved = match (moved.at, &props.composing) {
    (Located::At(at), Composing::Lines { path, side, .. }) => {
      let (state, current) = (state.clone(), props.composing.clone());
      let next = Composing::Lines {
        path: path.clone(),
        side: *side,
        span: at,
        revision: moved.to.clone(),
      };
      Some(
        ActionButton::new(
          "feedback-composer-use",
          ButtonKind::Secondary,
          ctx.t_args(
            "pull_requests.feedback.composer.changed.use",
            [("moved", words::lines(ctx, (None, at)).to_string()), ("head", to)],
          ),
        )
        .icon(Icon::Crosshair)
        .small()
        .on_click(move || {
          state.drafts.moved(&current, next.clone());
          state.follow.set(false);
          state.composing.set(Some(next.clone()));
        }),
      )
    }
    _ => None,
  };
  let show = {
    let follow = state.follow.clone();
    ActionButton::new(
      "feedback-composer-show",
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.composer.changed.show"),
    )
    .icon(Icon::GitCompare)
    .small()
    .on_click(move || follow.set(true))
  };
  let select = {
    let (composing, follow) = (state.composing.clone(), state.follow);
    ActionButton::new(
      "feedback-composer-select",
      ButtonKind::Ghost,
      ctx.t("pull_requests.feedback.composer.changed.select"),
    )
    .icon(Icon::BoxSelect)
    .small()
    .on_click(move || {
      follow.set(true);
      composing.set(None);
    })
  };
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(use_moved)
    .child(show)
    .child(select)
}
