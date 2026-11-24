use crate::error::{Result, SedmatError};
use crate::types::{BraceExpression, BraceSpan, InlineSpan};

pub fn resolve_color(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    match lower.as_str() {
        "red" => "#FF0000".to_string(),
        "green" => "#008000".to_string(),
        "blue" => "#0000FF".to_string(),
        "yellow" => "#FFFF00".to_string(),
        "orange" => "#FFA500".to_string(),
        "purple" => "#800080".to_string(),
        "pink" => "#FFC0CB".to_string(),
        "cyan" | "aqua" => "#00FFFF".to_string(),
        "magenta" => "#FF00FF".to_string(),
        "white" => "#FFFFFF".to_string(),
        "black" => "#000000".to_string(),
        "gray" | "grey" => "#808080".to_string(),
        "navy" => "#000080".to_string(),
        "teal" => "#008080".to_string(),
        "maroon" => "#800000".to_string(),
        "olive" => "#808000".to_string(),
        "silver" => "#C0C0C0".to_string(),
        "lime" => "#00FF00".to_string(),
        "brown" => "#A52A2A".to_string(),
        "gold" => "#FFD700".to_string(),
        "indigo" => "#4B0082".to_string(),
        "violet" => "#EE82EE".to_string(),
        "coral" => "#FF7F50".to_string(),
        "crimson" => "#DC143C".to_string(),
        "transparent" => "transparent".to_string(),
        _ => {
            if let Some(hex) = lower.strip_prefix('#') {
                if hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    let cs: Vec<char> = hex.to_ascii_uppercase().chars().collect();
                    return format!("#{}{}{}{}{}{}", cs[0], cs[0], cs[1], cs[1], cs[2], cs[2]);
                }
                if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return format!("#{}", hex.to_ascii_uppercase());
                }
            }
            value.to_string()
        }
    }
}

fn bool_flag_name(flag: &str) -> Option<&'static str> {
    match flag {
        "b" | "bold" => Some("bold"),
        "i" | "italic" => Some("italic"),
        "_" | "underline" => Some("underline"),
        "-" | "strike" => Some("strike"),
        "#" | "code" => Some("code"),
        "^" | "sup" => Some("sup"),
        "," | "sub" => Some("sub"),
        "w" | "smallcaps" => Some("small_caps"),
        _ => None,
    }
}

fn set_bool_flag(expr: &mut BraceExpression, name: &str, val: bool) {
    match name {
        "bold" => expr.bold = Some(val),
        "italic" => expr.italic = Some(val),
        "underline" => expr.underline = Some(val),
        "strike" => expr.strike = Some(val),
        "code" => expr.code = Some(val),
        "sup" => expr.sup = Some(val),
        "sub" => expr.sub = Some(val),
        "small_caps" => expr.small_caps = Some(val),
        _ => {}
    }
}

