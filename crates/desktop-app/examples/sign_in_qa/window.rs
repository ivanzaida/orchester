//! The window before it shows, driven with the pointer of the example itself (never an MCP client's, which a
//! person-only control refuses), as the sign-in's QA and the QA app's team scenarios do.

use std::{
  error::Error,
  thread,
  time::{Duration, Instant},
};

use lurq::app::{App, Tree, events::MouseButton};

pub struct Window<'a> {
  pub app: &'a mut App,
  pub tree: &'a mut Tree,
}

impl Window<'_> {
  /// Lays the window out, delivering what its futures finished, until `id` shows; an error after 20 seconds.
  pub fn until(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
      self.tree.tick_futures();
      self.tree.pass_headless(self.app);
      if self.tree.get_element_by_id(id).is_some() {
        return Ok(());
      }
      if Instant::now() > deadline {
        return Err(format!("{id} did not show").into());
      }
      thread::sleep(Duration::from_millis(10));
    }
  }

  pub fn center(&mut self, id: &str) -> Result<(f32, f32), Box<dyn Error>> {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .ok_or_else(|| format!("{id} is laid out"))?;
    Ok((bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0))
  }

  /// Presses the middle of `id` with the pointer.
  pub fn press(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
    let (x, y) = self.center(id)?;
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    // The pointer leaves for the panel's empty side, so what it pressed shows as it rests.
    self.tree.mouse_move(1.0, y);
    self.tree.pass_headless(self.app);
    Ok(())
  }

  /// Types `value` into the text input `id`, in place of what it held.
  pub fn set(&mut self, id: &str, value: &str) -> Result<(), Box<dyn Error>> {
    self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .ok_or_else(|| format!("{id} is a text input"))?
      .set_value(value);
    self.tree.pass_headless(self.app);
    Ok(())
  }
}
