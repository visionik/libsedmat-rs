//! Context-aware IntelliSense / IntelliCode completion, live grammar signatures,
//! structured diagnostics, and lightweight syntax tokenization for SEDMAT v3.6 / v3.7.
//!
//! UI-agnostic and zero-dependency: usable from web editor bars, TUIs, CLIs, and LSPs.

use std::collections::HashMap;

/// Category of an IntelliSense completion item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    Scope,
    Command,
    Flag,
    Value,
    Snippet,
}

/// Target capability profile used to filter SEDMAT IntelliSense completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Document,
    Word,
    Slides,
    Spreadsheet,
    Vector,
    Timeline,
    All,
}

impl Profile {
    fn normalized(self) -> Profile {
        match self {
            Profile::Word => Profile::Document,
            other => other,
        }
    }
}

/// Single IntelliSense completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub insert_text: String,
    pub replace_from: usize,
    pub replace_to: usize,
    pub detail: String,
    pub kind: SuggestionKind,
    /// Optional `#RRGGBB` color swatch for color value completions.
    pub swatch: Option<String>,
}

/// Segment of a live grammar signature hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignaturePart {
    pub text: String,
    pub active: bool,
}

/// Full result returned by [`get_completions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntelliSenseResult {
    pub context_title: String,
    pub signature: Vec<SignaturePart>,
    pub suggestions: Vec<Suggestion>,
    /// Dimmed inline ghost text suffix when cursor is at the end of input.
    pub ghost_suffix: Option<String>,
}

/// Custom or built-in brace flag definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagDefinition {
    pub key: String,
    pub insert: String,
    pub detail: String,
    pub profiles: Vec<Profile>,
}

/// Custom or built-in value completion option for a `key=value` flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueOption {
    pub value: String,
    pub detail: String,
    pub swatch: Option<String>,
}

/// Custom or built-in snippet completion option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetOption {
    pub label: String,
    pub insert: String,
    pub detail: String,
    pub kind: SuggestionKind,
    pub swatch: Option<String>,
}

/// Optional host-application extensions merged into IntelliSense completions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IntelliSenseExtensions {
    pub flags: Vec<FlagDefinition>,
    pub values: HashMap<String, Vec<ValueOption>>,
    pub starters: Vec<SnippetOption>,
    pub scope_snippets: Vec<SnippetOption>,
}

/// Host context passed to [`get_completions`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextInfo {
    pub mode: Profile,
    /// 1-based active container index (slide, page, sheet, tab).
    pub active_container: Option<usize>,
    pub total_containers: Option<usize>,
    /// Alias for `active_container`.
    pub active_slide: Option<usize>,
    /// Alias for `total_containers`.
    pub total_slides: Option<usize>,
    pub extensions: Option<IntelliSenseExtensions>,
}

/// Structured diagnostic extracted from any SEDMAT error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub ok: bool,
    pub summary: String,
    pub error_offset: Option<usize>,
    pub error_length: usize,
    pub hint: Option<String>,
}

/// Syntax token category produced by [`tokenize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Comment,
    Scope,
    Address,
    Command,
    Delimiter,
    Pattern,
    Brace,
    Text,
    Flag,
    Whitespace,
}

/// Single syntax token emitted by [`tokenize`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// Extract structured error offset, span length, clean reason, and hint from any SEDMAT error message.
pub fn extract_diagnostic(err: &str) -> Diagnostic {
    let lower = err.to_ascii_lowercase();
    if let Some(pos) = lower.find("at offset ") {
        let after = &err[pos + "at offset ".len()..];
        if let Some(colon) = after.find(':') {
            let num_str = after[..colon].trim();
            if let Ok(offset) = num_str.parse::<usize>() {
                let rest = after[colon + 1..].trim();
                if let Some((summary, hint)) = rest.split_once(" — ") {
                    return Diagnostic {
                        ok: false,
                        summary: summary.trim().to_string(),
                        error_offset: Some(offset),
                        error_length: 1,
                        hint: Some(hint.trim().to_string()),
                    };
                }
                return Diagnostic {
                    ok: false,
                    summary: rest.to_string(),
                    error_offset: Some(offset),
                    error_length: 1,
                    hint: None,
                };
            }
        }
    }
    Diagnostic {
        ok: false,
        summary: err.to_string(),
        error_offset: None,
        error_length: 0,
        hint: None,
    }
}

fn flag_def(key: &str, insert: &str, detail: &str, profiles: &[Profile]) -> FlagDefinition {
    FlagDefinition {
        key: key.to_string(),
        insert: insert.to_string(),
        detail: detail.to_string(),
        profiles: profiles.to_vec(),
    }
}

