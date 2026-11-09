//! A capture of an open seed request: its name, `←` and where in the answer it is read, then its ×. Its fields type
//! into the page's requests at its place; leaving one saves, and a capture whose name and place are both cleared is
//! left out of the request.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agent_testing::editor::form::CaptureLine;

use super::super::{Commit, sections::Seeds};
use crate::{
  theme::ProjectSettingsText,
  ui::{field::FieldInput, icon::Icon, icon_button::IconButton},
};

/// A capture's name (`team_id` in `oeECn`).
const NAME_WIDTH: f32 = 150.0;
/// What stands between a capture's name and where it is read (`oeECn`): a sign, not words.
pub(super) const READ_FROM: &str = "←";

#[derive(Clone)]
pub(super) struct CaptureProps {
  /// The request it is of, and its place among the request's captures, both counted from 0.
  pub request: usize,
  pub at: usize,
  pub line: CaptureLine,
  pub seeds: Signal<Seeds>,
  pub commit: Commit,
}

/// Compared by what the row shows; the signal and the save are the page's.
impl PartialEq for CaptureProps {
  fn eq(&self, other: &Self) -> bool {
    self.request == other.request && self.at == other.at && self.line == other.line
  }
}

impl DevtoolsInspectable for CaptureProps {}

pub(super) struct CaptureRow {
  name: Signal<String>,
  path: Signal<String>,
}

impl Component for CaptureRow {
  type Props = CaptureProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<CaptureProps>().clone();
    let row = Self {
      name: ctx.signal(props.line.name.clone()),
      path: ctx.signal(props.line.path.clone()),
    };
    for (part, path) in [(&row.name, false), (&row.path, true)] {
      let (seeds, request, at) = (props.seeds.clone(), props.request, props.at);
      ctx.watch(part, move |typed: &String| {
        seeds.update(|seeds| {
          let capture = seeds
            .requests
            .get_mut(request)
            .and_then(|line| line.captures.get_mut(at));
          if let Some(capture) = capture {
            let slot = if path { &mut capture.path } else { &mut capture.name };
            slot.clone_from(typed);
          }
        });
      });
    }
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let CaptureProps {
      request,
      at,
      line,
      seeds,
      commit,
    } = ctx.props::<CaptureProps>().clone();
    // A fill or a move puts what is stored at its place in the fields.
    for (field, value) in [(&self.name, line.name), (&self.path, line.path)] {
      if field.get_untracked() != value {
        field.set(value);
      }
    }
    let id = format!("settings-service-seed-request-{request}-capture-{at}");
    let leave = || {
      let commit = commit.clone();
      move || commit()
    };
    let name = FieldInput::new(
      format!("{id}-name"),
      ctx.t("project_settings.services.editor.seed.captureName"),
      self.name.clone(),
    )
    .mono()
    .width(Dimension::Px(NAME_WIDTH))
    .placeholder(ctx.t("project_settings.services.editor.seed.captureNameExample"))
    .on_leave(leave())
    .render(ctx);
    let path = FieldInput::new(
      format!("{id}-path"),
      ctx.t("project_settings.services.editor.seed.capturePath"),
      self.path.clone(),
    )
    .mono()
    .placeholder(ctx.t("project_settings.services.editor.seed.capturePathExample"))
    .on_leave(leave())
    .render(ctx);
    let remove = IconButton::new(
      format!("{id}-remove"),
      Icon::X,
      ctx.t("project_settings.services.editor.seed.removeCapture"),
    )
    .small()
    .in_form()
    .on_click(move || {
      seeds.update(|seeds| {
        if let Some(line) = seeds.requests.get_mut(request).filter(|line| at < line.captures.len()) {
          line.captures.remove(at);
        }
      });
      commit();
    })
    .render(ctx);
    Row::new()
      .id(id)
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(name)
      .child(Text::new(READ_FROM).variant(ProjectSettingsText::Hint.role()))
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(path))
      .child(remove)
  }
}