fn unquote(s: &str) -> &str {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

pub fn tokenize_brace_content(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '\0';

    for ch in s.chars() {
        if in_quote {
            current.push(ch);
            if ch == quote_char {
                in_quote = false;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = ch;
            current.push(ch);
        } else if ch == ' ' || ch == '\t' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

pub fn parse_brace_expression(raw: &str) -> Result<BraceExpression> {
    let mut s = raw.trim().to_string();
    let mut expr = BraceExpression::default();
    if s.is_empty() {
        return Ok(expr);
    }

    if let Some(idx) = s.find("\"=") {
        expr.comment = Some(unquote(s[idx + 2..].trim()).to_string());
        s = s[..idx].trim().to_string();
    }

    if s == "0" || s.starts_with("0 ") || s.starts_with("0\t") {
        expr.reset = true;
        s = s[1..].trim().to_string();
    }

    for tok in tokenize_brace_content(&s) {
        if tok == "+" || tok.starts_with("+=") {
            expr.has_break = true;
            expr.break_kind = tok.strip_prefix("+=").unwrap_or("").to_string();
            continue;
        }
        if let Some(bm) = tok.strip_prefix("@=") {
            expr.bookmark = Some(unquote(bm).to_string());
            continue;
        }
        if tok == "0" {
            expr.reset = true;
            continue;
        }
        if tok == "!0" {
            expr.no_reset = true;
            continue;
        }
        if let Some(neg) = tok.strip_prefix('!') {
            if let Some(canon) = bool_flag_name(neg) {
                set_bool_flag(&mut expr, canon, false);
                continue;
            }
            return Err(SedmatError::new(format!("unknown negated flag: {neg}")));
        }
        if let Some((key, raw_val)) = tok.split_once('=') {
            let val = unquote(raw_val);
            if let Some(canon) = bool_flag_name(key) {
                expr.inline_spans.push(InlineSpan {
                    flag: canon.to_string(),
                    text: val.to_string(),
                });
                continue;
            }
            parse_kv(key, val, &mut expr)?;
            continue;
        }
        if let Some(canon) = bool_flag_name(&tok) {
            set_bool_flag(&mut expr, canon, true);
            continue;
        }
        match tok.as_str() {
            "check" => expr.check = Some(false),
            "toc" => {
                expr.has_toc = true;
                expr.toc = -1;
            }
            "h" => expr.heading = Some("1".to_string()),
            "p" => {
                expr.spacing_set = true;
                expr.spacing_above = 0.0;
                expr.spacing_below = 0.0;
            }
            "dup" => expr.dup = true,
            "hide" => expr.hide = true,
            "t" | "f" | "s" | "c" | "z" => {}
            _ => return Err(SedmatError::new(format!("unknown brace flag: {tok}"))),
        }
    }

    Ok(expr)
}

fn parse_kv(key: &str, val: &str, expr: &mut BraceExpression) -> Result<()> {
    match key {
        "t" | "text" => expr.text = Some(val.to_string()),
        "c" | "color" => expr.color = Some(resolve_color(val)),
        "z" | "bg" => expr.bg = Some(resolve_color(val)),
        "f" | "font" => expr.font = Some(val.to_string()),
        "s" | "size" => expr.size = val.parse().ok(),
        "u" | "url" => expr.url = Some(val.to_string()),
        "h" | "heading" => expr.heading = Some(val.to_string()),
        "l" | "leading" => expr.leading = val.parse().ok(),
        "a" | "align" => expr.align = Some(val.to_string()),
        "o" | "opacity" => expr.opacity = val.parse().ok(),
        "n" | "indent" => expr.indent = val.parse().ok(),
        "k" | "kerning" => expr.kerning = val.parse().ok(),
        "x" | "width" => expr.width = val.parse().ok(),
        "y" | "height" => expr.height = val.parse().ok(),
        "p" | "spacing" => {
            expr.spacing_set = true;
            if let Some((a, b)) = val.split_once(',') {
                expr.spacing_above = a.parse().unwrap_or(0.0);
                expr.spacing_below = b.parse().unwrap_or(0.0);
            } else {
                let v = val.parse().unwrap_or(0.0);
                expr.spacing_above = v;
                expr.spacing_below = v;
            }
        }
        "e" | "effect" => expr.effect = Some(val.to_string()),
        "cols" => expr.cols = val.parse().ok(),
        "check" => expr.check = Some(matches!(val, "y" | "true" | "1")),
        "toc" => {
            expr.has_toc = true;
            expr.toc = val.parse().unwrap_or(-1);
        }
        "img" => expr.img_ref = Some(val.to_string()),
        "T" => expr.table_ref = Some(val.to_string()),
        "layout" => expr.layout = Some(val.to_string()),
        "zone" => expr.zone = Some(val.to_string()),
        "shape" => expr.shape = Some(val.to_string()),
        "fill" => expr.fill = Some(resolve_color(val)),
        "stroke" => expr.stroke = Some(resolve_color(val)),
        "notes" => expr.notes = Some(val.to_string()),
        "transition" => expr.transition = Some(val.to_string()),
        "move" => expr.move_to = val.parse().ok(),
        _ => return Err(SedmatError::new(format!("unknown brace key: {key}"))),
    }
    Ok(())
}

pub fn find_matching_brace(s: &str, open_byte: usize) -> Option<usize> {
    if s[open_byte..].starts_with("{\"=") {
        return s[open_byte + 3..].find('}').map(|rel| open_byte + 3 + rel);
    }
    let mut depth = 0usize;
    let mut in_quote = false;
    let mut quote_char = '\0';
    for (rel, ch) in s[open_byte..].char_indices() {
        if in_quote {
            if ch == quote_char {
                in_quote = false;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_quote = true;
                quote_char = ch;
            }
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_byte + rel);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn looks_like_brace_expression(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.split_whitespace().next().unwrap_or("");
    if matches!(first, "0" | "!0" | "+" | "check" | "toc" | "dup" | "hide" | "f" | "s" | "c" | "z" | "h" | "p" | "t") {
        return true;
    }
    if bool_flag_name(first).is_some() {
        return true;
    }
    if let Some(neg) = first.strip_prefix('!') {
        if bool_flag_name(neg).is_some() {
            return true;
        }
    }
    if trimmed.starts_with("+=") || trimmed.starts_with("\"=") || trimmed.starts_with("@=") {
        return true;
    }
    const KEYS: &[&str] = &[
        "t=", "text=", "c=", "color=", "z=", "bg=", "f=", "font=", "s=", "size=", "u=", "url=",
        "h=", "heading=", "l=", "leading=", "a=", "align=", "o=", "opacity=", "n=", "indent=",
        "k=", "kerning=", "x=", "width=", "y=", "height=", "p=", "spacing=", "e=", "effect=",
        "cols=", "check=", "toc=", "img=", "T=", "@=", "\"=", "layout=", "zone=", "shape=",
        "fill=", "stroke=", "notes=", "transition=", "move=",
        "b=", "bold=", "i=", "italic=", "_=", "-=", "#=", "^=", ",=", "w=",
    ];
    KEYS.iter().any(|k| trimmed.contains(k))
}

pub fn brace_expression_has_any_format(expr: &BraceExpression) -> bool {
    expr.bold.is_some()
        || expr.italic.is_some()
        || expr.underline.is_some()
        || expr.strike.is_some()
        || expr.code.is_some()
        || expr.sup.is_some()
        || expr.sub.is_some()
        || expr.small_caps.is_some()
        || !expr.inline_spans.is_empty()
        || expr.text.is_some()
        || expr.color.is_some()
        || expr.bg.is_some()
        || expr.font.is_some()
        || expr.size.is_some()
        || expr.url.is_some()
        || expr.heading.is_some()
        || expr.leading.is_some()
        || expr.align.is_some()
        || expr.opacity.is_some()
        || expr.indent.is_some()
        || expr.kerning.is_some()
        || expr.width.is_some()
        || expr.height.is_some()
        || expr.spacing_set
        || expr.effect.is_some()
        || expr.cols.is_some()
        || expr.reset
        || expr.no_reset
        || expr.has_break
        || expr.comment.is_some()
        || expr.bookmark.is_some()
        || expr.check.is_some()
        || expr.has_toc
        || expr.img_ref.is_some()
        || expr.table_ref.is_some()
        || expr.layout.is_some()
        || expr.zone.is_some()
        || expr.shape.is_some()
        || expr.fill.is_some()
        || expr.stroke.is_some()
        || expr.notes.is_some()
        || expr.transition.is_some()
        || expr.move_to.is_some()
        || expr.dup
        || expr.hide
}

pub fn has_brace_formatting(replacement: &str) -> bool {
    let bytes = replacement.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'{' && (i == 0 || bytes[i - 1] != b'$') {
            if let Some(close) = find_matching_brace(replacement, i) {
                if looks_like_brace_expression(&replacement[i + 1..close]) {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

pub fn parse_brace_replacement(replacement: &str) -> (String, Vec<BraceSpan>) {
    if !replacement.contains('{') {
        return (replacement.to_string(), Vec::new());
    }
    let mut spans = Vec::new();
    let mut cleaned = String::new();
    let bytes = replacement.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if next == b'{' || next == b'}' {
                cleaned.push(next as char);
                i += 2;
                continue;
            }
        }
        if bytes[i] == b'{' && (i == 0 || bytes[i - 1] != b'$') {
            if let Some(close) = find_matching_brace(replacement, i) {
                let inner = &replacement[i + 1..close];
                if looks_like_brace_expression(inner) {
                    if let Ok(parsed) = parse_brace_expression(inner) {
                        if brace_expression_has_any_format(&parsed) {
                            let raw = replacement[i..=close].to_string();
                            if !parsed.inline_spans.is_empty() {
                                for inline in &parsed.inline_spans {
                                    let start = cleaned.len();
                                    cleaned.push_str(&inline.text);
                                    let end = cleaned.len();
                                    let mut span_expr = parsed.clone();
                                    span_expr.inline_spans.clear();
                                    set_bool_flag(&mut span_expr, &inline.flag, true);
                                    spans.push(BraceSpan {
                                        start,
                                        end,
                                        is_global: false,
                                        expr: span_expr,
                                        raw: raw.clone(),
                                    });
                                }
                            } else {
                                let start = cleaned.len();
                                if let Some(ref t) = parsed.text {
                                    cleaned.push_str(t);
                                }
                                let end = cleaned.len();
                                let is_global = i == 0 || close == bytes.len() - 1;
                                spans.push(BraceSpan {
                                    start,
                                    end,
                                    is_global,
                                    expr: parsed,
                                    raw,
                                });
                            }
                            i = close + 1;
                            continue;
                        }
                    }
                }
            }
        }
        let ch = replacement[i..].chars().next().unwrap();
        cleaned.push(ch);
        i += ch.len_utf8();
    }

    (cleaned, spans)
}

pub fn merge_brace_spans(spans: &[BraceSpan]) -> BraceExpression {
    let mut merged = BraceExpression::default();
    for span in spans {
        if !span.is_global {
            continue;
        }
        let s = &span.expr;
        if s.bold.is_some() { merged.bold = s.bold; }
        if s.italic.is_some() { merged.italic = s.italic; }
        if s.underline.is_some() { merged.underline = s.underline; }
        if s.strike.is_some() { merged.strike = s.strike; }
        if s.code.is_some() { merged.code = s.code; }
        if s.sup.is_some() { merged.sup = s.sup; }
        if s.sub.is_some() { merged.sub = s.sub; }
        if s.small_caps.is_some() { merged.small_caps = s.small_caps; }
        if s.color.is_some() { merged.color = s.color.clone(); }
        if s.bg.is_some() { merged.bg = s.bg.clone(); }
        if s.font.is_some() { merged.font = s.font.clone(); }
        if s.size.is_some() { merged.size = s.size; }
        if s.url.is_some() { merged.url = s.url.clone(); }
        if s.heading.is_some() { merged.heading = s.heading.clone(); }
        if s.align.is_some() { merged.align = s.align.clone(); }
        if s.leading.is_some() { merged.leading = s.leading; }
        if s.indent.is_some() { merged.indent = s.indent; }
        if s.cols.is_some() { merged.cols = s.cols; }
        if s.check.is_some() { merged.check = s.check; }
        if s.has_toc {
            merged.has_toc = true;
            merged.toc = s.toc;
        }
        if s.comment.is_some() { merged.comment = s.comment.clone(); }
        if s.bookmark.is_some() { merged.bookmark = s.bookmark.clone(); }
        if s.img_ref.is_some() { merged.img_ref = s.img_ref.clone(); }
        if s.table_ref.is_some() { merged.table_ref = s.table_ref.clone(); }
        if s.reset { merged.reset = true; }
        if s.no_reset { merged.no_reset = true; }
        if s.has_break {
            merged.has_break = true;
            merged.break_kind = s.break_kind.clone();
        }
    }
    merged
}
