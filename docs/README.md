# docs

Assets referenced by the top-level `README.md`.

| Path | What it is |
|---|---|
| `demo.tape` | VHS script for the animated tour (`img/demo.gif`) |
| `screens.tape` | runtime panes + modals (`overview`, `stack`, `mem`, `exec`, `stats`) |
| `screens-zoom.tape` | zoomed listing and register panes |
| `screens-gnu.tape` | GNU dialect and `:transpile rvs` |
| `demo/factorial.asm` | recursive factorial in RVS dialect, used by every recording |
| `demo/factorial-gnu.asm` | the same program in standard GNU assembly |
| `img/` | rendered PNGs and the demo GIF |

## Regenerating

```bash
./docs/render.sh            # builds the release binary, then runs every tape
# or, one at a time
nix run nixpkgs#vhs -- docs/demo.tape
```

Recording uses [VHS](https://github.com/charmbracelet/vhs); the Nix package bundles
ttyd, ffmpeg and Chromium, so nothing else is needed. If `pngquant`/`gifsicle` are on
`PATH` the script also compresses the results (repo copy: ~4.5 MB total).

The tapes drive the real TUI through a PTY, so they double as a smoke test of the
keybindings they exercise. Note that `p`/`P` on the Source pane are Vim paste — the
tapes `Tab` to another pane before pressing `p` to reverse-step.

## Inspecting a single frame without recording

`cargo run --example frame -- <file.asm> [cols] [rows] [keys]` renders one frame with
ratatui's `TestBackend` and prints it as text, which is handy for checking layout
changes or a specific key sequence:

```bash
cargo run -q --example frame -- docs/demo/factorial.asm 150 44 "c,n,n,n,:,stack,<enter>"
```
