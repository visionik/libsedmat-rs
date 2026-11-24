use crate::brace::{
    find_matching_brace, has_brace_formatting, merge_brace_spans, parse_brace_expression,
    parse_brace_replacement,
};
use crate::error::{Result, SedmatError};
use crate::types::{Address, Command, ContainerScope, Expression, Program, TableCreateSpec};

pub fn parse_address(raw: &str) -> Result<(Option<Address>, &str)> {
    let bytes = raw.as_bytes();
    if bytes.is_empty() || (bytes[0] != b'$' && !bytes[0].is_ascii_digit()) {
        return Ok((None, raw));
    }
    let mut i = 0;
    let start: i32;
    if bytes[i] == b'$' {
        start = -1;
        i += 1;
    } else {
        let s_idx = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        start = raw[s_idx..i]
            .parse::<i32>()
            .map_err(|_| SedmatError::new(format!("invalid address: {raw}")))?;
        if start < 1 {
            return Err(SedmatError::new(format!("address must be >= 1, got {start}")));
        }
    }

    let mut has_range = false;
    let mut end = 0;
    if i < bytes.len() && bytes[i] == b',' {
        if start == -1 {
            return Err(SedmatError::new("'$' cannot be used as range start"));
        }
        has_range = true;
        i += 1;
        if i >= bytes.len() {
            return Err(SedmatError::new("missing range end after ','"));
        }
        if bytes[i] == b'$' {
            end = -1;
            i += 1;
        } else if bytes[i].is_ascii_digit() {
            let e_idx = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            end = raw[e_idx..i]
                .parse::<i32>()
                .map_err(|_| SedmatError::new(format!("invalid range end: {raw}")))?;
            if end < 1 {
                return Err(SedmatError::new(format!("address range end must be >= 1, got {end}")));
            }
            if end < start {
                return Err(SedmatError::new(format!("address range {start},{end} is reversed")));
            }
        } else {
            return Err(SedmatError::new(format!("invalid range end in {raw}")));
        }
    }

    let rest = &raw[i..];
    if rest.is_empty() {
        return Err(SedmatError::new(format!("address without command: {raw}")));
    }
    Ok((
        Some(Address {
            start,
            end,
            has_range,
        }),
        rest,
    ))
}

pub fn parse_container_scope(raw: &str) -> Result<(Option<ContainerScope>, &str)> {
    let Some(after_at) = raw.strip_prefix('@') else {
        return Ok((None, raw));
    };
    if after_at.is_empty() {
        return Err(SedmatError::new("empty container scope '@'"));
    }

    // Bare region shorthand `@header:...` / `@footer:...`
    for reg in ["header:", "footer:"] {
        if after_at.len() > reg.len() && after_at[..reg.len()].eq_ignore_ascii_case(reg) {
            return Ok((
                Some(ContainerScope {
                    container_type: None,
                    address: None,
                    all: false,
                    name: None,
                    region: Some(reg.trim_end_matches(':').to_string()),
                    container_op: false,
                }),
                &after_at[reg.len()..],
            ));
        }
    }

    let mut rest = after_at;
    let mut container_type = None;
    for t in ["slide=", "sheet=", "tab=", "page=", "section="] {
        if let Some(r) = rest.strip_prefix(t) {
            container_type = Some(t.trim_end_matches('=').to_string());
            rest = r;
            break;
        }
    }

    let mut address = None;
    let mut all = false;
    let mut name = None;

    if let Some(r) = rest.strip_prefix('*') {
        all = true;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('"') {
        let Some(end_q) = r.find('"') else {
            return Err(SedmatError::new(format!("unterminated quoted container name: {raw}")));
        };
        name = Some(r[..end_q].to_string());
        rest = &r[end_q + 1..];
    } else if rest.starts_with('$') || rest.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
        let mut idx = 0;
        let b = rest.as_bytes();
        if b[idx] == b'$' {
            idx += 1;
        } else {
            while idx < b.len() && b[idx].is_ascii_digit() {
                idx += 1;
            }
        }
        if idx < b.len() && b[idx] == b',' {
            idx += 1;
            if idx < b.len() && b[idx] == b'$' {
                idx += 1;
            } else {
                while idx < b.len() && b[idx].is_ascii_digit() {
                    idx += 1;
                }
            }
        }
        let dummy = format!("{}d", &rest[..idx]);
        let (addr, _) = parse_address(&dummy)?;
        address = addr;
        rest = &rest[idx..];
    } else {
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(rest.len());
        if end == 0 {
            return Err(SedmatError::new(format!("invalid container target: {raw}")));
        }
        name = Some(rest[..end].to_string());
        rest = &rest[end..];
    }

    if let Some(after_colon) = rest.strip_prefix(':') {
        let mut region = None;
        let mut body = after_colon;
        if let Some(c_idx) = after_colon.find(':') {
            let cand = &after_colon[..c_idx];
            let lower = cand.to_ascii_lowercase();
            if matches!(lower.as_str(), "title" | "sub" | "subtitle" | "body" | "notes" | "header" | "footer")
                || lower.strip_prefix("shape=").is_some_and(|n| n.parse::<usize>().is_ok_and(|v| v >= 1))
            {
                region = Some(lower);
                body = &after_colon[c_idx + 1..];
            }
        }
        if body.is_empty() {
            return Err(SedmatError::new(format!("container scope without command: {raw}")));
        }
        return Ok((
            Some(ContainerScope {
                container_type,
                address,
                all,
                name,
                region,
                container_op: false,
            }),
            body,
        ));
    }

    if rest.starts_with('d') || rest.starts_with('a') || rest.starts_with('i') || rest.starts_with('{') {
        return Ok((
            Some(ContainerScope {
                container_type,
                address,
                all,
                name,
                region: None,
                container_op: true,
            }),
            rest,
        ));
    }

    Err(SedmatError::new(format!("invalid container scope: {raw}")))
}

