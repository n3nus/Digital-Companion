# Nøkk

Nøkk is a tiny native digital pet companion: a green Mooswald troll that quietly hangs around while you code, watch something, or use a second monitor.

V1 has two entrypoints:

- `nokk` or `nokk desktop` launches the desktop companion.
- `nokk console` launches the ANSI terminal companion.

The design intentionally avoids chores, hunger, network access, notifications, project scanning, or attention-demanding mechanics. Nøkk reacts to time, a little randomness, pokes, and mouse stroking.

## Current Target

- Linux: Hyprland/Wayland-first layer-shell backend.
- Windows: native layered-window backend.
- Console: cross-platform `crossterm` alternate-screen mode.

## Assets

Sprites live in `assets/nokk/`.

- `generated/source.png`: image-generator source spritesheet.
- `generated/walk_source.png`: image-generator directional walking source.
- `generated/walk_diagonal_source.png`: stronger 3/4 diagonal walk source.
- `spritesheet.png`: transparent 192x192 runtime frames processed from the generated source.
- `manifest.ron`: animation frame timing, hit zones, and heart spawn points.
- `preview.png`: enlarged animation rows for idle, walk, sleep, and dance.

Current animations: idle, blink, walk down/up/left/right, sit, sleep, happy, poke, dance, plus heart particles.

Regenerate the processed art after replacing `assets/nokk/generated/source.png` with:

```bash
python3 tools/generate_assets.py
```

## Build

Install a current Rust toolchain, then:

```bash
cargo test
cargo run -p nokk -- console
cargo run -p nokk -- desktop
```
