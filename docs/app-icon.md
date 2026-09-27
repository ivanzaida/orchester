# Application icon

The icon is the Orchester mark (the three conductor bars of *Brand / Orchester*, `DIMZ6`) on
the `canvas` tile. It is drawn in `data/orchester.pen`, section *Brand · App icon* of
*00 / Foundations*:

| Node | Use |
| --- | --- |
| *App icon* (`Dzics`, 256) | Master: Windows 64, 256 and 512 px, the runtime window icon |
| *App icon / 48, 32, 24, 16* (`U5I3Pf`, `B7Pgr`, `E0QZ3`, `lyAVK`) | Windows small sizes, fitted to the pixel grid so the bars stay sharp |
| *App icon / macOS* (`PmIu1`, 256) | macOS: the tile on Apple's 824-on-1024 grid, with its shadow |

## Files

`crates/desktop/assets/icon/`:

- `windows/orchester-<size>.png` and `orchester.iconset/icon_<n>x<n>[@2x].png`: the PNGs exported
  from Pencil (`export_nodes`; node and scale per file are listed in `tools/app-icon.ps1`).
- `orchester.ico` (16, 24, 32, 48, 64, 256) and `orchester.icns` (16 to 1024, with @2x): packed from
  those PNGs by `powershell -File tools/app-icon.ps1`, offline and byte-for-byte reproducible.
  `tests/app_icon.rs` fails when the packed files no longer match the PNGs.
- `orchester.rc`: the Windows resource script that names the icon.

To change the icon, edit the Pencil nodes, export them over the PNGs, run the script and commit
all of it.

## Where it is shown

- **Windows executable** (Explorer, a taskbar pin, the Start menu shortcut of
  `tools/install.ps1 -StartMenuShortcut`): `crates/desktop/build.rs` compiles `orchester.rc` with
  the Windows SDK's resource compiler (through `embed-resource`) and links it into
  `orchester.exe`. The shortcut takes the program's first icon, so it needs no `IconLocation`.
  `orchester-runner.exe` has no window and no icon.
- **Running window, Windows**: `main.rs` passes the 256 px PNG to lurq's `WinitWindow::with_icon`.
  lurq sets it as the window's small icon (`WM_SETICON`, `ICON_SMALL`). Orchester draws its own
  title bar, so it appears nowhere in the window itself. lurq cannot set the big icon
  (`ICON_BIG`), which the taskbar button and Alt+Tab prefer; without it Windows falls back to the
  small icon or the executable's icon, both of which are this icon.
- **macOS**: a window icon has no effect there; the Dock and Finder take the icon of the `.app`
  bundle, which does not exist yet (see `docs/stable-build.md`, *macOS*). The build script does
  nothing on macOS. When the bundle is added, copy `orchester.icns` to
  `Orchester.app/Contents/Resources/orchester.icns` and set `CFBundleIconFile` to `orchester` in
  `Contents/Info.plist`. On a Mac, `iconutil -c icns orchester.iconset` builds an equivalent
  `.icns` from the same PNGs.
