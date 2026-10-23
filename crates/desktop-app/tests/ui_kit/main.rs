//! The UI kit headless at the design's 1440 × 900: each control's measured height, padding and gaps are
//! the design's (`data/orchester2.pen`, Shared components `ASNYj` and `DMMLd` and the dogfood boards),
//! unavailable controls show their lock and say why, and text cut to its room keeps its value in a
//! tooltip. lurq does not read a laid-out element's corner radius back, so radii are checked as the
//! theme roles the controls use, resolved to the design's values.

mod blocks;
mod buttons;
mod chat_links;
mod choices;
mod fields;
mod harness;
mod icons;
mod labels;
mod loader;
mod markdown_link_layout;
mod prompt_input;
mod textarea_focus;
mod virtualized_list;
mod virtualized_list_follow;