fn builtin_flags() -> Vec<FlagDefinition> {
    use Profile::*;
    vec![
        // Timeline directives
        flag_def("+2d", "+2d", "Extend task duration by +2 working days", &[Timeline]),
        flag_def("-1d", "-1d", "Shorten task duration by -1 working day", &[Timeline]),
        flag_def("5d", "5d", "Set exact task duration to 5 working days", &[Timeline]),
        flag_def("+4h", "+4h", "Extend task duration by +4 working hours", &[Timeline]),
        flag_def("50%", "50%", "Set task completion to 50%", &[Timeline]),
        flag_def("100%", "100%", "Mark task 100% complete", &[Timeline]),
        flag_def("milestone", "milestone", "Convert task into a 0-day milestone", &[Timeline]),
        // Core inline formatting
        flag_def("b", "b", "Bold", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("i", "i", "Italic", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("_", "_", "Underline", &[Document, Slides, Spreadsheet]),
        flag_def("-", "-", "Strikethrough", &[Document, Slides, Spreadsheet]),
        flag_def("c=", "c=", "Text / foreground color (red, blue, #FF0000…)", &[]),
        flag_def("z=", "z=", "Background / highlight / fill color", &[]),
        flag_def("s=", "s=", "Font size in pt (e.g. s=14)", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("f=", "f=", "Font family (e.g. f=Inter)", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("a=", "a=", "Alignment (left, center, right, justify)", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("t=", "t=", "Replacement text while styling match", &[Document, Slides, Vector]),
        flag_def("u=", "u=", "Hyperlink URL, #bookmark, or chip:// URI", &[Document, Slides, Spreadsheet]),
        flag_def("#", "#", "Inline monospace code", &[Document, Slides]),
        flag_def("w", "w", "Small caps", &[Document, Slides]),
        flag_def("^", "^", "Superscript", &[Document, Slides]),
        flag_def(",", ",", "Subscript", &[Document, Slides]),
        flag_def("!b", "!b", "Remove bold (negate any boolean flag with !)", &[Document, Slides, Vector]),
        flag_def("!0", "!0", "Additive mode (opt out of implicit reset)", &[Document, Slides, Spreadsheet, Vector]),
        flag_def("0", "0", "Reset all formatting", &[Document, Slides, Spreadsheet, Vector]),
        // Slides, Vector & Shape flags (v3.7)
        flag_def("shape=", "shape=", "Shape geometry (roundRect, rect, diamond, ellipse…)", &[Slides, Vector]),
        flag_def("fill=", "fill=", "Shape fill hex or named color", &[Slides, Vector, Timeline]),
        flag_def("stroke=", "stroke=", "Shape border hex or named color", &[Slides, Vector]),
        flag_def("layout=", "layout=", "Slide layout (title, content, two-col, section, blank)", &[Slides]),
        flag_def("zone=", "zone=", "Placement zone (left, right, top, bottom, center, footer)", &[Slides]),
        flag_def("transition=", "transition=", "Slide transition (fade, push, wipe, none)", &[Slides]),
        flag_def("move=", "move=", "Move container to 1-based position (e.g. @3{move=1})", &[Slides, Spreadsheet]),
        flag_def("dup", "dup", "Duplicate container (e.g. @2{dup})", &[Slides, Vector, Spreadsheet]),
        flag_def("hide", "hide", "Hide container", &[Slides, Spreadsheet]),
        // Document-specific structural flags
        flag_def("h=", "h=", "Heading level (t=Title, s=Subtitle, 1..6, 0=Normal)", &[Document]),
        flag_def("p=", "p=", "Paragraph spacing before,after pt (e.g. p=12,6)", &[Document]),
        flag_def("n=", "n=", "Paragraph left indent level", &[Document]),
        flag_def("cols=", "cols=", "Section columns (e.g. cols=2)", &[Document]),
        flag_def("\"=", "\"=", "Attach review comment (e.g. \"=Check figure)", &[Document]),
        flag_def("@=", "@=", "Insert bookmark anchor (e.g. @=summary)", &[Document]),
        flag_def("check", "check", "Insert checkbox (check or check=y)", &[Document]),
        flag_def("toc", "toc", "Insert Table of Contents", &[Document]),
        flag_def("+=p", "+=p", "Insert page break (+, +=p, +=c, +=s)", &[Document]),
        // Tables & images
        flag_def("T=", "T=", "Table create/cell ref (T=3x4:header or T=1!A1)", &[Document, Slides]),
        flag_def("img=", "img=", "Target image by 1-based index (img=1, img=-1)", &[Document, Slides]),
        flag_def("x=", "x=", "Image width in px (e.g. x=420)", &[Document, Slides]),
    ]
}

fn val_opt(value: &str, detail: &str, swatch: Option<&str>) -> ValueOption {
    ValueOption {
        value: value.to_string(),
        detail: detail.to_string(),
        swatch: swatch.map(str::to_string),
    }
}

fn color_values() -> Vec<ValueOption> {
    // Canonical sedmat.org 18-color palette swatches
    vec![
        val_opt("red", "Canonical red", Some("#FF0000")),
        val_opt("blue", "Canonical blue", Some("#0000FF")),
        val_opt("green", "Canonical green", Some("#00FF00")),
        val_opt("orange", "Canonical orange", Some("#FF8C00")),
        val_opt("purple", "Canonical purple", Some("#800080")),
        val_opt("navy", "Canonical navy", Some("#000080")),
        val_opt("teal", "Canonical teal", Some("#008080")),
        val_opt("gray", "Canonical gray", Some("#808080")),
        val_opt("yellow", "Highlight yellow", Some("#FFFF00")),
        val_opt("black", "Canonical black", Some("#000000")),
        val_opt("white", "Canonical white", Some("#FFFFFF")),
    ]
}

fn builtin_values(key: &str) -> Option<Vec<ValueOption>> {
    match key {
        "c" | "z" | "fill" | "stroke" => Some(color_values()),
        "a" => Some(vec![
            val_opt("left", "Align left", None),
            val_opt("center", "Align center", None),
            val_opt("right", "Align right", None),
            val_opt("justify", "Justify paragraph", None),
        ]),
        "h" => Some(vec![
            val_opt("t", "Document Title", None),
            val_opt("s", "Subtitle", None),
            val_opt("1", "Heading 1", None),
            val_opt("2", "Heading 2", None),
            val_opt("3", "Heading 3", None),
            val_opt("0", "Normal paragraph", None),
        ]),
        "layout" => Some(vec![
            val_opt("content", "Title + bullet body", None),
            val_opt("title", "Centered cover title", None),
            val_opt("two-col", "Two-column comparison", None),
            val_opt("section", "Section divider header", None),
            val_opt("comparison", "Side-by-side comparison", None),
            val_opt("blank", "Blank canvas", None),
        ]),
        "zone" => Some(vec![
            val_opt("center", "Center stage", None),
            val_opt("left", "Left column", None),
            val_opt("right", "Right column", None),
            val_opt("top", "Upper band", None),
            val_opt("bottom", "Lower band", None),
            val_opt("footer", "Bottom footer", None),
        ]),
        "shape" => Some(vec![
            val_opt("roundRect", "Rounded rectangle", None),
            val_opt("rect", "Rectangle", None),
            val_opt("ellipse", "Circle / oval", None),
            val_opt("diamond", "Decision diamond", None),
            val_opt("rightArrow", "Right arrow", None),
            val_opt("chevron", "Process chevron", None),
        ]),
        "transition" => Some(vec![
            val_opt("fade", "Smooth crossfade", None),
            val_opt("push", "Slide push", None),
            val_opt("wipe", "Directional wipe", None),
            val_opt("none", "No transition", None),
        ]),
        "f" => Some(vec![
            val_opt("Arial", "Classic sans-serif", None),
            val_opt("Calibri", "Office sans-serif", None),
            val_opt("Georgia", "Editorial serif", None),
            val_opt("Inter", "Modern UI sans", None),
            val_opt("Consolas", "Monospace code", None),
        ]),
        "s" => Some(vec![
            val_opt("11", "11 pt default", None),
            val_opt("12", "12 pt body", None),
            val_opt("16", "16 pt lead", None),
            val_opt("24", "24 pt heading", None),
            val_opt("36", "36 pt title", None),
        ]),
        _ => None,
    }
}

fn matches_profile(def: &FlagDefinition, profile: Profile) -> bool {
    if profile == Profile::All || def.profiles.is_empty() {
        return true;
    }
    def.profiles.iter().any(|p| p.normalized() == profile)
}

fn find_open_brace(before: &str) -> Option<usize> {
    let bytes = before.as_bytes();
    let mut depth = 0usize;
    let mut last_open = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'{' {
            depth += 1;
            last_open = Some(i);
        } else if bytes[i] == b'}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                last_open = None;
            }
        }
        i += 1;
    }
    if depth > 0 {
        last_open
    } else {
        None
    }
}

fn count_unescaped_slashes(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'/' {
            count += 1;
        }
        i += 1;
    }
    count
}

fn compute_ghost_suffix(
    input: &str,
    cursor: usize,
    suggestions: &[Suggestion],
) -> Option<String> {
    if cursor != input.len() || suggestions.is_empty() {
        return None;
    }
    let top = &suggestions[0];
    let typed = input.get(top.replace_from..top.replace_to)?;
    if top.insert_text.len() > typed.len()
        && top
            .insert_text
            .to_ascii_lowercase()
            .starts_with(&typed.to_ascii_lowercase())
    {
        return Some(top.insert_text[typed.len()..].to_string());
    }
    None
}

fn find_sub_cmd_end(seg: &str) -> Option<usize> {
    let bytes = seg.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b's' && bytes[i + 1] == b'/' {
            let valid_prev = i == 0 || {
                let p = bytes[i - 1];
                p == b':' || p.is_ascii_digit() || p == b'$' || p.is_ascii_whitespace()
            };
            if valid_prev {
                return Some(i + 2);
            }
        }
        i += 1;
    }
    None
}

