use std::{collections::HashMap, sync::Arc};

use lurq::{app::theme::ThemeShadows, node::box_shadow::BoxShadow};

use super::ExtraColor;

/// Elevations of the design: `sm` controls, `md` menus and popovers, `lg` dialogs. The color is the
/// `shadow` role, so it follows the mode.
pub fn shadows() -> ThemeShadows {
  let shadow = |offset_y, blur, spread| BoxShadow::new(0.0, offset_y, blur, ExtraColor::Shadow.role()).spread(spread);
  ThemeShadows {
    sm: Arc::from([shadow(1.0, 2.0, 0.0)]),
    md: Arc::from([shadow(4.0, 12.0, -2.0), shadow(1.0, 3.0, 0.0)]),
    lg: Arc::from([shadow(16.0, 40.0, -8.0), shadow(2.0, 6.0, -2.0)]),
    extra: HashMap::new(),
  }
}
