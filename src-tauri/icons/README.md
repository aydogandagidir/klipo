# Application Icons

This directory holds the platform-specific app icons referenced by `tauri.conf.json`.

**M1 status:** placeholder — actual icon design happens in M7 (Polish).

When the icon design lands, generate the full set with:

```bash
pnpm tauri icon path/to/icon.png
```

That produces:

- `32x32.png`, `128x128.png`, `128x128@2x.png` (Linux + Windows tray)
- `icon.icns` (macOS bundle)
- `icon.ico` (Windows bundle)
- `Square*.png` (Microsoft Store)

Until then, `pnpm tauri dev` will warn about missing icons but still launch.
The first developer to bootstrap the project should drop a 1024×1024 PNG
placeholder (e.g. solid Klipo blue with a clip glyph) and run `tauri icon`
once to populate this directory.
