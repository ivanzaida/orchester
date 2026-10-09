use orchester_app::brand;

#[test]
fn the_window_icon_decodes_at_256_pixels() {
  let icon = brand::window_icon().expect("the bundled window icon is a valid PNG");
  assert_eq!((icon.width(), icon.height()), (256, 256));
  assert_eq!(icon.rgba().len(), 256 * 256 * 4);
}
