mod app;
mod board;
mod device;
mod generator;
mod geometry;
mod keycode;
mod layout;
mod oryx;
mod stats;
mod store;
mod theme;
mod typing;
mod ui;
mod words;

const USAGE: &str = "\
tui-typer: a typing trainer for the ZSA Voyager

usage: tui-typer [--hid-debug | --help]

  --hid-debug   print raw events from the board (key presses, layer changes) and exit on Ctrl+C
";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {}
        Some("--hid-debug") => {
            device::debug_print();
            return Ok(());
        }
        Some("--help" | "-h") => {
            print!("{USAGE}");
            return Ok(());
        }
        Some(other) => {
            eprint!("unknown argument: {other}\n\n{USAGE}");
            std::process::exit(2);
        }
    }

    let mut terminal = ratatui::init();
    let result = app::App::new().run(&mut terminal);
    ratatui::restore();
    result
}
