//! Ordered engine-command batch execution.

use super::open;
use deckcraft_engine::Session;
use serde_json::{Value, json};

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut sample = false;
    let mut cmds = vec![];
    let mut save = None;
    let mut export = None;
    let mut print = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--in" => input = Some(it.next().cloned().ok_or("--in needs a file")?),
            "--sample" => sample = true,
            "--cmd" => cmds.push(it.next().cloned().ok_or("--cmd needs ID[=JSON]")?),
            "--save" => save = Some(it.next().cloned().ok_or("--save needs a path")?),
            "--export" => export = Some(it.next().cloned().ok_or("--export needs a path")?),
            "--print" => print = true,
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    let mut s = Session::new();
    open(&mut s, input.as_deref(), sample)?;
    for c in cmds {
        let (id, p) = match c.split_once('=') {
            Some((id, j)) => (id.to_string(), serde_json::from_str::<Value>(j).map_err(|e| format!("{id}: bad JSON: {e}"))?),
            None => (c.clone(), json!({})),
        };
        let r = s.execute(&id, &p).map_err(|e| e.to_string())?;
        if print {
            outln!("{id} → {r}");
        }
    }
    if let Some(p) = save {
        s.execute("file.saveAs", &json!({"path": p})).map_err(|e| e.to_string())?;
    }
    if let Some(p) = export {
        s.execute("file.export", &json!({"path": p})).map_err(|e| e.to_string())?;
    }
    Ok(())
}
