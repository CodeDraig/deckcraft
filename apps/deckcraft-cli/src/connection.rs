//! Desktop control and MCP connection commands.

use serde_json::{Value, json};

pub(super) fn app(args: &[String]) -> Result<(), String> {
    use deckcraft_mcp::{Backend, Remote, control_addr};
    let mut port = deckcraft_mcp::DEFAULT_PORT.to_string();
    let mut method: Option<String> = None;
    let mut rest = vec![];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next().cloned().ok_or("--port needs a value")?,
            "--method" => method = Some(it.next().cloned().ok_or("--method needs a name")?),
            other => rest.push(other.to_string()),
        }
    }
    let mut r = Remote::connect(&control_addr(&port))
        .map_err(|e| format!("no DeckCraft app on port {port} ({e}); start it with `deckcraft --control {port}`"))?;
    let parse = |s: Option<&String>| -> Result<Value, String> {
        s.map(|j| serde_json::from_str(j).map_err(|e| format!("bad JSON: {e}"))).transpose().map(|v| v.unwrap_or(json!({})))
    };
    let v = match method {
        Some(m) => r.call(&m, parse(rest.first())?)?,
        None => {
            let id = rest.first().ok_or("app needs a COMMAND or --method")?;
            r.call("engine.execute", json!({"command": id, "params": parse(rest.get(1))?}))?
        }
    };
    outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    Ok(())
}

pub(super) fn mcp(args: &[String]) -> Result<(), String> {
    use deckcraft_mcp::{Backend, Headless, Remote, Server, control_addr};
    let mut connect: Option<String> = None;
    let mut sample = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--connect" => connect = Some(it.next().cloned().ok_or("--connect needs a port or host:port")?),
            "--sample" => sample = true,
            other => return Err(format!("unknown mcp option `{other}`")),
        }
    }
    let backend: Box<dyn Backend> = match connect {
        Some(c) => Box::new(Remote::connect(&control_addr(&c)).map_err(|e| format!("can't reach the app at {c}: {e}"))?),
        None => {
            let mut h = Headless::new();
            if sample {
                deckcraft_engine::sample::open_sample(&mut h.session).map_err(|e| e.to_string())?;
            }
            Box::new(h)
        }
    };
    let mut server = Server::new(backend);
    let stdin = std::io::stdin();
    server.serve(stdin.lock(), std::io::stdout()).map_err(|e| e.to_string())
}
