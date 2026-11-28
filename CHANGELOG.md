# Changelog

## v0.1.2

### Added
- **Automatic resolution** (`auto_res`, on by default): while `res_x`/`res_y` are empty, the game runs at your monitor's resolution, so a fresh install needs no setup.
- **Proper UI adjustment**: below 2160p the whole UI is laid out at 4K proportions, so the HUD, minimap, D-pad markers, pause menu, settings and help are the right size and in the right place on 16:9, 21:9, 32:9 and wider.
- **`hud_scale`** (0.8 - 2): makes the UI smaller or larger on top of that (`0.8` = 20% smaller, `2` = twice as large).
- **Native rendering** (`disable_fsr`): turns off the game's built-in upscaling (it renders at 2/3 of the resolution) for a sharper picture.
- **8K and very wide screens**: 7680x4320 (rendered natively) and 7680x1080 work. The output size, back-buffer pool, render-target heap and page pool grow when a resolution, native rendering or a scaled UI needs it.
- **Borderless fit or stretch**: `borderless_fullscreen = true` fits the picture to the monitor keeping its shape (black bars when the shapes differ), `stretch` fills the monitor.
- `CHANGELOG.md`, new README with previews for 21:9, 32:9, 64:9 and 8K.

### Removed
- **`patch_mode`**: the game is always patched in memory and its files are never touched. If an older version patched `game_scarlett_master.exe` on disk, v0.1.2 puts the original back from `game_scarlett_master.exe.bak-uw` on its first start. A leftover `patch_mode` line in your ini is ignored.

### Fixed
- **The UI is now nearly flawless below 2160p**: in v0.1.1 the HUD, minimap and menus kept their native-pixel layout and came out oversized on 1440p and 1080p screens. Gameplay HUD, D-pad view, pause menu, settings, help and the boot screen are now laid out as the game intends at 4K. The pause-menu map is the one exception (see Known issues).
- Options missing from an older ini are inserted in their proper place with their comments, not appended at the end.
- Native rendering and large UIs no longer crash in the render-target heap.

### Known issues
- Pause-menu map below 2160p: the cursor circle and icons are drawn offset and don't follow panning. A fix is in progress.
- Some title cards and the intro cutscene are stretched on ultrawide.
- At 7680x1080, a lighting seam shows near the right edge in some outdoor cutscenes.
- GTA VI v1.0.2.1 only.

## v0.1.1

### Added
- `patch_mode` option: `memory` (default) leaves the game file untouched, `exe` edits it on disk. Fixes conflicts with tools that check the game exe (e.g. foxhound).
- An existing `uw-options.ini` gets any missing options appended (with comments); your values are never changed.

### Changed
- README: clearer install step (the dll goes next to `victory.exe`).

## v0.1.0

First release: a drop-in `dxgi.dll` for VIctory with any resolution (640x360 - 7680x4320, `res_x`/`res_y`), the real aspect in 3D and cutscenes, borderless fullscreen and an FOV multiplier (`fov_scale`).
