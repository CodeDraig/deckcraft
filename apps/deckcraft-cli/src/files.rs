//! Presentation inspection and file conversion commands.

use super::open;
use deckcraft_engine::Session;
use serde_json::json;

pub(super) fn info(args: &[String]) -> Result<(), String> {
    let path = args.first().ok_or("info needs a FILE")?;
    let mut s = Session::new();
    open(&mut s, Some(path), false)?;
    let v = s.execute("document.inspect", &json!({})).map_err(|e| e.to_string())?;
    outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    Ok(())
}

pub(super) fn convert(args: &[String]) -> Result<(), String> {
    let (Some(input), Some(out)) = (args.first(), args.get(1)) else { return Err("convert needs IN and OUT".into()) };
    let mut s = Session::new();
    open(&mut s, Some(input), false)?;
    let fmt = deckcraft_engine::cmd::file::format_for_path(out);
    let r = if matches!(fmt, "png" | "jpeg") {
        s.execute("file.export", &json!({"path": out, "slide": 0}))
    } else {
        s.execute("file.saveAs", &json!({"path": out}))
    };
    r.map_err(|e| e.to_string())?;
    outln!("{out}");
    Ok(())
}
