//! Pipeline canvas (`C6Bjps` in 03.01 `Q21HOA`): the graph ([`graph`]) in an area that zooms
//! and pans, above a band along the bottom with the zoom controls (`iarCw`, [`controls`]) that
//! the graph never runs into. The shortcuts and the wheel are in [`input`]. Dragging the canvas
//! or turning the wheel pans; with the zoom modifier the wheel zooms around the pointer. Each
//! pipeline keeps its zoom for the session. While the published notice (`O8ivsy`) floats over the
//! bottom of the area, panning and fitting leave that part free, so the route notes under the
//! graph can be brought out from under it at any window size.

mod controls;
mod graph;
mod input;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::{DragEvent, KeyboardEvent, MouseButtonMask, MouseEvent, ScrollEvent},
    theme::PaletteColor,
  },
  components::{Column, Stack},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, transform::Transform2D},
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, VersionNumber},
};
use parking_lot::Mutex;

use self::{
  controls::{NoticeRefs, band},
  graph::{CanvasGraph, GraphProps},
  input::{ZoomCommand, ZoomModifier, shortcut},
};
use super::Selection;
use crate::pipelines::{
  editor::EditorState,
  zoom::{SessionZooms, Size, View, Zoom},
};

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasProps {
  pipeline: ResourceId,
  graph: GraphProps,
  just_published: Option<VersionNumber>,
}

impl lurq::app::component::DevtoolsInspectable for CanvasProps {}

impl CanvasProps {
  /// The draft being edited, with `selection` marked.
  pub fn draft(state: &EditorState, selection: &Selection) -> Self {
    Self {
      pipeline: state.id(),
      graph: GraphProps {
        steps: Arc::new(state.edit().steps().to_vec()),
        description: state.edit().description().into(),
        selected: match selection {
          Selection::Step(key) => Some(key.clone()),
          Selection::Pipeline => None,
        },
        cycle: state.edit().cycle(),
        assignables: state.assignables().clone(),
        read_only: false,
      },
      just_published: state
        .latest()
        .filter(|_| state.just_published())
        .map(|latest| latest.number),
    }
  }

  /// A published version of the pipeline, read-only.
  pub fn version(state: &EditorState, body: &Pipeline) -> Self {
    Self {
      pipeline: state.id(),
      graph: GraphProps {
        steps: Arc::new(body.steps().to_vec()),
        description: body.description().into(),
        selected: None,
        cycle: None,
        assignables: state.assignables().clone(),
        read_only: true,
      },
      just_published: None,
    }
  }
}

pub struct Canvas {
  view: Signal<View>,
  zooms: SessionZooms,
  /// The part of the canvas the graph shows in, above the zoom controls' band.
  area: ElementRef,
  modifier: ZoomModifier,
  /// Where the published notice is anchored and drawn.
  notice: NoticeRefs,
  /// The view a drag of the canvas started from.
  drag_from: Arc<Mutex<Option<View>>>,
}

impl Component for Canvas {
  type Props = CanvasProps;

  fn create(ctx: &mut Ctx) -> Self {
    let zooms = ctx
      .use_context::<SessionZooms>()
      .expect("the root provides the session's canvas zooms");
    let pipeline = ctx.props::<CanvasProps>().pipeline;
    Self {
      view: ctx.signal(View::new(zooms.get(pipeline))),
      zooms,
      area: ElementRef::new(),
      notice: NoticeRefs::default(),
      modifier: ZoomModifier::default(),
      drag_from: Arc::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CanvasProps>().clone();
    let viewport = Viewport {
      view: self.view.clone(),
      zooms: self.zooms.clone(),
      pipeline: props.pipeline,
      area: self.area.clone(),
      notice: props.just_published.map(|_| self.notice.clone()),
      content: props.graph.size(),
    };
    let view = self.view.get().within(viewport.content, viewport.size());
    let graph = ctx.mount_keyed::<CanvasGraph>("pipeline-canvas-graph", props.graph.clone());
    let zoomed = Stack::new()
      .absolute(
        0.0,
        0.0,
        Dimension::Px(viewport.content.width),
        Dimension::Px(viewport.content.height),
      )
      .transform(transform(view, viewport.content))
      .child(graph);
    let area = Stack::new()
      .id("pipeline-canvas-area")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .clip()
      .ref_element(self.area.clone())
      .child(zoomed);
    let command: Arc<dyn Fn(ZoomCommand) + Send + Sync> = {
      let viewport = viewport.clone();
      Arc::new(move |command| viewport.command(command))
    };
    let canvas = Column::new()
      .id("pipeline-canvas")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .background(PaletteColor::SurfacePanel)
      .child(self.with_pan(area, &viewport))
      .child(band(ctx, view.zoom(), &command, props.just_published, &self.notice));
    self.with_input(canvas, &viewport)
  }
}

impl Canvas {
  /// Dragging the graph's area with the primary button pans; a press that does not move still
  /// selects the card under it. The other buttons stay free, e.g. the side buttons for back and
  /// forward.
  fn with_pan(&self, area: Stack, viewport: &Viewport) -> Stack {
    let (start, stop) = (self.drag_from.clone(), self.drag_from.clone());
    let (from, pan) = (self.drag_from.clone(), viewport.clone());
    let view = self.view.clone();
    area
      .start_drag_buttons(MouseButtonMask::LEFT)
      .on_drag_start(move |_: DragEvent| *start.lock() = Some(view.get()))
      .on_drag_move(move |event: DragEvent| {
        if let Some(view) = *from.lock() {
          pan.show(view.panned(event.total_delta_x, event.total_delta_y, pan.content, pan.size()));
        }
      })
      .on_drag_end(move |_: DragEvent| *stop.lock() = None)
  }