fn split_by_delimiter(value: &str, delim: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            if chars[i + 1] == delim {
                current.push(delim);
            } else {
                current.push('\\');
                current.push(chars[i + 1]);
            }
            i += 2;
            continue;
        }
        if chars[i] == delim {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(chars[i]);
        }
        i += 1;
    }
    parts.push(current);
    parts
}

fn apply_regex_flags(pattern: &str, flags: &str) -> String {
    let mut res = pattern.to_string();
    if flags.contains('i') && !res.starts_with("(?i)") {
        res = format!("(?i){res}");
    }
    if flags.contains('m') && !res.starts_with("(?m)") {
        res = format!("(?m){res}");
    }
    res
}

fn nth_flag(flags: &str) -> usize {
    let digits: String = flags.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<usize>().unwrap_or(0)
}

pub fn parse_expression(raw: &str) -> Result<Expression> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SedmatError::new("empty expression"));
    }

    let (container, after_container) = parse_container_scope(trimmed)?;

    if container.is_some() && after_container.starts_with('{') {
        let Some(close) = find_matching_brace(after_container, 0) else {
            return Err(SedmatError::new(format!("unclosed container brace: {raw}")));
        };
        if close != after_container.len() - 1 {
            return Err(SedmatError::new(format!("trailing chars after container brace: {raw}")));
        }
        let brace = parse_brace_expression(&after_container[1..close])?;
        return Ok(Expression {
            command: Command::Brace,
            pattern: String::new(),
            replacement: after_container.to_string(),
            global: false,
            nth_match: 0,
            flags: String::new(),
            address: None,
            container,
            brace: Some(brace),
            brace_spans: Vec::new(),
            table_create: None,
            enriched: true,
        });
    }

    let (address, body) = if container.as_ref().is_some_and(|c| c.container_op) {
        (None, after_container)
    } else {
        parse_address(after_container)?
    };
    let has_addr = address.is_some() || container.is_some();

    let first = body.chars().next().ok_or_else(|| SedmatError::new("empty command"))?;
    let mut expr = match first {
        's' => {
            let mut chars = body.chars();
            chars.next();
            let delim = chars.next().ok_or_else(|| SedmatError::new(format!("missing delimiter: {raw}")))?;
            if delim.is_ascii_alphanumeric() || delim == '\\' {
                return Err(SedmatError::new(format!("invalid delimiter '{delim}'")));
            }
            let parts = split_by_delimiter(chars.as_str(), delim);
            if parts.len() < 3 || parts.len() > 4 {
                return Err(SedmatError::new(format!("malformed substitution: {raw}")));
            }
            let flags = parts[2].clone();
            Expression {
                command: Command::Substitute,
                pattern: apply_regex_flags(&parts[0], &flags),
                replacement: parts[1].clone(),
                global: flags.contains('g'),
                nth_match: nth_flag(&flags),
                flags,
                address: None,
                container: None,
                brace: None,
                brace_spans: Vec::new(),
                table_create: None,
                enriched: false,
            }
        }
        'd' => {
            if body == "d" {
                if !has_addr {
                    return Err(SedmatError::new("delete command requires pattern or address"));
                }
                Expression {
                    command: Command::Delete,
                    pattern: String::new(),
                    replacement: String::new(),
                    global: false,
                    nth_match: 0,
                    flags: String::new(),
                    address: None,
                    container: None,
                    brace: None,
                    brace_spans: Vec::new(),
                    table_create: None,
                    enriched: false,
                }
            } else {
                let mut chars = body.chars();
                chars.next();
                let delim = chars.next().ok_or_else(|| SedmatError::new(format!("missing delimiter: {raw}")))?;
                let parts = split_by_delimiter(chars.as_str(), delim);
                if parts.len() < 2 {
                    return Err(SedmatError::new(format!("malformed delete: {raw}")));
                }
                let flags = parts[1].clone();
                Expression {
                    command: Command::Delete,
                    pattern: apply_regex_flags(&parts[0], &flags),
                    replacement: String::new(),
                    global: flags.contains('g'),
                    nth_match: 0,
                    flags,
                    address: None,
                    container: None,
                    brace: None,
                    brace_spans: Vec::new(),
                    table_create: None,
                    enriched: false,
                }
            }
        }
        'a' | 'i' => {
            let cmd = if first == 'a' { Command::Append } else { Command::Insert };
            let mut chars = body.chars();
            chars.next();
            let delim = chars.next().ok_or_else(|| SedmatError::new(format!("missing delimiter: {raw}")))?;
            let parts = split_by_delimiter(chars.as_str(), delim);
            if has_addr {
                if parts.len() < 2 {
                    return Err(SedmatError::new(format!("malformed addressed {first}: {raw}")));
                }
                Expression {
                    command: cmd,
                    pattern: String::new(),
                    replacement: parts[0].clone(),
                    global: false,
                    nth_match: 0,
                    flags: parts[1].clone(),
                    address: None,
                    container: None,
                    brace: None,
                    brace_spans: Vec::new(),
                    table_create: None,
                    enriched: false,
                }
            } else {
                if parts.len() < 3 {
                    return Err(SedmatError::new(format!("malformed {first}: {raw}")));
                }
                let flags = parts[2].clone();
                Expression {
                    command: cmd,
                    pattern: apply_regex_flags(&parts[0], &flags),
                    replacement: parts[1].clone(),
                    global: flags.contains('g'),
                    nth_match: 0,
                    flags,
                    address: None,
                    container: None,
                    brace: None,
                    brace_spans: Vec::new(),
                    table_create: None,
                    enriched: false,
                }
            }
        }
        'y' => {
            let mut chars = body.chars();
            chars.next();
            let delim = chars.next().ok_or_else(|| SedmatError::new(format!("missing delimiter: {raw}")))?;
            let parts = split_by_delimiter(chars.as_str(), delim);
            if parts.len() < 3 {
                return Err(SedmatError::new(format!("malformed transliterate: {raw}")));
            }
            if parts[0].chars().count() != parts[1].chars().count() {
                return Err(SedmatError::new("transliterate source and dest lengths differ"));
            }
            Expression {
                command: Command::Transliterate,
                pattern: parts[0].clone(),
                replacement: parts[1].clone(),
                global: false,
                nth_match: 0,
                flags: parts[2].clone(),
                address: None,
                container: None,
                brace: None,
                brace_spans: Vec::new(),
                table_create: None,
                enriched: false,
            }
        }
        _ => return Err(SedmatError::new(format!("unsupported command: {raw}"))),
    };

    expr.address = address;
    expr.container = container;
    Ok(expr)
}

