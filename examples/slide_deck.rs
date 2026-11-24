//! Launch-a-Web SEDMAT v3.7 Slide Deck Studio (zero external dependencies, Rust stdlib only).
//!
//! Run: cargo run --example slide_deck

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use libsedmat::{
    get_completions, ContextInfo, ExecuteOptions, Executor, MemoryDocument, Profile,
};

const HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>libsedmat-rs — Slide Deck Studio</title>
  <style>
    :root { color-scheme: dark; font-family: ui-sans-serif, system-ui, sans-serif; }
    body { margin: 0; background: #020617; color: #f8fafc; padding: 24px; display: grid; gap: 16px; max-width: 960px; margin-inline: auto; }
    .stage { aspect-ratio: 16 / 9; border-radius: 16px; padding: 40px; display: flex; flex-direction: column; justify-content: center; border: 1px solid #334155; }
    .stage h2 { margin: 0 0 16px; font-size: 36px; }
    .stage li { font-size: 20px; margin: 8px 0; }
    .notes { background: #0f172a; padding: 12px 16px; border-radius: 8px; font-size: 14px; color: #94a3b8; }
    .bar { display: flex; gap: 8px; }
    input { flex: 1; background: #0f172a; color: #f8fafc; border: 1px solid #334155; border-radius: 8px; padding: 10px 12px; font-family: ui-monospace, monospace; }
    button { background: #2563eb; color: #fff; border: 0; border-radius: 8px; padding: 10px 16px; cursor: pointer; font-weight: 600; }
    .sugs { display: flex; flex-wrap: wrap; gap: 6px; }
    .sug { background: #1e293b; border: 1px solid #334155; color: #e2e8f0; border-radius: 6px; padding: 4px 8px; font-size: 12px; cursor: pointer; }
  </style>
</head>
<body>
  <h1 style="margin:0;font-size:20px;">libsedmat-rs — Slide Deck Studio (v3.7 Container Scopes)</h1>
  <div id="stage" class="stage"></div>
  <div id="notes" class="notes"></div>
  <div class="bar">
    <input id="cmd" value="@2:s/145ms/95ms/g" />
    <button id="run">Run</button>
    <button id="prev" style="background:#334155">Prev</button>
    <button id="next" style="background:#334155">Next</button>
  </div>
  <div id="sugs" class="sugs"></div>
  <script>
    let idx = 0, deck = [];
    const cmd = document.getElementById("cmd");
    async function render() {
      deck = await fetch("/api/slides").then(r => r.json());
      const s = deck[idx] || deck[0];
      const stage = document.getElementById("stage");
      stage.style.background = s.background || "#0F172A";
      stage.innerHTML = "<h2>" + s.title + "</h2><ul>" + s.paragraphs.map(p => "<li>" + p + "</li>").join("") + "</ul>";
      document.getElementById("notes").textContent = "Speaker notes (" + (idx+1) + "/" + deck.length + "): " + (s.notes || "(none)");
    }
    async function complete() {
      const r = await fetch("/api/complete", { method: "POST", body: cmd.value }).then(x => x.json());
      const sugs = document.getElementById("sugs");
      sugs.innerHTML = "";
      for (const s of r.suggestions.slice(0, 6)) {
        const b = document.createElement("button");
        b.className = "sug";
        b.textContent = s.label + " — " + s.detail;
        b.onclick = () => { cmd.value = cmd.value.slice(0, s.replaceFrom) + s.insertText + cmd.value.slice(s.replaceTo); complete(); };
        sugs.appendChild(b);
      }
    }
    document.getElementById("run").onclick = async () => {
      await fetch("/api/apply", { method: "POST", body: cmd.value });
      await render();
    };
    document.getElementById("prev").onclick = () => { idx = Math.max(0, idx - 1); render(); };
    document.getElementById("next").onclick = () => { idx = Math.min(deck.length - 1, idx + 1); render(); };
    cmd.addEventListener("input", complete);
    render(); complete();
  </script>
</body>
</html>"##;

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn initial_deck() -> MemoryDocument {
    let mut doc = MemoryDocument::new(&["Container-scoped editing with @1:, @2:, and @*:"]);
    doc.tabs[0].title = "SEDMAT v3.7 Slide Studio".to_string();
    doc.tabs[0].background = Some("#0F172A".to_string());
    doc.tabs[0].notes = Some("Welcome to the Rust Slide Deck Studio.".to_string());
    doc.add_tab(
        "slide-2",
        "Architecture Benchmarks",
        &[
            "Latency is 145ms across regions",
            "Zero external runtime dependencies",
        ],
    );
    doc.tabs[1].background = Some("#1E293B".to_string());
    doc.tabs[1].notes = Some("Highlight benchmark methodology.".to_string());
    doc
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn handle(mut stream: TcpStream, deck: &mut MemoryDocument) {
    let mut buf = [0u8; 16384];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");
    let body = req.split("\r\n\r\n").nth(1).unwrap_or("");

    match (method, path) {
        ("GET", "/") => respond(&mut stream, "200 OK", "text/html; charset=utf-8", HTML),
        ("GET", "/api/slides") => {
            let slides: Vec<String> = deck
                .tabs
                .iter()
                .map(|t| {
                    let paras: Vec<String> = t.paragraphs.iter().map(|p| json_escape(&p.text)).collect();
                    format!(
                        "{{\"id\":{},\"title\":{},\"paragraphs\":[{}],\"notes\":{},\"background\":{}}}",
                        json_escape(&t.id),
                        json_escape(&t.title),
                        paras.join(","),
                        json_escape(t.notes.as_deref().unwrap_or("")),
                        json_escape(t.background.as_deref().unwrap_or("#0F172A"))
                    )
                })
                .collect();
            respond(
                &mut stream,
                "200 OK",
                "application/json",
                &format!("[{}]", slides.join(",")),
            );
        }
        ("POST", "/api/complete") => {
            let res = get_completions(
                body,
                body.len(),
                &ContextInfo {
                    mode: Profile::Slides,
                    active_slide: Some(1),
                    total_slides: Some(deck.tabs.len()),
                    ..Default::default()
                },
            );
            let sugs: Vec<String> = res
                .suggestions
                .iter()
                .map(|s| {
                    format!(
                        "{{\"label\":{},\"insertText\":{},\"replaceFrom\":{},\"replaceTo\":{},\"detail\":{}}}",
                        json_escape(&s.label),
                        json_escape(&s.insert_text),
                        s.replace_from,
                        s.replace_to,
                        json_escape(&s.detail)
                    )
                })
                .collect();
            respond(
                &mut stream,
                "200 OK",
                "application/json",
                &format!("{{\"suggestions\":[{}]}}", sugs.join(",")),
            );
        }
        ("POST", "/api/apply") => {
            let mut ex = Executor::new(deck);
            match ex.execute_string(body, &ExecuteOptions::default()) {
                Ok(_) => respond(&mut stream, "200 OK", "application/json", "{\"ok\":true}"),
                Err(e) => respond(
                    &mut stream,
                    "400 Bad Request",
                    "application/json",
                    &format!("{{\"ok\":false,\"error\":{}}}", json_escape(&e.0)),
                ),
            }
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", "Not Found"),
    }
}

fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "4174".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).expect("bind HTTP port");
    println!("SEDMAT Rust Slide Deck Studio running at http://{addr}");
    let mut deck = initial_deck();
    for stream in listener.incoming().flatten() {
        handle(stream, &mut deck);
    }
}
