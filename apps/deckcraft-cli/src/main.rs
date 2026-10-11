//! Headless DeckCraft.
//!
//! ```text
//! deckcraft-cli info FILE                       # slides, titles, layouts (JSON)
//! deckcraft-cli render (FILE | --sample) [--slide N | --all] [--scale S] OUT.png|OUT_DIR
//! deckcraft-cli convert IN OUT                  # .deckcraft ⇄ .pptx, outline .txt, .png (first slide)
//! deckcraft-cli run [--in FILE | --sample] [--cmd ID[=JSON]]... [--save OUT] [--export OUT] [--print]
//! deckcraft-cli commands [FILTER]               # list commands (JSON)
//! deckcraft-cli describe ID                     # one command
//! deckcraft-cli app [--port PORT] COMMAND [JSON] | --method METHOD [JSON]   # drive the running app
//! deckcraft-cli mcp [--connect PORT] [--sample] # MCP server over stdio
//! deckcraft-cli links | --version
//! ```
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use files::{convert, info};

use connection::{app, mcp};

use batch::run;

use render::render;

use std::process::ExitCode;

use deckcraft_engine::Session;
use serde_json::{Value, json};

macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if let Err(e) = writeln!(std::io::stdout(), $($arg)*) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
            eprintln!("deckcraft-cli: can't write to stdout: {e}");
            std::process::exit(1);
        }
    }};
}

mod batch;
mod connection;
mod files;
mod render;

const USAGE: &str = "usage: deckcraft-cli info FILE
       deckcraft-cli render (FILE | --sample) [--slide N | --all] [--scale S] [--edit] OUT
       deckcraft-cli convert IN OUT
       deckcraft-cli run [--in FILE | --sample] [--cmd ID[=JSON]]... [--save OUT] [--export OUT] [--print]
       deckcraft-cli commands [FILTER]
       deckcraft-cli describe COMMAND
       deckcraft-cli app [--port PORT] COMMAND [JSON] | --method METHOD [JSON]
       deckcraft-cli mcp [--connect PORT] [--sample]
       deckcraft-cli links | --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        Some("--version" | "-V" | "version") => {
            outln!("deckcraft-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("info") => report(info(rest)),
        Some("render") => report(render(rest)),
        Some("convert") => report(convert(rest)),
        Some("run") => report(run(rest)),
        Some("commands") => {
            let s = Session::new();
            let f = rest.first().map(|x| x.to_lowercase());
            let list: Vec<Value> = s
                .commands()
                .into_iter()
                .filter(|c| f.as_ref().is_none_or(|f| c.id.to_lowercase().contains(f.as_str()) || c.label.to_lowercase().contains(f.as_str())))
                .map(|c| serde_json::to_value(c).unwrap_or_default())
                .collect();
            outln!("{}", serde_json::to_string_pretty(&list).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Some("describe") => report(describe(rest.first().map(String::as_str))),
        Some("app") => report(app(rest)),
        Some("mcp") => report(mcp(rest)),
        Some("links") => {
            use deckcraft_engine::links::*;
            outln!("Discord   {DISCORD}\nWebsite   {WEBSITE}\nApp page  {APP_PAGE}\nGitHub    {GITHUB}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            eprintln!("\nCommunity: {}  ·  {}", deckcraft_engine::links::DISCORD, deckcraft_engine::links::APP_PAGE);
            ExitCode::FAILURE
        }
    }
}

fn report(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("deckcraft-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

fn open(s: &mut Session, input: Option<&str>, sample: bool) -> Result<(), String> {
    if sample {
        return deckcraft_engine::sample::open_sample(s).map_err(|e| e.to_string());
    }
    match input {
        Some(p) => s.execute("file.open", &json!({"path": p})).map(|_| ()).map_err(|e| e.to_string()),
        None => s.execute("file.new", &json!({})).map(|_| ()).map_err(|e| e.to_string()),
    }
}

fn describe(id: Option<&str>) -> Result<(), String> {
    let id = id.ok_or("describe needs a command id")?;
    let c = deckcraft_engine::find_command(id).ok_or_else(|| format!("unknown command `{id}`"))?;
    outln!("{}  —  {}\n  where:    {}\n  shortcut: {}\n  params:   {}", c.id, c.label, c.menu.join(" ▸ "), c.shortcut.unwrap_or("—"), c.params);
    Ok(())
}
