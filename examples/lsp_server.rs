//! Zero-dependency SEDMAT Language Server (LSP over stdio).
//!
//! Run: cargo run --example lsp_server

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};

use libsedmat::{
    extract_diagnostic, get_completions, parse_expression, tokenize, ContextInfo, Profile,
    SuggestionKind, TokenKind,
};

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
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn find_json_string(body: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let idx = body.find(&pat)?;
    let rest = &body[idx + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = after[1..].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                other => out.push(other),
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn find_json_number(body: &str, key: &str) -> Option<usize> {
    let pat = format!("\"{key}\"");
    let idx = body.find(&pat)?;
    let rest = &body[idx + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

fn send_frame(out: &mut impl Write, json_body: &str) {
    let _ = write!(out, "Content-Length: {}\r\n\r\n{}", json_body.len(), json_body);
    let _ = out.flush();
}

fn read_frame(reader: &mut impl BufRead) -> Option<String> {
    let mut content_len: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
            content_len = rest.trim().parse().ok();
        }
    }
    let len = content_len?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

fn kind_to_lsp(k: SuggestionKind) -> u8 {
    match k {
        SuggestionKind::Scope => 9,
        SuggestionKind::Command => 14,
        SuggestionKind::Flag => 10,
        SuggestionKind::Value => 12,
        SuggestionKind::Snippet => 15,
    }
}

fn token_type_index(k: TokenKind) -> usize {
    match k {
        TokenKind::Comment => 0,
        TokenKind::Scope => 1,
        TokenKind::Address => 2,
        TokenKind::Command => 3,
        TokenKind::Delimiter => 4,
        TokenKind::Pattern => 5,
        TokenKind::Brace => 6,
        TokenKind::Text => 7,
        TokenKind::Flag => 8,
        TokenKind::Whitespace => 9,
    }
}

fn offset_to_line_char(text: &str, offset: usize) -> (usize, usize) {
    let safe = offset.min(text.len());
    let slice = &text[..safe];
    let line = slice.bytes().filter(|&b| b == b'\n').count();
    let last_nl = slice.rfind('\n').map(|i| i + 1).unwrap_or(0);
    (line, safe - last_nl)
}

fn publish_diagnostics(out: &mut impl Write, uri: &str, text: &str) {
    let mut items = Vec::new();
    for (idx, line) in text.split('\n').enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Err(err) = parse_expression(trimmed) {
            let d = extract_diagnostic(&err.0);
            let col = d.error_offset.unwrap_or(0);
            let len = d.error_length.max(1);
            let msg = match d.hint {
                Some(h) => format!("{} — {h}", d.summary),
                None => d.summary,
            };
            items.push(format!(
                "{{\"range\":{{\"start\":{{\"line\":{idx},\"character\":{col}}},\"end\":{{\"line\":{idx},\"character\":{}}}}},\"severity\":1,\"source\":\"libsedmat-rs\",\"message\":{}}}",
                col + len,
                json_escape(&msg)
            ));
        }
    }
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/publishDiagnostics\",\"params\":{{\"uri\":{},\"diagnostics\":[{}]}}}}",
        json_escape(uri),
        items.join(",")
    );
    send_frame(out, &body);
}

fn encode_semantic_tokens(text: &str) -> String {
    let tokens = tokenize(text);
    let mut nums = Vec::new();
    let (mut prev_line, mut prev_char) = (0usize, 0usize);
    for tok in tokens {
        if tok.kind == TokenKind::Whitespace || tok.text.contains('\n') {
            continue;
        }
        let (line, ch) = offset_to_line_char(text, tok.start);
        let delta_line = line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            ch.saturating_sub(prev_char)
        } else {
            ch
        };
        nums.push(format!(
            "{delta_line},{delta_start},{},{},0",
            tok.end - tok.start,
            token_type_index(tok.kind)
        ));
        prev_line = line;
        prev_char = ch;
    }
    nums.join(",")
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut out = stdout.lock();
    let mut docs: HashMap<String, String> = HashMap::new();
    let mut shutdown = false;

    while let Some(body) = read_frame(&mut reader) {
        let method = find_json_string(&body, "method").unwrap_or_default();
        let id = find_json_number(&body, "id");

        match method.as_str() {
            "initialize" => {
                let id_num = id.unwrap_or(1);
                let resp = format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":{id_num},\"result\":{{\"capabilities\":{{\"textDocumentSync\":1,\"completionProvider\":{{\"triggerCharacters\":[\"{{\",\"@\",\"/\",\"=\",\":\",\" \"]}},\"signatureHelpProvider\":{{\"triggerCharacters\":[\"/\",\"{{\",\"=\"]}},\"semanticTokensProvider\":{{\"legend\":{{\"tokenTypes\":[\"comment\",\"namespace\",\"number\",\"keyword\",\"operator\",\"regexp\",\"decorator\",\"string\",\"modifier\",\"variable\"],\"tokenModifiers\":[]}},\"full\":true}}}},\"serverInfo\":{{\"name\":\"libsedmat-rs-lsp\",\"version\":\"0.1.0\"}}}}}}"
                );
                send_frame(&mut out, &resp);
            }
            "initialized" => {}
            "shutdown" => {
                shutdown = true;
                let id_num = id.unwrap_or(0);
                send_frame(
                    &mut out,
                    &format!("{{\"jsonrpc\":\"2.0\",\"id\":{id_num},\"result\":null}}"),
                );
            }
            "exit" => {
                std::process::exit(if shutdown { 0 } else { 1 });
            }
            "textDocument/didOpen" | "textDocument/didChange" => {
                if let (Some(uri), Some(text)) =
                    (find_json_string(&body, "uri"), find_json_string(&body, "text"))
                {
                    publish_diagnostics(&mut out, &uri, &text);
                    docs.insert(uri, text);
                }
            }
            "textDocument/completion" => {
                let id_num = id.unwrap_or(0);
                let uri = find_json_string(&body, "uri").unwrap_or_default();
                let line = find_json_number(&body, "line").unwrap_or(0);
                let ch = find_json_number(&body, "character").unwrap_or(0);
                let text = docs.get(&uri).map(String::as_str).unwrap_or("");
                let line_text = text.split('\n').nth(line).unwrap_or("");
                let res = get_completions(
                    line_text,
                    ch,
                    &ContextInfo {
                        mode: Profile::All,
                        ..Default::default()
                    },
                );
                let items: Vec<String> = res
                    .suggestions
                    .iter()
                    .map(|s| {
                        let detail = match &s.swatch {
                            Some(sw) => format!("{} ({sw})", s.detail),
                            None => s.detail.clone(),
                        };
                        format!(
                            "{{\"label\":{},\"kind\":{},\"detail\":{},\"textEdit\":{{\"range\":{{\"start\":{{\"line\":{line},\"character\":{}}},\"end\":{{\"line\":{line},\"character\":{}}}}},\"newText\":{}}}}}",
                            json_escape(&s.label),
                            kind_to_lsp(s.kind),
                            json_escape(&detail),
                            s.replace_from,
                            s.replace_to,
                            json_escape(&s.insert_text)
                        )
                    })
                    .collect();
                send_frame(
                    &mut out,
                    &format!(
                        "{{\"jsonrpc\":\"2.0\",\"id\":{id_num},\"result\":{{\"isIncomplete\":false,\"items\":[{}]}}}}",
                        items.join(",")
                    ),
                );
            }
            "textDocument/semanticTokens/full" => {
                let id_num = id.unwrap_or(0);
                let uri = find_json_string(&body, "uri").unwrap_or_default();
                let text = docs.get(&uri).map(String::as_str).unwrap_or("");
                let data = encode_semantic_tokens(text);
                send_frame(
                    &mut out,
                    &format!("{{\"jsonrpc\":\"2.0\",\"id\":{id_num},\"result\":{{\"data\":[{data}]}}}}"),
                );
            }
            _ => {
                if let Some(id_num) = id {
                    send_frame(
                        &mut out,
                        &format!(
                            "{{\"jsonrpc\":\"2.0\",\"id\":{id_num},\"error\":{{\"code\":-32601,\"message\":{}}}}}",
                            json_escape(&format!("Method not found: {method}"))
                        ),
                    );
                }
            }
        }
    }
}