  /// The wheel pans, or zooms with the zoom modifier; the zoom shortcuts.
  fn with_input(&self, canvas: Column, viewport: &Viewport) -> Column {
    let (keys, released, pointer, wheel) = (
      self.modifier.clone(),
      self.modifier.clone(),
      self.modifier.clone(),
      self.modifier.clone(),
    );
    let (shortcuts, scrolled) = (viewport.clone(), viewport.clone());
    canvas
      .on_key_down(move |event: KeyboardEvent| {
        keys.key_down(&event);
        if event.default_prevented() {
          return;
        }
        if let Some(command) = shortcut(&event) {
          event.prevent_default();
          shortcuts.command(command);
        }
      })
      .on_key_up(move |event: KeyboardEvent| released.key_up(&event))
      .on_mouse_move(move |event: MouseEvent| pointer.pointer(&event))
      .on_scroll(move |event: ScrollEvent| {
        event.prevent_default();
        if wheel.held() {
          scrolled.zoom_at(&event);
        } else {
          scrolled.pan(event.delta_x, event.delta_y);
        }
      })
  }
}

/// The view of the graph and the changes made to it, with the graph's size at 100 %.
#[derive(Clone)]
struct Viewport {
  view: Signal<View>,
  zooms: SessionZooms,
  pipeline: ResourceId,
  area: ElementRef,
  /// The published notice, while it is shown.
  notice: Option<NoticeRefs>,
  content: Size,
}

impl Viewport {
  /// The part of the graph's area the graph is shown in: all of it but what the published notice
  /// covers along its bottom.
  fn size(&self) -> Size {
    let covered = self.notice.as_ref().map_or(0.0, |notice| notice.covered(&self.area));
    Size {
      width: self.area.width(),
      height: self.area.height() - covered,
    }
  }

  fn show(&self, view: View) {
    self.zooms.set(self.pipeline, view.zoom());
    self.view.set(view);
  }

  fn command(&self, command: ZoomCommand) {
    let (view, size) = (self.view.get(), self.size());
    let center = (size.width / 2.0, size.height / 2.0);
    let zoom_to = |zoom: Zoom| view.zoomed(zoom, center, self.content, size);
    self.show(match command {
      ZoomCommand::In => zoom_to(view.zoom().zoomed_in()),
      ZoomCommand::Out => zoom_to(view.zoom().zoomed_out()),
      // As the canvas first shows the pipeline: 100 %, from its top left.
      ZoomCommand::Full => View::new(Zoom::FULL),
      ZoomCommand::Fit => View::fit(self.content, size),
    });
  }

  /// Zooms around the pointer.
  fn zoom_at(&self, event: &ScrollEvent) {
    let view = self.view.get();
    let anchor = (event.x - self.area.x(), event.y - self.area.y());
    let zoom = view.zoom().wheeled(event.delta_y);
    self.show(view.zoomed(zoom, anchor, self.content, self.size()));
  }

  fn pan(&self, dx: f32, dy: f32) {
    self.show(self.view.get().panned(dx, dy, self.content, self.size()));
  }
}

/// Scales the graph from its top-left corner and scrolls it. lurq transforms a node around its
/// center, so the translation moves the scaled graph's corner back to the origin first.
fn transform(view: View, content: Size) -> Transform2D {
  let scale = view.zoom().factor();
  let (x, y) = view.offset();
  Transform2D {
    tx: (scale - 1.0) * content.width / 2.0 - x,
    ty: (scale - 1.0) * content.height / 2.0 - y,
    ..Transform2D::scale_uniform(scale)
  }
}
