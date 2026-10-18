# Spotify Terminal (spotify-terminal)

A fast, minimalist terminal music player and visualizer written in Rust with [Ratatui](https://github.com/ratatui-org/ratatui).

## Features

- **60-FPS Real-time Spectrum Visualizer:** Powered by `rustfft` with logarithmic frequency mapping, Hann windowing, and CAVA-style gravity decay falloff. Zero external dependencies.
- **Dynamic Mood Gradients:** 6 built-in color themes with horizontal spectrum interpolation:
  - *Cyberpunk Neon*
  - *Sunset Horizon*
  - *Matrix Emerald*
  - *Deep Ocean*
  - *Vaporwave Pastel*
  - *Minimalist Monochrome*
- **ANSI TrueColor Album Cover Art:** 24-bit TrueColor rendering via ANSI half-blocks `▀` with animated procedural synthwave canvas fallback.
- **Synced Lyrics Display:** Centered active line highlighting with glow accents and distance-based fading.
- **Minimalist Layout:** Clean split-panel interface with quick search (`/`), playlist navigation, and sleek transport controls (`⏮`, `▶ / ⏸`, `⏹`, `⏭`).

## Keyboard Shortcuts

| Key | Action |
| :--- | :--- |
| `Space` | Toggle Play / Pause |
| `m` | Cycle Mood Gradient |
| `j` / `k` or `↓` / `↑` | Navigate Playlists |
| `/` | Focus Search (`Esc` or `Enter` to return) |
| `+` / `-` | Volume Up / Down |
| `n` / `p` | Next / Previous track |
| `q` or `Esc` | Quit |

## Build & Run

```bash
cargo run --release
```
