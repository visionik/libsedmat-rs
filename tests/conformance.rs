use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use libsedmat::{
    enrich, format_numbered, format_structure, parse, parse_program, select_segment, Backend,
    ExecuteOptions, Executor, MemoryDocument,
};

// --- Minimal zero-dependency JSON parser for loading testdata/conformance/v3.6.json ---

#[derive(Debug, Clone)]
enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    fn as_obj(&self) -> &BTreeMap<String, Json> {
        match self {
            Json::Obj(m) => m,
            _ => panic!("expected json object"),
        }
    }
    fn as_arr(&self) -> &[Json] {
        match self {
            Json::Arr(a) => a,
            _ => panic!("expected json array"),
        }
    }
    fn as_str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            _ => panic!("expected json string"),
        }
    }
    fn as_bool(&self) -> bool {
        match self {
            Json::Bool(b) => *b,
            _ => panic!("expected json bool"),
        }
    }
    fn as_i64(&self) -> i64 {
        match self {
            Json::Num(n) => *n,
            _ => panic!("expected json num"),
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(s: &'a str) -> Self {
        Self { bytes: s.as_bytes(), pos: 0 }
    }
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn parse_val(&mut self) -> Json {
        self.skip_ws();
        match self.bytes[self.pos] {
            b'{' => self.parse_obj(),
            b'[' => self.parse_arr(),
            b'"' => Json::Str(self.parse_str()),
            b't' => { self.pos += 4; Json::Bool(true) }
            b'f' => { self.pos += 5; Json::Bool(false) }
            b'n' => { self.pos += 4; Json::Null }
            _ => self.parse_num(),
        }
    }
    fn parse_obj(&mut self) -> Json {
        self.pos += 1;
        let mut map = BTreeMap::new();
        loop {
            self.skip_ws();
            if self.bytes[self.pos] == b'}' {
                self.pos += 1;
                break;
            }
            let key = self.parse_str();
            self.skip_ws();
            assert_eq!(self.bytes[self.pos], b':');
            self.pos += 1;
            let val = self.parse_val();
            map.insert(key, val);
            self.skip_ws();
            if self.bytes[self.pos] == b',' {
                self.pos += 1;
            }
        }
        Json::Obj(map)
    }
    fn parse_arr(&mut self) -> Json {
        self.pos += 1;
        let mut arr = Vec::new();
        loop {
            self.skip_ws();
            if self.bytes[self.pos] == b']' {
                self.pos += 1;
                break;
            }
            arr.push(self.parse_val());
            self.skip_ws();
            if self.bytes[self.pos] == b',' {
                self.pos += 1;
            }
        }
        Json::Arr(arr)
    }
    fn parse_str(&mut self) -> String {
        assert_eq!(self.bytes[self.pos], b'"');
        self.pos += 1;
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            if b == b'"' {
                break;
            }
            if b == b'\\' {
                let esc = self.bytes[self.pos];
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    other => out.push(other as char),
                }
            } else {
                out.push(b as char);
            }
        }
        out
    }
    fn parse_num(&mut self) -> Json {
        let start = self.pos;
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        Json::Num(s.parse().unwrap())
    }
}