fn sig(text: &str, active: bool) -> SignaturePart {
    SignaturePart {
        text: text.to_string(),
        active,
    }
}

fn snip(label: impl Into<String>, insert: impl Into<String>, detail: impl Into<String>, kind: SuggestionKind) -> SnippetOption {
    SnippetOption {
        label: label.into(),
        insert: insert.into(),
        detail: detail.into(),
        kind,
        swatch: None,
    }
}

/// Produce live IntelliSense suggestions, grammar parameter hints, and ghost completion
/// for SEDMAT v3.6 / v3.7.
pub fn get_completions(input: &str, cursor: usize, ctx: &ContextInfo) -> IntelliSenseResult {
    let profile = ctx.mode.normalized();
    let mut safe_cursor = cursor.min(input.len());
    while safe_cursor > 0 && !input.is_char_boundary(safe_cursor) {
        safe_cursor -= 1;
    }
    let before = &input[..safe_cursor];
    let container_num = ctx.active_container.or(ctx.active_slide).unwrap_or(1);

    let seg_start = before.rfind(';').map(|i| i + 1).unwrap_or(0);
    let raw_seg = &before[seg_start..];
    let trimmed_seg = raw_seg.trim_start();
    let cmd_start = seg_start + (raw_seg.len() - trimmed_seg.len());
    let seg = &before[cmd_start..];

    // 1. Inside `{...}` formatting / operation braces
    if let Some(brace_idx) = find_open_brace(before) {
        if brace_idx >= cmd_start {
            let inside = &before[brace_idx + 1..];
            let last_space = inside.rfind([' ', '\t']);
            let token_start = match last_space {
                Some(idx) => brace_idx + 1 + idx + 1,
                None => brace_idx + 1,
            };
            let token = &before[token_start..];

            if let Some(eq_idx) = token.find('=') {
                if eq_idx > 0 {
                    let key = &token[..eq_idx];
                    let val_typed = token[eq_idx + 1..].to_ascii_lowercase();
                    let val_start = token_start + eq_idx + 1;

                    let value_list = ctx
                        .extensions
                        .as_ref()
                        .and_then(|ext| ext.values.get(key).cloned())
                        .or_else(|| builtin_values(key));

                    if let Some(values) = value_list {
                        let filtered: Vec<Suggestion> = values
                            .into_iter()
                            .filter(|v| {
                                val_typed.is_empty()
                                    || v.value.to_ascii_lowercase().contains(&val_typed)
                                    || v.detail.to_ascii_lowercase().contains(&val_typed)
                            })
                            .map(|v| Suggestion {
                                label: v.value.clone(),
                                insert_text: v.value,
                                replace_from: val_start,
                                replace_to: safe_cursor,
                                detail: v.detail,
                                kind: SuggestionKind::Value,
                                swatch: v.swatch,
                            })
                            .collect();
                        let ghost = compute_ghost_suffix(input, safe_cursor, &filtered);
                        return IntelliSenseResult {
                            context_title: format!("Value for {{{key}=…}}"),
                            signature: vec![
                                sig("{ ", false),
                                sig(&format!("{key}="), false),
                                sig("value", true),
                                sig(" }", false),
                            ],
                            suggestions: filtered,
                            ghost_suffix: ghost,
                        };
                    }
                }
            }

            let q = token.to_ascii_lowercase();
            let mut all_flags = Vec::new();
            if let Some(ext) = &ctx.extensions {
                all_flags.extend(ext.flags.iter().cloned());
            }
            all_flags.extend(builtin_flags());

            let suggestions: Vec<Suggestion> = all_flags
                .into_iter()
                .filter(|f| matches_profile(f, profile))
                .filter(|f| {
                    q.is_empty()
                        || f.key.to_ascii_lowercase().starts_with(&q)
                        || f.detail.to_ascii_lowercase().contains(&q)
                })
                .take(12)
                .map(|f| Suggestion {
                    label: f.key,
                    insert_text: f.insert,
                    replace_from: token_start,
                    replace_to: safe_cursor,
                    detail: f.detail,
                    kind: SuggestionKind::Flag,
                    swatch: None,
                })
                .collect();

            let ghost = compute_ghost_suffix(input, safe_cursor, &suggestions);
            return IntelliSenseResult {
                context_title: "SEDMAT Brace Flags ({…})".to_string(),
                signature: vec![
                    sig("{", false),
                    sig("b i _ - c= z= s= f= a= !0 …", true),
                    sig("}", false),
                ],
                suggestions,
                ghost_suffix: ghost,
            };
        }
    }

    // 2. Typing a `@` container scope prefix
    if seg.starts_with('@') {
        let first_colon = seg.find(':');
        if first_colon.is_none() {
            let q = seg.to_ascii_lowercase();
            let base_scopes: Vec<SnippetOption> = match profile {
                Profile::Slides => vec![
                    snip(format!("@{container_num}:"), format!("@{container_num}:"), format!("Target slide {container_num}"), SuggestionKind::Scope),
                    snip(format!("@{container_num}:title:"), format!("@{container_num}:title:"), format!("Target slide {container_num} title"), SuggestionKind::Scope),
                    snip(format!("@{container_num}:body:"), format!("@{container_num}:body:"), format!("Target slide {container_num} body"), SuggestionKind::Scope),
                    snip(format!("@{container_num}:notes:"), format!("@{container_num}:notes:"), format!("Target slide {container_num} speaker notes"), SuggestionKind::Scope),
                    snip("@*:", "@*:", "Target ALL slides", SuggestionKind::Scope),
                    snip(format!("@{container_num}:{{z=#0F172A}}"), format!("@{container_num}:{{z=#0F172A}}"), "Set slide background", SuggestionKind::Snippet),
                    snip("@$a/{layout=two-col}New Slide/", "@$a/{layout=two-col}New Slide/", "Append new slide", SuggestionKind::Snippet),
                    snip(format!("@{container_num}{{dup}}"), format!("@{container_num}{{dup}}"), format!("Duplicate slide {container_num}"), SuggestionKind::Command),
                    snip(format!("@{container_num}{{move=1}}"), format!("@{container_num}{{move=1}}"), format!("Move slide {container_num} to position 1"), SuggestionKind::Command),
                    snip(format!("@{container_num}d"), format!("@{container_num}d"), format!("Delete slide {container_num}"), SuggestionKind::Command),
                ],
                Profile::Spreadsheet => vec![
                    snip(format!("@{container_num}:"), format!("@{container_num}:"), format!("Target sheet {container_num}"), SuggestionKind::Scope),
                    snip("@sheet=", "@sheet=", "Target named worksheet", SuggestionKind::Scope),
                    snip("@*:", "@*:", "Target ALL sheets", SuggestionKind::Scope),
                ],
                _ => vec![
                    snip("@header:", "@header:", "Target document header", SuggestionKind::Scope),
                    snip("@footer:", "@footer:", "Target document footer", SuggestionKind::Scope),
                    snip(format!("@{container_num}:"), format!("@{container_num}:"), format!("Target container {container_num}"), SuggestionKind::Scope),
                    snip("@*:", "@*:", "Target all containers", SuggestionKind::Scope),
                ],
            };

            let mut scope_snippets = Vec::new();
            if let Some(ext) = &ctx.extensions {
                scope_snippets.extend(ext.scope_snippets.iter().cloned());
            }
            scope_snippets.extend(base_scopes);

            let filtered: Vec<Suggestion> = scope_snippets
                .into_iter()
                .filter(|s| {
                    s.label.to_ascii_lowercase().contains(&q)
                        || s.detail.to_ascii_lowercase().contains(&q)
                })
                .map(|s| Suggestion {
                    label: s.label,
                    insert_text: s.insert,
                    replace_from: cmd_start,
                    replace_to: safe_cursor,
                    detail: s.detail,
                    kind: s.kind,
                    swatch: s.swatch,
                })
                .collect();

            let ghost = compute_ghost_suffix(input, safe_cursor, &filtered);
            return IntelliSenseResult {
                context_title: "Container Scope (@)".to_string(),
                signature: vec![sig("@container", true), sig("[:region]:command", false)],
                suggestions: filtered,
                ghost_suffix: ghost,
            };
        }

        let fc = first_colon.unwrap();
        let after_first = &seg[fc + 1..];
        if !after_first.contains(':') && !after_first.contains('/') && !after_first.starts_with('{')
        {
            let token_start = cmd_start + fc + 1;
            let q = after_first.to_ascii_lowercase();
            let post_scope: Vec<SnippetOption> = if profile == Profile::Slides {
                vec![
                    snip("title:", "title:", "Slide title placeholder", SuggestionKind::Scope),
                    snip("sub:", "sub:", "Slide subtitle placeholder", SuggestionKind::Scope),
                    snip("body:", "body:", "Slide body / bullets", SuggestionKind::Scope),
                    snip("notes:", "notes:", "Speaker notes", SuggestionKind::Scope),
                    snip("s/old/new/g", "s/", "Search & replace text or style", SuggestionKind::Command),
                    snip("a/text/", "a/", "Append bullet / paragraph", SuggestionKind::Command),
                    snip("d/pattern/", "d/", "Delete matching lines", SuggestionKind::Command),
                    snip("{z=#0F172A}", "{z=", "Set slide background color", SuggestionKind::Flag),
                    snip("{transition=fade}", "{transition=", "Set slide transition", SuggestionKind::Flag),
                ]
            } else {
                vec![
                    snip("s/old/new/g", "s/", "Search & replace in container", SuggestionKind::Command),
                    snip("a/text/", "a/", "Append paragraph", SuggestionKind::Command),
                    snip("d/pattern/", "d/", "Delete matching paragraphs", SuggestionKind::Command),
                ]
            };

            let filtered: Vec<Suggestion> = post_scope
                .into_iter()
                .filter(|item| {
                    q.is_empty()
                        || item.label.to_ascii_lowercase().starts_with(&q)
                        || item.detail.to_ascii_lowercase().contains(&q)
                })
                .map(|item| Suggestion {
                    label: item.label,
                    insert_text: item.insert,
                    replace_from: token_start,
                    replace_to: safe_cursor,
                    detail: item.detail,
                    kind: item.kind,
                    swatch: item.swatch,
                })
                .collect();

            let prefix = &seg[..fc + 1];
            let ghost = compute_ghost_suffix(input, safe_cursor, &filtered);
            return IntelliSenseResult {
                context_title: format!("Inside {prefix}"),
                signature: vec![
                    sig(prefix, false),
                    sig("region: | command | {flags}", true),
                ],
                suggestions: filtered,
                ghost_suffix: ghost,
            };
        }
    }

    // 3. Inside an active `s/.../.../...` substitution command
    if let Some(after_s_idx) = find_sub_cmd_end(seg) {
        let after_s = &seg[after_s_idx..];
        let slashes = count_unescaped_slashes(after_s);
        if slashes == 0 {
            return IntelliSenseResult {
                context_title: "Substitution — Pattern (RegExp)".to_string(),
                signature: vec![
                    sig("s/", false),
                    sig("pattern", true),
                    sig("/replacement/[g i m N]", false),
                ],
                suggestions: vec![Suggestion {
                    label: ".*/{b}/".to_string(),
                    insert_text: ".*/{b}/".to_string(),
                    replace_from: safe_cursor,
                    replace_to: safe_cursor,
                    detail: "Match entire element and apply formatting".to_string(),
                    kind: SuggestionKind::Snippet,
                    swatch: None,
                }],
                ghost_suffix: None,
            };
        }
        if slashes == 1 {
            return IntelliSenseResult {
                context_title: "Substitution — Replacement (type { for formatting flags)"
                    .to_string(),
                signature: vec![
                    sig("s/pattern/", false),
                    sig("{flags}replacement", true),
                    sig("/[g i m N]", false),
                ],
                suggestions: vec![
                    Suggestion {
                        label: "{b c=red}".to_string(),
                        insert_text: "{b c=red}".to_string(),
                        replace_from: safe_cursor,
                        replace_to: safe_cursor,
                        detail: "Bold red highlight (keeps matched text)".to_string(),
                        kind: SuggestionKind::Snippet,
                        swatch: Some("#FF0000".to_string()),
                    },
                    Suggestion {
                        label: "{b}".to_string(),
                        insert_text: "{b}".to_string(),
                        replace_from: safe_cursor,
                        replace_to: safe_cursor,
                        detail: "Make match bold".to_string(),
                        kind: SuggestionKind::Flag,
                        swatch: None,
                    },
                    Suggestion {
                        label: "{z=yellow}".to_string(),
                        insert_text: "{z=yellow}".to_string(),
                        replace_from: safe_cursor,
                        replace_to: safe_cursor,
                        detail: "Highlight match yellow".to_string(),
                        kind: SuggestionKind::Flag,
                        swatch: Some("#FFFF00".to_string()),
                    },
                    Suggestion {
                        label: "{!0 b c=blue}".to_string(),
                        insert_text: "{!0 b c=blue}".to_string(),
                        replace_from: safe_cursor,
                        replace_to: safe_cursor,
                        detail: "Add bold blue without resetting existing style".to_string(),
                        kind: SuggestionKind::Snippet,
                        swatch: Some("#0000FF".to_string()),
                    },
                    Suggestion {
                        label: "$0".to_string(),
                        insert_text: "$0".to_string(),
                        replace_from: safe_cursor,
                        replace_to: safe_cursor,
                        detail: "Insert original matched text".to_string(),
                        kind: SuggestionKind::Value,
                        swatch: None,
                    },
                ],
                ghost_suffix: None,
            };
        }

        let trailing = after_s
            .rfind('/')
            .map(|i| &after_s[i + 1..])
            .unwrap_or("");
        let opts = [
            ("g", "Global — replace every match"),
            ("i", "Case-insensitive regex match"),
            ("m", "Multiline ^ and $ anchors"),
            ("2", "Replace only the 2nd match"),
        ];
        let trailing_opts: Vec<Suggestion> = opts
            .iter()
            .filter(|(f, _)| !trailing.contains(*f))
            .map(|(f, d)| Suggestion {
                label: (*f).to_string(),
                insert_text: (*f).to_string(),
                replace_from: safe_cursor,
                replace_to: safe_cursor,
                detail: (*d).to_string(),
                kind: SuggestionKind::Flag,
                swatch: None,
            })
            .collect();
        let ghost = compute_ghost_suffix(input, safe_cursor, &trailing_opts);
        return IntelliSenseResult {
            context_title: "Substitution — Trailing Flags".to_string(),
            signature: vec![
                sig("s/pattern/replacement/", false),
                sig("[g i m N]", true),
            ],
            suggestions: trailing_opts,
            ghost_suffix: ghost,
        };
    }

    // 4. Default / Start of expression
    let q = seg.to_ascii_lowercase();
    let base_starters: Vec<SnippetOption> = if profile == Profile::Slides {
        vec![
            snip(format!("@{container_num}:title:s/.*/New Title/"), format!("@{container_num}:title:s/.*/"), format!("Edit slide {container_num} title"), SuggestionKind::Snippet),
            snip("s/old/{b c=blue t=new}/g", "s/", format!("Search & replace on slide {container_num}"), SuggestionKind::Command),
            snip("@*:s/old/new/gi", "@*:s/", "Search & replace across ALL slides", SuggestionKind::Snippet),
            snip(format!("@{container_num}:notes:a/Talking point/"), format!("@{container_num}:notes:a/"), format!("Append speaker note on slide {container_num}"), SuggestionKind::Snippet),
            snip("@$a/{layout=two-col}New Slide/", "@$a/{layout=two-col}", "Append a new slide", SuggestionKind::Snippet),
        ]
    } else {
        vec![
            snip("s/old/new/g", "s/", "Search & replace across document", SuggestionKind::Command),
            snip("s/TODO/{b c=red}/g", "s/TODO/{b c=red}/g", "Highlight every TODO in bold red", SuggestionKind::Snippet),
            snip("1s/.*/{h=t}/", "1s/.*/{h=t}/", "Format element 1 as Title", SuggestionKind::Snippet),
            snip("$a/{h=2}Next Steps/", "$a/{h=2}", "Append a new Heading 2 at end", SuggestionKind::Snippet),
            snip("d/DRAFT/i", "d/", "Delete elements matching pattern", SuggestionKind::Command),
            snip("s/{T=1!A1}/{b}Header/", "s/{T=1!A1}/", "Edit cell A1 of table 1", SuggestionKind::Snippet),
            snip("@header:s/.*/CONFIDENTIAL/", "@header:s/", "Edit document header", SuggestionKind::Scope),
        ]
    };

    let mut starters = Vec::new();
    if let Some(ext) = &ctx.extensions {
        starters.extend(ext.starters.iter().cloned());
    }
    starters.extend(base_starters);

    let filtered: Vec<Suggestion> = starters
        .into_iter()
        .filter(|s| {
            q.is_empty()
                || s.label.to_ascii_lowercase().contains(&q)
                || s.detail.to_ascii_lowercase().contains(&q)
        })
        .map(|s| Suggestion {
            label: s.label,
            insert_text: s.insert,
            replace_from: cmd_start,
            replace_to: safe_cursor,
            detail: s.detail,
            kind: s.kind,
            swatch: s.swatch,
        })
        .collect();

    let ghost = compute_ghost_suffix(input, safe_cursor, &filtered);
    IntelliSenseResult {
        context_title: "SEDMAT v3.7".to_string(),
        signature: vec![
            sig("[@scope:][addr]", seg.is_empty()),
            sig("cmd/pattern/{flags}replacement/[g i m N]", false),
        ],
        suggestions: filtered,
        ghost_suffix: ghost,
    }
}

