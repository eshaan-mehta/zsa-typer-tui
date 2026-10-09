# zsa-typer-tui

A minimal, [monkeytype](https://monkeytype.com)-style typing test that runs in your terminal, made for the
[ZSA Voyager](https://www.zsa.io/voyager). Use it to get faster, or just to practice.

What sets it apart: it reads the exact configuration on your board, every layer included, and draws it on
screen while you type. The keys you see are always your keys, which makes it especially useful if you're new
to the Voyager or learning a new layout.

```
                           ZSA Voyager · QWERTY · Base

   a s d l f k j e i h t o r n u c g m w y p b v x q z   focus a
   progressive · 25 words · hints on

   al alsall add saff kaslak fall saff dal fall lad fall dall al lad dal
   sad fall flad as lak as saddld fal slad aslad

   ────────────────────────────────────────────────────────────────────────

               ╭───╮                                        ╭───╮
          ╭───╮│ 3 │╭───╮                              ╭───╮│ 8 │╭───╮
╭───╮╭───╮│ 2 │├───┤│ 4 │╭───╮                    ╭───╮│ 7 │├───┤│ 9 │╭───╮╭───╮
│Esc││ 1 │├───┤│ E │├───┤│ 5 │                    │ 6 │├───┤│ I │├───┤│ 0 ││ - │
├───┤├───┤│ W │├───┤│ R │├───┤                    ├───┤│ U │├───┤│ O │├───┤├───┤
│Tab││ Q │├───┤│ D │├───┤│ T │                    │ Y │├───┤│ K │├───┤│ P ││ \ │
├───┤├───┤│ S │├───┤│ F │├───┤                    ├───┤│ J │├───┤│ L │├───┤├───┤
│Sft││ A │├───┤│ C │├───┤│ G │                    │ H │├───┤│ , │├───┤│ ; ││ ' │
├───┤├───┤│ X │╰───╯│ V │├───┤                    ├───┤│ M │╰───╯│ . │├───┤├───┤
│Ctl││ Z │╰───╯     ╰───╯│ B │                    │ N │╰───╯     ╰───╯│ / ││Sft│
╰───╯╰───╯               ╰───╯                    ╰───╯               ╰───╯╰───╯
                         ╭───╮                    ╭───╮
                         │L1 │╭───╮          ╭───╮│Ent│
                         ╰───╯│Spc│          │Bsp│╰───╯
                              ╰───╯          ╰───╯

                           tab new test · esc settings
```

<sub>The typing screen in progressive mode, on a QWERTY layout. In the terminal, locked letters are dimmed,
learned ones turn green, and keys light up on the board as you press them.</sub>

## Features

### Your board, on screen

The app asks your Voyager which layout it's running and fetches the full keymap from Oryx: every layer, plus
holds, double taps and tap-then-holds. It draws the board in its real shape (staggered columns, split halves,
thumb keys) and keeps it live while you type:

- it follows the layer you're on;
- keys light up as you press them, reported by the board itself rather than guessed from what you typed;
- every keypress flashes green when it's right and red when it's wrong;
- hints outline the next key to press, plus any shift or layer key you need to hold for it (you can turn
  them off).

When you flash a new revision of your layout, the app notices, shows you which keys changed, and asks before
switching. Anything it gets wrong can be fixed by hand in the built-in layout editor.

### Progressive mode that fits your layout

The idea is the classic one: start on the home row, then add a few keys at a time. The difference is that the
details come from your layout instead of assuming QWERTY:

- you start with the letters under your resting fingers (on QWERTY `a s d f j k l`, on Colemak-DH
  `a r s t n e i o`);
- new letters unlock in order of how easy their key is to reach on the Voyager and how common they are in
  English (on QWERTY `e i h t o r n u …`, on Colemak-DH `h l d u m c …`);
- tests use only the letters you've unlocked: real words when there are enough of them, and pronounceable
  made-up ones to fill in while your set is small;
- the letter that needs the most work comes up more often.

A new letter unlocks once every current letter is learned, meaning you've beaten your speed and accuracy
targets on it by a small margin. It stays learned unless you fall well below them, so one bad test doesn't set
you back. Each new letter needs a little more practice than the one before.

### More ways to practice

- **Weak keys** mode weights the words toward the letters you're slowest or least accurate on.
- **Words** mode is a plain test of random common English words.
- **Bring your own words.** Point it at any text file and tests draw random words from it, kept exactly as
  written, so capitals and punctuation count. Progressive and weak keys modes use the words from it that fit
  the letters you're practising.
- **The usual options:** words per test, instant death (the first mistake ends the test), and a line, block or
  underscore cursor.

### Results

After each test you get wpm, raw speed, accuracy and the keys you missed, plus a graph of the test second by
second (wpm and raw, or accuracy) with mistakes marked. Step through it to read the exact numbers at any
second, or expand it to fill the screen for more detail.

```
                                69 wpm     98% acc

                             raw 69 · 25 words · 20.9s

                                missed  l ×1   s ×1

           a s d l f k j e i h t o r n u c g m w y p b v x q z   focus j

                wpm  accuracy      20.9s  wpm 69  raw 79  errors 0
150 ┤┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈│
    │                                                                            │
    │                                                                            │
100 ┤┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈⢀⣀⣠⠤⠤⠤⣄⣀⢀⣠⠴⠋⠉⠓⠲⠤⣄⣀┈│
    │⠤⠤⠤⠤⠤⢤⣀⣀⣀⡀               ⣠⠞⠉⠙⠒⠦⢤⣀⣀⣀⣠⠤⠖⠋⠓⠦⣄⣀     ⣀⣀⣀    ⢀⡴⠋      ⠈⠉⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀⣀
 50 ┤⠉⠉┈┈┈┈⠉⠉⠓⠉⠉⠉⠉⠉⠉⠉⠓⠒⠒×⠒⠒⠦⠤⠖⠒⠒⠒⠒⠒⠒⠒⠒⠚⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉×⠒⠋⠉⠉⠉⠉⠉⠉⠉⠉⠉⠉⠁┈┈┈┈┈┈┈┈┈│
    │                     ⠈⠙⠋                             ⠉⠁                     │
    │                                                                            │
  0 ┤                                                                            │
    └───────────────┬──────────────────┬──────────────────┬───────────────────────
                   5s                 10s                15s                 20.9s

                   ←→/hl inspect · ↑↓/jk wpm/accuracy · e expand
                  tab next test · r retry · esc settings · q quit
```

No Voyager plugged in? It still works as a typing test, just without the board on screen.

## Install

```sh
cargo install zsa-typer-tui
```

You'll need Rust, which you can get from [rustup.rs](https://rustup.rs). On Linux, building also needs the
libudev headers and pkg-config (`sudo apt install libudev-dev pkg-config` on Debian and Ubuntu, or your
distribution's equivalent), and reading the board needs ZSA's udev rules (see their
[Linux setup guide](https://github.com/zsa/wally/wiki/Linux-install)).

To build from source instead:

```sh
git clone https://github.com/eshaan-mehta/zsa-typer-tui
cd zsa-typer-tui
cargo install --path .
```

## Getting started

Plug in your Voyager and run:

```sh
zsa-typer-tui
```

The first time, it fetches your layout from Oryx and shows it to you. Check the keys (`tab` flips through the
layers), fix any that are wrong, and save with `ctrl+s`. From then on it only fetches again when you flash a
new revision.

Then start typing: the test begins with your first key. `tab` gives you a new test, and `esc` opens the
settings, where you can switch modes.

If your layout wasn't made in Oryx (say you compile QMK yourself), there's nothing to fetch, so the app opens
the layout editor for you to enter your keys. Without Oryx firmware the board can't report key presses or
layer changes either, so the overlay flashes keys based on what you type and stays on your first layer.

### Requirements

- A ZSA Voyager. Loading your layout automatically, and seeing your key presses and layers live, needs a
  layout made in [Oryx](https://configure.zsa.io); any other layout can be entered by hand.
- A terminal at least 80 columns wide and 31 lines tall to see the board (smaller works, the board is just
  hidden). The board and graph are drawn with box-drawing and braille characters, which most terminals show
  fine.
- macOS or Linux. It's developed on macOS; Windows is untested.
- An internet connection, only for fetching your layout.

## Controls

| Where | Keys |
| --- | --- |
| Typing | type to start · `tab` new test · `esc` settings · `ctrl+w` / `alt+backspace` delete a word · `ctrl+c` quit |
| Results | `tab` / `enter` next test · `r` retry the same text · `←→` / `h l` step through the graph (`home` / `end` jump to the ends) · `↑↓` / `j k` switch between the wpm and accuracy graphs · `e` expand the graph (`e` / `esc` to shrink) · `esc` settings · `q` quit |
| Settings | `↑↓` / `j k` move · type a setting's name to jump to it · `enter` change · `←→` adjust · `esc` close |
| Layout editor | arrows, or press a key on the board, to select · `enter` change the key · `del` undo your change · `tab` next layer · `ctrl+s` save · `esc` cancel |

In the layout editor, each of a key's four actions (tap, hold, double tap, tap-then-hold; `tab` switches
between them while editing) can be set to a character (`q`, `!`), a QMK keycode (`left_shift`, `mo 1`), or
`none`. On the board, a key's label is its tap, its hold is written in its bottom border, and a dot (`•`)
marks keys that also have a double tap or tap-then-hold. When a hint points at one of those, the bottom
border says how to press it (`hold`, `2×`, `t+hld`).

## Settings

| Setting | |
| --- | --- |
| Mode | progressive, weak keys or words |
| Hints | outline the next key to press on the board |
| Cursor | line, block or underscore (it's your terminal's own cursor, so its color comes from your terminal theme) |
| Instant death | end the test on the first mistake |
| Words | words per test (5–500) |
| Word list | path to a text file of your own words (separated by spaces or new lines); leave empty for the built-in words |
| Target speed | the speed a letter needs to count as learned in progressive mode (10–200 wpm) |
| Target accuracy | the accuracy it needs alongside that (50–100%) |
| Edit layout | fix keys by hand |
| Refetch layout from board | fetch your flashed layout from Oryx again |
| Reset progress | clear your letter stats and progressive unlocks |

## How it works

- **Talking to the board** (`src/device.rs`): the Voyager's raw HID interface speaks ZSA's Oryx protocol, the
  one Live Training uses. The app asks the firmware which layout and revision it's running, then pairs so the
  board streams physical key presses and layer changes. It opens the board without taking it over, so Keymapp
  can stay running.
- **Your keymap** (`src/oryx.rs`): the board doesn't hand over its keymap, so the app fetches it from Oryx by
  layout and revision. That API isn't officially documented and could change, so responses are validated, and
  every revision is cached and never needs fetching again. Your manual fixes are stored separately and applied
  on top.
- **The board's shape** (`src/geometry.rs`, `src/board.rs`): key positions and wiring come from QMK's Voyager
  definition.
- **Progression** (`src/stats.rs`, `src/generator.rs`): the starting letters come from your resting keys, and
  the unlock order weighs how easy each key is to reach against how common its letter is. Made-up words come
  from a letter model trained on the built-in word list. Each letter's speed and accuracy are running averages.

Settings, cached layouts and stats are kept in your config directory:
`~/Library/Application Support/zsa-typer-tui` on macOS, `~/.config/zsa-typer-tui` on Linux.

## Troubleshooting

- **"Your voyager is missing":** check the cable. On Linux, make sure ZSA's udev rules are installed.
- **The keys on screen are wrong:** in the settings, *Refetch layout from board* fetches it again, and *Edit
  layout* fixes individual keys.
- **To see what the board is sending,** run `zsa-typer-tui --hid-debug`. It prints raw events: the connection,
  key presses with their matrix positions, and layer changes.

## Development

```sh
cargo test
cargo clippy
```

`tests/fixtures/oryx_layout.json` is a real Oryx response, used by the parser and progression tests.

## License

[MIT](LICENSE)