#[test]
fn test_conformance_v36_json() {
    let raw = fs::read_to_string("testdata/conformance/v3.6.json").unwrap();
    let root = JsonParser::new(&raw).parse_val();
    let obj = root.as_obj();

    // Parse vectors
    for v in obj["parse_vectors"].as_arr() {
        let tc = v.as_obj();
        let name = tc["name"].as_str();
        let input = tc["input"].as_str();
        let want_err = tc.get("want_error").map(|b| b.as_bool()).unwrap_or(false);

        let res = parse(input).map(enrich);
        if want_err {
            assert!(res.is_err(), "parse/{name}: expected error for {input:?}, got {res:?}");
            continue;
        }
        let prog = res.unwrap_or_else(|e| panic!("parse/{name}: unexpected error: {e}"));
        let expr = &prog.expressions[0];

        if let Some(cmd) = tc.get("command") {
            assert_eq!(expr.command.as_str(), cmd.as_str(), "parse/{name}: command");
        }
        if let Some(pat) = tc.get("pattern") {
            assert_eq!(expr.pattern, pat.as_str(), "parse/{name}: pattern");
        }
        if let Some(repl) = tc.get("replacement") {
            assert_eq!(expr.replacement, repl.as_str(), "parse/{name}: replacement");
        }
        if let Some(g) = tc.get("global") {
            assert_eq!(expr.global, g.as_bool(), "parse/{name}: global");
        }
        if let Some(nth) = tc.get("nth") {
            assert_eq!(expr.nth_match, nth.as_i64() as usize, "parse/{name}: nth");
        }
        if let Some(addr_j) = tc.get("address") {
            let a = addr_j.as_obj();
            let got = expr.address.as_ref().unwrap_or_else(|| panic!("parse/{name}: missing address"));
            assert_eq!(got.start, a["start"].as_i64() as i32, "parse/{name}: addr.start");
            assert_eq!(got.end, a["end"].as_i64() as i32, "parse/{name}: addr.end");
            assert_eq!(got.has_range, a["has_range"].as_bool(), "parse/{name}: addr.has_range");
        }
        if let Some(sc) = tc.get("expect_span_count") {
            assert_eq!(expr.brace_spans.len(), sc.as_i64() as usize, "parse/{name}: span_count");
        }
        if let Some(tc_j) = tc.get("expect_table_create") {
            let t = tc_j.as_obj();
            let got = expr.table_create.as_ref().unwrap_or_else(|| panic!("parse/{name}: missing table_create"));
            assert_eq!(got.rows, t["rows"].as_i64() as usize, "parse/{name}: rows");
            assert_eq!(got.columns, t["columns"].as_i64() as usize, "parse/{name}: cols");
        }
        if let Some(eb_j) = tc.get("expect_brace") {
            let eb = eb_j.as_obj();
            let b = expr.brace.as_ref().unwrap_or_else(|| panic!("parse/{name}: missing brace"));
            if let Some(v) = eb.get("bold") { assert_eq!(b.bold, Some(v.as_bool()), "parse/{name}: bold"); }
            if let Some(v) = eb.get("color") {
                assert_eq!(b.color.as_deref().map(str::to_uppercase), Some(v.as_str().to_uppercase()), "parse/{name}: color");
            }
            if let Some(v) = eb.get("reset") { assert_eq!(b.reset, v.as_bool(), "parse/{name}: reset"); }
            if let Some(v) = eb.get("no_reset") { assert_eq!(b.no_reset, v.as_bool(), "parse/{name}: no_reset"); }
            if let Some(v) = eb.get("has_break") { assert_eq!(b.has_break, v.as_bool(), "parse/{name}: has_break"); }
            if let Some(v) = eb.get("break") { assert_eq!(b.break_kind, v.as_str(), "parse/{name}: break"); }
            if let Some(v) = eb.get("cols") { assert_eq!(b.cols, Some(v.as_i64() as i32), "parse/{name}: cols"); }
            if let Some(v) = eb.get("check_set") { if v.as_bool() { assert!(b.check.is_some(), "parse/{name}: check"); } }
            if let Some(v) = eb.get("toc") { assert_eq!(b.toc, v.as_i64() as i32, "parse/{name}: toc"); }
            if let Some(v) = eb.get("comment") { assert_eq!(b.comment.as_deref(), Some(v.as_str()), "parse/{name}: comment"); }
            if let Some(v) = eb.get("bookmark") { assert_eq!(b.bookmark.as_deref(), Some(v.as_str()), "parse/{name}: bookmark"); }
            if let Some(v) = eb.get("url") { assert_eq!(b.url.as_deref(), Some(v.as_str()), "parse/{name}: url"); }
            if let Some(v) = eb.get("img_ref") { assert_eq!(b.img_ref.as_deref(), Some(v.as_str()), "parse/{name}: img_ref"); }
        }
    }

    // Execution vectors
    for v in obj["execution_vectors"].as_arr() {
        let tc = v.as_obj();
        let name = tc["name"].as_str();
        let paras: Vec<&str> = tc["paragraphs"].as_arr().iter().map(|p| p.as_str()).collect();
        let script = tc["script"].as_str();
        let want_err = tc.get("want_error").map(|b| b.as_bool()).unwrap_or(false);

        let mut doc = MemoryDocument::new(&paras);
        let res = Executor::new(&mut doc).execute_string(script, &ExecuteOptions::default());
        if want_err {
            assert!(res.is_err(), "exec/{name}: expected error");
            continue;
        }
        res.unwrap_or_else(|e| panic!("exec/{name}: unexpected error: {e}"));
        let expected: Vec<String> = tc["expected_paragraphs"]
            .as_arr()
            .iter()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(doc.paragraph_texts(""), expected, "exec/{name}");
    }
}

#[test]
fn test_ported_sed_fixtures_parse_cleanly() {
    let dir = Path::new("testdata/fixtures");
    let mut count = 0;
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("sed") {
            let content = fs::read_to_string(&path).unwrap();
            let prog = parse_program(&content)
                .unwrap_or_else(|e| panic!("fixture {:?} failed: {e}", path.file_name()));
            assert!(!prog.expressions.is_empty());
            count += 1;
        }
    }
    assert!(count >= 9, "expected at least 9 .sed fixtures, got {count}");
}

#[test]
fn test_v37_container_scopes_and_introspection() {
    let mut doc = MemoryDocument::new(&["Slide 1 Title", "Draft subtitle"]);
    doc.tabs[0].id = "slide-1".to_string();
    doc.tabs[0].title = "Intro".to_string();
    doc.add_tab("slide-2", "Metrics", &["Latency is 500ms", "Keep line"]);
    doc.add_tab("slide-3", "Q4 Budget", &["Total in USD"]);

    let mut exec = Executor::new(&mut doc);
    exec.execute_string(
        r#"
@1:{z=#0F172A transition=fade}
@2:s/500ms/{b c=green}120ms/g
@2:{notes="Highlight 120ms WebTransport benchmark"}
@"Q4 Budget":s/USD/EUR/g
@$a/Next Steps/
@4{hide}
"#,
        &ExecuteOptions::default(),
    )
    .unwrap();

    assert_eq!(doc.tabs[0].background.as_deref(), Some("#0F172A"));
    assert_eq!(doc.tabs[0].transition.as_deref(), Some("fade"));
    assert_eq!(doc.paragraph_texts("slide-2"), vec!["Latency is 120ms", "Keep line"]);
    assert_eq!(doc.tabs[1].notes.as_deref(), Some("Highlight 120ms WebTransport benchmark"));
    assert_eq!(doc.paragraph_texts("Q4 Budget"), vec!["Total in EUR"]);
    assert_eq!(doc.tabs.len(), 4);
    assert!(doc.tabs[3].hidden);

    let proj = doc.project().unwrap();
    let seg = select_segment(&proj, "slide-2").unwrap();
    assert!(format_structure(seg).contains("[1] paragraph (NORMAL_TEXT): \"Latency is 120ms\""));
    assert!(format_numbered(seg).contains("[2] Keep line"));
}
