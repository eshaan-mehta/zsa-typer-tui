# zsa-typer-tui

A minimal, monkeytype-style typing trainer for the terminal, built for people learning to type on the
**ZSA Voyager**.

It reads which layout is flashed on your board, draws the Voyager in its real shape (column stagger, split
halves, thumb keys) with your keys on it, and lights up keys as you type: what you pressed, what you got
wrong, and (optionally) what to press next.

```
                  ╭─────╮│  3  │╭─────╮
    ╭─────╮╭─────╮│  2  │├─────┤│  4  │╭─────╮
    │  `  ││  1  │├─────┤│  D  │├─────┤│  5  │
    ├─────┤├─────┤│  L  │├─────┤│  C  │├─────┤
    │ Esc ││  B  │├─────┤│  T  │├─────┤│  V  │
    ├─────┤├─────┤│  R  │├─────┤│  S  │├─────┤
    │ Tab ││  N  │├─────┤│  M  │├─────┤│  G  │
    ├─────┤├─────┤│  Q  │╰─────╯│  W  │├─────┤
    │ Ctl ││  X  │╰─────╯       ╰─────╯│  Z  │
    ╰─────╯╰─L3──╯                     ├─────┤
                                       │     │╭─────╮
                                       ╰─Opt─╯│ Ent │
                                              ╰─Cmd─╯
```

## Features

- **Your layout, read from the board.** The Voyager reports which Oryx layout and revision it's running; the
  app fetches that keymap from Oryx, asks you to confirm it, and caches it. Any key can be corrected by hand.
- **Live overlay.** Keys light up as you press them (straight from the board, not guessed from characters),
  flash green or red for right and wrong, and show the active layer.
- **Hints.** Optionally outline the next key to press, plus the shift or layer key to hold for it.
- **Progressive mode.** Learn your layout a few letters at a time. You start with whatever letters sit under
  your resting fingers on *your* layout; new letters unlock (easiest-to-reach and most useful first) once every
  current letter meets your speed and accuracy targets. Early stages mix real words with pronounceable
  letter combinations when your letters can't spell enough words yet.
- **Weak keys mode.** Words weighted toward the letters you're slowest or least accurate on.
- **Words mode.** Random common English words.
- **Instant death.** Optional: the first wrong key ends the test.
- Works without the board too: if no Voyager is connected you get a plain typing test (and a note that your
  Voyager is missing).

## Requirements

- A ZSA Voyager running a layout made in [Oryx](https://configure.zsa.io) (for automatic layout loading).
  Layouts compiled elsewhere still work: enter your keys in the built-in editor.
- Rust (stable).
- A terminal at least 80 columns wide and ~30 lines tall to see the board.
- Developed and tested on macOS. On Linux, reading the board needs ZSA's udev rules
  ([setup guide](https://github.com/zsa/wally/wiki/Linux-install)).

## Run

```sh
cargo run --release
```

On first launch the app finds your Voyager, fetches its layout from Oryx and shows it for you to confirm.
After that it only fetches again when you flash a new revision.

## Controls

| Where | Keys |
| --- | --- |
| Typing | type to start · `tab` new test · `esc` settings · `ctrl+w` / `alt+backspace` delete word · `ctrl+c` quit |
| Results | `tab` / `enter` next test · `r` retry same text · `esc` settings · `q` quit |
| Settings | `↑↓` / `j k` move · type a setting's name to jump to it · `enter` change · `←→` adjust · `esc` close |
| Layout editor | arrows or press a key on the board to select · `enter` change key · `del` undo change · `tab` next layer · `ctrl+s` save · `esc` cancel |

In the layout editor each of a key's four actions (tap, hold, double tap, tap-then-hold; `tab` switches
while editing) can be set to a character (`q`, `!`) or a QMK keycode (`left_shift`, `mo 1`), or cleared
with `none`. On the board, a key's tap is its label, its hold is in the bottom border, and a dot (`•`)
marks keys that also have a double tap or tap-then-hold. When a hinted character is on one of those,
the key's bottom border says how to press it (`hold`, `2×`, `t+hld`).

### Settings

| Setting | |
| --- | --- |
| Mode | progressive / weak keys / words |
| Hints | outline the next key on the board |
| Instant death | end the test on the first mistake |
| Words | words per test (5–500) |
| Word list | path to your own word file (words separated by spaces or new lines); empty for the built-in words |
| Target speed | per-letter speed a letter needs to count as learned (10–200 wpm) |
| Target accuracy | per-letter accuracy needed alongside it (50–100%) |
| Edit layout | fix keys by hand |
| Refetch layout from board | fetch the flashed layout from Oryx again |
| Reset progress | clear letter stats and progressive unlocks |

## How it works

- **Board connection** (`src/device.rs`): the Voyager's raw HID interface speaks ZSA's Oryx protocol (the one
  Live Training uses). The app asks for the firmware's layout/revision ID, then pairs so the board streams
  physical key down/up positions and layer changes. It opens the device non-exclusively, so Keymapp can stay
  running.
- **Keymap** (`src/oryx.rs`): the board doesn't expose its keymap, so the app fetches it from Oryx's GraphQL
  backend by layout/revision ID. That API isn't officially documented and could change; responses are
  validated, and every fetched revision is cached so it never needs fetching again.
- **Geometry** (`src/geometry.rs`, `src/board.rs`): key positions and matrix wiring come from QMK's Voyager
  definition.
- **Progression** (`src/stats.rs`, `src/generator.rs`): starting letters come from your resting keys; unlock
  order weighs how easy a key is to reach on the Voyager against English letter frequency. Pseudo-words come
  from a letter model trained on the built-in word list.

Settings, cached layouts and stats are stored in your config directory (`~/Library/Application
Support/zsa-typer-tui` on macOS, `~/.config/zsa-typer-tui` on Linux).

### Debugging the board connection

```sh
cargo run -- --hid-debug
```

prints raw events from the board (connection, key down/up with matrix positions, layer changes).

## Development

```sh
cargo test
cargo clippy
```

`tests/fixtures/oryx_layout.json` is a real Oryx response used by the parser and progression tests.
