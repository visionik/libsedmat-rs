//! Launch-a-Web SEDMAT Document Playground (zero external dependencies, Rust stdlib only).
//!
//! Run: cargo run --example doc_playground

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use libsedmat::{
    extract_diagnostic, format_numbered, get_completions, select_segment, tokenize, Backend,
    ContextInfo, ExecuteOptions, Executor, MemoryDocument, Profile, TokenKind,
};

const HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>libsedmat-rs — Document Playground</title>
  <style>
    :root { color-scheme: dark; font-family: ui-sans-serif, system-ui, sans-serif; }
    body { margin: 0; background: #0b1120; color: #e2e8f0; display: grid; place-items: center; min-height: 100vh; }
    .app { width: min(920px, 94vw); display: grid; gap: 16px; padding: 24px 0; }
    .card { background: #111827; border: 1px solid #1f2937; border-radius: 12px; padding: 16px; }
    .row { display: flex; gap: 8px; }
    input { flex: 1; background: #0f172a; color: #f8fafc; border: 1px solid #334155; border-radius: 8px; padding: 10px 12px; font-family: ui-monospace, monospace; font-size: 14px; }
    button { background: #2563eb; color: #fff; border: 0; border-radius: 8px; padding: 10px 16px; font-weight: 600; cursor: pointer; }
    button.sec { background: #334155; }
    .tokens { font-family: ui-monospace, monospace; font-size: 13px; margin-top: 8px; min-height: 20px; }
    .tok-scope { color: #c084fc; } .tok-address { color: #fbbf24; } .tok-command { color: #38bdf8; font-weight: 700; }
    .tok-pattern { color: #f472b6; } .tok-brace { color: #4ade80; } .tok-flag { color: #fb923c; } .tok-delimiter { color: #64748b; }
    .ok { color: #4ade80; } .err { color: #f87171; }
    .sugs { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
    .sug { background: #1e293b; border: 1px solid #334155; color: #e2e8f0; border-radius: 6px; padding: 4px 8px; font-size: 12px; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; }
    .swatch { width: 10px; height: 10px; border-radius: 2px; border: 1px solid #475569; }
    pre { margin: 0; white-space: pre-wrap; font-family: ui-monospace, monospace; font-size: 14px; line-height: 1.6; }
  </style>
</head>
<body>
  <div class="app">
    <h1 style="margin:0;font-size:20px;">libsedmat-rs — Document Playground (Zero-Dep Rust stdlib)</h1>
    <div class="card">
      <div class="row">
        <input id="cmd" value="2s/TODO/{b c=red}/g" />
        <button id="apply">Apply</button>
        <button id="reset" class="sec">Reset</button>
      </div>
      <div id="tokens" class="tokens"></div>
      <div id="status" style="font-size:13px;margin-top:8px;"></div>
      <div id="sugs" class="sugs"></div>
    </div>
    <div class="card">
      <h3 style="margin:0 0 10px;font-size:13px;color:#94a3b8;text-transform:uppercase;">Numbered Document (cat -N)</h3>
      <pre id="doc"></pre>
    </div>
  </div>
  <script>
    const cmd = document.getElementById("cmd");
    async function refresh() {
      const r = await fetch("/api/state").then(x => x.json());
      document.getElementById("doc").textContent = r.numbered;
    }
    async function inspect() {
      const input = cmd.value;
      const r = await fetch("/api/inspect", { method: "POST", body: input }).then(x => x.json());
      document.getElementById("tokens").innerHTML = r.tokens.map(t => '<span class="tok-'+t.kind+'">'+t.text.replace(/[<>&]/g,c=>({'<':'&lt;','>':'&gt;','&':'&amp;'}[c]))+'</span>').join("") + (r.ghost ? '<span style="opacity:.4">'+r.ghost+'</span>' : "");
      const st = document.getElementById("status");
      st.className = r.ok ? "ok" : "err";
      st.textContent = (r.ok ? "✓ " : "✗ ") + r.summary;
      const sugs = document.getElementById("sugs");
      sugs.innerHTML = "";
      for (const s of r.suggestions.slice(0, 8)) {
        const b = document.createElement("button");
        b.className = "sug";
        b.innerHTML = (s.swatch ? '<span class="swatch" style="background:'+s.swatch+'"></span>' : '') + '<strong>'+s.label+'</strong> <span style="opacity:.7">'+s.detail+'</span>';
        b.onclick = () => { cmd.value = input.slice(0, s.replaceFrom) + s.insertText + input.slice(s.replaceTo); cmd.focus(); inspect(); };
        sugs.appendChild(b);
      }
    }
    document.getElementById("apply").onclick = async () => {
      await fetch("/api/apply", { method: "POST", body: cmd.value });
      await refresh(); await inspect();
    };
    document.getElementById("reset").onclick = async () => { await fetch("/api/reset", { method: "POST" }); await refresh(); };
    cmd.addEventListener("input", inspect);
    refresh(); inspect();
  </script>
</body>
</html>"#;

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

fn kind_name(k: TokenKind) -> &'static str {
    match k {
        TokenKind::Comment => "comment",
        TokenKind::Scope => "scope",
        TokenKind::Address => "address",
        TokenKind::Command => "command",
        TokenKind::Delimiter => "delimiter",
        TokenKind::Pattern => "pattern",
        TokenKind::Brace => "brace",
        TokenKind::Text => "text",
        TokenKind::Flag => "flag",
        TokenKind::Whitespace => "whitespace",
    }
}

fn initial_doc() -> MemoryDocument {
    MemoryDocument::new(&[
        "SEDMAT v3.7 Interactive Document",
        "This paragraph contains a TODO item and a critical warning.",
        "DRAFT: delete this scratch note with 3d or d/DRAFT/",
        "Closing summary ready for formatting and appends.",
    ])
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn handle(mut stream: TcpStream, doc: &mut MemoryDocument) {
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
        ("GET", "/api/state") => {
            let numbered = doc
                .project()
                .ok()
                .and_then(|p| select_segment(&p, "").ok().map(format_numbered))
                .unwrap_or_default();
            respond(
                &mut stream,
                "200 OK",
                "application/json",
                &format!("{{\"numbered\":{}}}", json_escape(&numbered)),
            );
        }
        ("POST", "/api/reset") => {
            *doc = initial_doc();
            respond(&mut stream, "200 OK", "application/json", "{\"ok\":true}");
        }
        ("POST", "/api/inspect") => {
            let res = get_completions(
                body,
                body.len(),
                &ContextInfo {
                    mode: Profile::Document,
                    ..Default::default()
                },
            );
            let toks: Vec<String> = tokenize(body)
                .into_iter()
                .map(|t| {
                    format!(
                        "{{\"kind\":\"{}\",\"text\":{}}}",
                        kind_name(t.kind),
                        json_escape(&t.text)
                    )
                })
                .collect();
            let sugs: Vec<String> = res
                .suggestions
                .iter()
                .map(|s| {
                    let sw = s
                        .swatch
                        .as_deref()
                        .map(json_escape)
                        .unwrap_or_else(|| "null".to_string());
                    format!(
                        "{{\"label\":{},\"insertText\":{},\"replaceFrom\":{},\"replaceTo\":{},\"detail\":{},\"swatch\":{sw}}}",
                        json_escape(&s.label),
                        json_escape(&s.insert_text),
                        s.replace_from,
                        s.replace_to,
                        json_escape(&s.detail)
                    )
                })
                .collect();
            let (ok, summary) = if body.trim().is_empty() {
                (true, "Ready".to_string())
            } else {
                let mut ex = Executor::new(doc);
                match ex.execute_string(
                    body,
                    &ExecuteOptions {
                        dry_run: true,
                        ..Default::default()
                    },
                ) {
                    Ok(rep) => (true, format!("Dry-run matched {} operation(s)", rep.total_matches)),
                    Err(e) => {
                        let d = extract_diagnostic(&e.0);
                        (false, d.summary)
                    }
                }
            };
            let ghost = res
                .ghost_suffix
                .as_deref()
                .map(json_escape)
                .unwrap_or_else(|| "null".to_string());
            let json = format!(
                "{{\"ok\":{ok},\"summary\":{},\"ghost\":{ghost},\"tokens\":[{}],\"suggestions\":[{}]}}",
                json_escape(&summary),
                toks.join(","),
                sugs.join(",")
            );
            respond(&mut stream, "200 OK", "application/json", &json);
        }
        ("POST", "/api/apply") => {
            let mut ex = Executor::new(doc);
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
    let port = std::env::var("PORT").unwrap_or_else(|_| "4173".to_string());
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).expect("bind HTTP port");
    println!("SEDMAT Rust Document Playground running at http://{addr}");
    let mut doc = initial_doc();
    for stream in listener.incoming().flatten() {
        handle(stream, &mut doc);
    }
}
