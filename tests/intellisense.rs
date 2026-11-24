use std::collections::HashMap;
use libsedmat::{
    extract_diagnostic, get_completions, tokenize, ContextInfo, FlagDefinition,
    IntelliSenseExtensions, Profile, SuggestionKind, TokenKind, ValueOption,
};

#[test]
fn diagnostic_parses_offset_and_hint() {
    let diag = extract_diagnostic(
        "SEDMAT error at offset 9: Unclosed brace expression — add a closing '}'",
    );
    assert!(!diag.ok);
    assert_eq!(diag.error_offset, Some(9));
    assert_eq!(diag.error_length, 1);
    assert_eq!(diag.summary, "Unclosed brace expression");
    assert_eq!(diag.hint.as_deref(), Some("add a closing '}'"));

    let plain = extract_diagnostic("invalid command");
    assert_eq!(plain.error_offset, None);
    assert_eq!(plain.summary, "invalid command");
}

#[test]
fn completions_document_and_slides_profiles() {
    let doc = get_completions("s/foo/{", 7, &ContextInfo::default());
    assert!(doc.suggestions.iter().any(|s| s.label == "b"));
    assert!(!doc.suggestions.iter().any(|s| s.label == "transition="));

    let slides_ctx = ContextInfo {
        mode: Profile::Slides,
        active_container: Some(3),
        ..Default::default()
    };
    let slides = get_completions("s/foo/{tr", 9, &slides_ctx);
    assert!(slides.suggestions.iter().any(|s| s.label == "transition="));

    let at_res = get_completions("@", 1, &slides_ctx);
    assert!(at_res.suggestions.iter().any(|s| s.label == "@3:title:"));
    assert_eq!(at_res.ghost_suffix.as_deref(), Some("3:"));
}

#[test]
fn completions_canonical_color_swatches() {
    let res = get_completions("s/foo/{c=", 9, &ContextInfo::default());
    let green = res
        .suggestions
        .iter()
        .find(|s| s.label == "green")
        .expect("green swatch");
    assert_eq!(green.swatch.as_deref(), Some("#00FF00"));

    let orange = res
        .suggestions
        .iter()
        .find(|s| s.label == "orange")
        .expect("orange swatch");
    assert_eq!(orange.swatch.as_deref(), Some("#FF8C00"));
}

#[test]
fn completions_semicolon_chaining_and_extensions() {
    let res = get_completions("s/a/b/g; s/c/{c=re", 18, &ContextInfo::default());
    assert!(res.suggestions.iter().any(|s| s.label == "red"));
    assert_eq!(res.ghost_suffix.as_deref(), Some("d"));

    let mut values = HashMap::new();
    values.insert(
        "brand".to_string(),
        vec![ValueOption {
            value: "acme".to_string(),
            detail: "Acme primary".to_string(),
            swatch: Some("#123456".to_string()),
        }],
    );
    let ctx = ContextInfo {
        mode: Profile::Document,
        extensions: Some(IntelliSenseExtensions {
            flags: vec![FlagDefinition {
                key: "brand=".to_string(),
                insert: "brand=".to_string(),
                detail: "Custom brand flag".to_string(),
                profiles: vec![],
            }],
            values,
            ..Default::default()
        }),
        ..Default::default()
    };

    let flag_res = get_completions("s/x/{br", 7, &ctx);
    assert_eq!(flag_res.suggestions[0].label, "brand=");
    assert_eq!(flag_res.suggestions[0].kind, SuggestionKind::Flag);

    let val_res = get_completions("s/x/{brand=", 11, &ctx);
    assert_eq!(val_res.suggestions[0].label, "acme");
    assert_eq!(val_res.suggestions[0].swatch.as_deref(), Some("#123456"));
}

#[test]
fn tokenizer_covers_scope_command_brace_text_and_flags() {
    let input = "@2:title:s/foo/{b c=red}bar/gi";
    let tokens = tokenize(input);
    let reconstructed: String = tokens.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(reconstructed, input);

    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Scope && t.text == "@2:title:"));
    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Command && t.text == "s"));
    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Pattern && t.text == "foo"));
    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Brace && t.text == "{b c=red}"));
    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Text && t.text == "bar"));
    assert!(tokens
        .iter()
        .any(|t| t.kind == TokenKind::Flag && t.text == "gi"));
}

#[test]
fn tokenizer_handles_incomplete_input_without_panicking() {
    let input = "# comment\n@1:s/unclosed/{b c=";
    let tokens = tokenize(input);
    let reconstructed: String = tokens.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(reconstructed, input);
    assert_eq!(tokens[0].kind, TokenKind::Comment);
}