pub fn parse(raw: &str) -> Result<Program> {
    Ok(Program {
        expressions: vec![parse_expression(raw)?],
    })
}

pub fn enrich(mut program: Program) -> Program {
    for expr in &mut program.expressions {
        if expr.enriched || expr.command != Command::Substitute {
            continue;
        }
        expr.enriched = true;
        if has_brace_formatting(&expr.replacement) {
            let (cleaned, spans) = parse_brace_replacement(&expr.replacement);
            if !spans.is_empty() {
                expr.replacement = cleaned;
                if spans.len() == 1 && spans[0].is_global {
                    expr.brace = Some(spans[0].expr.clone());
                } else {
                    expr.brace = Some(merge_brace_spans(&spans));
                }
                expr.brace_spans = spans;
                if let Some(ref b) = expr.brace {
                    if let Some(ref tref) = b.table_ref {
                        if let Some(spec) = parse_table_create_dims(tref) {
                            expr.table_create = Some(spec);
                            expr.brace = None;
                            expr.brace_spans.clear();
                        }
                    }
                }
            }
        }
    }
    program
}

fn parse_table_create_dims(spec: &str) -> Option<TableCreateSpec> {
    let header = spec.ends_with(":header");
    let core = spec.strip_suffix(":header").unwrap_or(spec);
    let (r, c) = core.split_once('x').or_else(|| core.split_once('X'))?;
    let rows: usize = r.parse().ok()?;
    let columns: usize = c.parse().ok()?;
    if rows < 1 || columns < 1 {
        return None;
    }
    Some(TableCreateSpec {
        rows,
        columns,
        header,
        cells: Vec::new(),
    })
}

pub fn parse_program(script: &str) -> Result<Program> {
    let mut expressions = Vec::new();
    for (idx, line) in script.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let expr = parse_expression(trimmed)
            .map_err(|e| SedmatError::new(format!("line {}: {}", idx + 1, e)))?;
        expressions.push(expr);
    }
    Ok(enrich(Program { expressions }))
}
