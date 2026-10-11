//! Slide render command.

use super::open;
use deckcraft_engine::Session;

pub(super) fn render(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut sample = false;
    let mut slide: Option<usize> = None;
    let mut all = false;
    let mut scale = 2.0;
    let mut edit = false;
    let mut out = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--sample" => sample = true,
            "--slide" => slide = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--slide needs a number")?),
            "--all" => all = true,
            "--edit" => edit = true,
            "--scale" => scale = it.next().and_then(|v| v.parse().ok()).ok_or("--scale needs a number")?,
            other
                if input.is_none()
                    && !sample
                    && !other.starts_with("--")
                    && out.is_none()
                    && !(other.ends_with(".png") || other.ends_with(".jpg"))
                    || (input.is_none() && !sample && std::path::Path::new(other).is_file()) =>
            {
                input = Some(other.to_string())
            }
            other => out = Some(other.to_string()),
        }
    }
    let out = out.ok_or("render needs an output path")?;
    let mut s = Session::new();
    open(&mut s, input.as_deref(), sample)?;
    let st = s.doc().map_err(|e| e.to_string())?;
    let n = st.doc.slides.len();
    let list: Vec<usize> = if all { (0..n).collect() } else { vec![slide.unwrap_or(0)] };
    if all {
        std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
    }
    for i in list {
        if i >= n {
            return Err(format!("no slide {i} (the deck has {n})"));
        }
        let (png, w, h) = deckcraft_engine::cmd::file::render_png(&st.doc, i, scale, edit);
        let path = if all { format!("{out}/slide-{:02}.png", i + 1) } else { out.clone() };
        std::fs::write(&path, png).map_err(|e| format!("{path}: {e}"))?;
        outln!("{path} ({w}×{h})");
    }
    Ok(())
}