fn match_region_prefix(rem: &str) -> Option<usize> {
    let lower = rem.to_ascii_lowercase();
    for reg in [
        "title:",
        "subtitle:",
        "sub:",
        "body:",
        "notes:",
        "header:",
        "footer:",
    ] {
        if lower.starts_with(reg) {
            return Some(reg.len());
        }
    }
    if lower.starts_with("shape=") {
        let rest = &rem[6..];
        if let Some(colon) = rest.find(':') {
            let num = &rest[..colon];
            if !num.is_empty()
                && num.as_bytes()[0] != b'0'
                && num.bytes().all(|b| b.is_ascii_digit())
            {
                return Some(6 + colon + 1);
            }
        }
    }
    None
}

/// Lightweight, non-throwing syntax tokenizer for SEDMAT expressions.
/// Useful for syntax highlighting in web inputs, TUIs, CLIs, and LSP semantic tokens.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let push = |tokens: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize| {
        if end > start {
            tokens.push(Token {
                kind,
                start,
                end,
                text: input[start..end].to_string(),
            });
        }
    };

    while i < len {
        // Whitespace
        if bytes[i].is_ascii_whitespace() {
            let s = i;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            push(&mut tokens, TokenKind::Whitespace, s, i);
            continue;
        }
        // Semicolon separator
        if bytes[i] == b';' {
            push(&mut tokens, TokenKind::Delimiter, i, i + 1);
            i += 1;
            continue;
        }
        // Comment line
        if bytes[i] == b'#' {
            let s = i;
            i = input[i..].find('\n').map(|nl| i + nl).unwrap_or(len);
            push(&mut tokens, TokenKind::Comment, s, i);
            continue;
        }
        // Container scope `@...:` or `@3`
        if bytes[i] == b'@' {
            let s = i;
            i += 1;
            while i < len
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b';'
                && bytes[i] != b'{'
                && bytes[i] != b'/'
            {
                if bytes[i] == b'"' {
                    i += 1;
                    while i < len && bytes[i] != b'"' {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    continue;
                }
                if bytes[i] == b':' {
                    i += 1;
                    if let Some(reg_len) = match_region_prefix(&input[i..]) {
                        i += reg_len;
                    }
                    break;
                }
                if matches!(bytes[i], b'd' | b'a' | b'i')
                    && (i + 1 >= len
                        || bytes[i + 1].is_ascii_whitespace()
                        || bytes[i + 1] == b';'
                        || bytes[i + 1] == b'/')
                {
                    break;
                }
                i += 1;
            }
            push(&mut tokens, TokenKind::Scope, s, i);
            continue;
        }
        // Numeric / `$` address prefix
        if bytes[i] == b'$' || bytes[i].is_ascii_digit() {
            let s = i;
            while i < len
                && (bytes[i].is_ascii_digit()
                    || bytes[i] == b','
                    || bytes[i] == b'$'
                    || bytes[i] == b'!')
            {
                i += 1;
            }
            push(&mut tokens, TokenKind::Address, s, i);
            continue;
        }
        // Standalone brace block `{...}`
        if bytes[i] == b'{' {
            let s = i;
            let mut depth = 0usize;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'{' {
                    depth += 1;
                } else if bytes[i] == b'}' {
                    depth = depth.saturating_sub(1);
                    i += 1;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                i += 1;
            }
            push(&mut tokens, TokenKind::Brace, s, i);
            continue;
        }
        // Commands: s, d, a, i, c, y
        if matches!(bytes[i], b's' | b'd' | b'a' | b'c' | b'i' | b'y') {
            let cmd = bytes[i];
            push(&mut tokens, TokenKind::Command, i, i + 1);
            i += 1;
            if i < len && !bytes[i].is_ascii_whitespace() && bytes[i] != b';' {
                let delim = bytes[i];
                push(&mut tokens, TokenKind::Delimiter, i, i + 1);
                i += 1;
                let expected_parts = if cmd == b's' || cmd == b'y' { 2 } else { 1 };
                for part in 0..expected_parts {
                    if i >= len {
                        break;
                    }
                    let mut p_start = i;
                    while i < len && bytes[i] != b'\n' {
                        if bytes[i] == b'\\' && i + 1 < len {
                            i += 2;
                            continue;
                        }
                        if bytes[i] == delim {
                            break;
                        }
                        if part == 1 && bytes[i] == b'{' {
                            push(&mut tokens, TokenKind::Text, p_start, i);
                            let b_start = i;
                            let mut depth = 0usize;
                            while i < len && bytes[i] != b'\n' {
                                if bytes[i] == b'\\' && i + 1 < len {
                                    i += 2;
                                    continue;
                                }
                                if bytes[i] == b'{' {
                                    depth += 1;
                                } else if bytes[i] == b'}' {
                                    depth = depth.saturating_sub(1);
                                    i += 1;
                                    if depth == 0 {
                                        break;
                                    }
                                    continue;
                                }
                                i += 1;
                            }
                            push(&mut tokens, TokenKind::Brace, b_start, i);
                            p_start = i;
                            continue;
                        }
                        i += 1;
                    }
                    if i > p_start {
                        let kind = if part == 0 {
                            TokenKind::Pattern
                        } else {
                            TokenKind::Text
                        };
                        push(&mut tokens, kind, p_start, i);
                    }
                    if i < len && bytes[i] == delim {
                        push(&mut tokens, TokenKind::Delimiter, i, i + 1);
                        i += 1;
                    }
                }
                let f_start = i;
                while i < len
                    && (matches!(bytes[i], b'g' | b'i' | b'm') || bytes[i].is_ascii_digit())
                {
                    i += 1;
                }
                push(&mut tokens, TokenKind::Flag, f_start, i);
            }
            continue;
        }

        // Fallback single UTF-8 char
        let ch_len = input[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        push(&mut tokens, TokenKind::Text, i, i + ch_len);
        i += ch_len;
    }

    tokens
}
