//! The code diff of the selected file (`rz27k`): the diff toolbar (`SrS47`) with the compared
//! revisions and the unified mode, above the shared diff of the file (`ui::diff`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, SpacingSize},
  },
  components::{Column, Row, Spacer},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::heading::SHORT_REVISION;
use crate::{
  pull_requests::detail::ReadDiff,
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    diff::{DiffView, DiffViewProps},
    field::{ValueTone, value_box_sized},
    icon::Icon,
    status_label::status_label,
  },
};

/// Toolbar height, the compared revisions' width and height, and the Viewed button's height
/// (`SrS47`, `e63KPo`, `q2AN6`).
const TOOLBAR_HEIGHT: f32 = 54.0;
const RANGE_WIDTH: f32 = 252.0;
const RANGE_HEIGHT: f32 = 32.0;
const VIEWED_HEIGHT: f32 = 30.0;

#[derive(Clone)]
pub struct DiffPaneProps {
  pub diff: Arc<ReadDiff>,
  pub index: usize,
  /// The base branch the diff starts from, when the pull request is known.
  pub base_branch: Option<Arc<str>>,
}

impl PartialEq for DiffPaneProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.diff, &other.diff) && self.index == other.index && self.base_branch == other.base_branch
  }
}

impl lurq::app::component::DevtoolsInspectable for DiffPaneProps {}

pub struct DiffPane;

impl Component for DiffPane {
  type Props = DiffPaneProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DiffPaneProps>().clone();
    let view = ctx.mount_keyed::<DiffView>(
      "pull-request-diff-view",
      DiffViewProps {
        diff: props.diff.diff.clone(),
        key: props.diff.head.as_str().into(),
        index: props.index,
        omitted: "pullRequests.diff.omitted",
      },
    );
    Column::new()
      .id("pull-request-diff")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(toolbar(ctx, &props))
      .child(view)
  }
}

/// The compared revisions, the unified mode and Viewed (`SrS47`). Other ranges and Viewed
/// markers do not exist yet: both show as unavailable.
fn toolbar(ctx: &Ctx, props: &DiffPaneProps) -> Element {
  let revision = &props.diff.head.as_str()[..SHORT_REVISION];
  let range = match &props.base_branch {
    Some(base) => ctx.t_args("pullRequests.diff.range", [("base", &**base), ("revision", revision)]),
    None => ctx.t_args("pullRequests.diff.rangeHead", [("revision", revision)]),
  };
  let range = Column::new().width(Dimension::Px(RANGE_WIDTH)).child(value_box_sized(
    ctx,
    "pull-request-range",
    &range,
    ValueTone::Unavailable,
    None,
    Dimension::Px(RANGE_HEIGHT),
  ));
  let viewed = ActionButton::new(ctx.t("pullRequests.diff.viewed"))
    .icon(Icon::Square)
    .id("pull-request-viewed")
    .height(Dimension::Px(VIEWED_HEIGHT))
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(range)
    .child(Spacer::new().flex(1.0))
    .child(status_label(Icon::AlignLeft, &ctx.t("pullRequests.diff.unified")))
    .child(viewed)
    .into()
}
