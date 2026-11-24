# libsedmat-rs

Zero-dependency Rust library implementing **[SEDMAT](https://sedmat.org)** (*Sed-Expression-Driven Mark(up|down) Annotation & Transformation*, **v3.6 + v3.7 `@` container scopes**).

For full language syntax, flags, and container scope documentation, see **[sedmat.org](https://sedmat.org)** and [`visionik/sedmat`](https://github.com/visionik/sedmat).

## Installation

Add `libsedmat` to your `Cargo.toml`:

```toml
[dependencies]
libsedmat = "0.1"
```

**Zero third-party dependencies** — compiles offline using Rust `std` only.

## Quick Start

```rust
use libsedmat::{
    format_numbered, select_segment, Backend, ExecuteOptions, Executor, MemoryDocument,
};

fn main() -> libsedmat::Result<()> {
    let mut doc = MemoryDocument::new(&[
        "Draft Proposal",
        "Critical warning in paragraph two",
        "Temporary note to remove",
    ]);

    let script = r#"
1s/Draft/{h=1}Final/
2s/warning/{b c=red}/
$d
$a/Appendix A/
"#;

    let mut exec = Executor::new(&mut doc);
    let preview = exec.execute_string(script, &ExecuteOptions { dry_run: true, ..Default::default() })?;
    println!("Dry-run matched {} operations", preview.total_matches);

    exec.execute_string(script, &ExecuteOptions::default())?;
    let proj = doc.project()?;
    let seg = select_segment(&proj, "")?;
    println!("{}", format_numbered(seg));
    Ok(())
}
```

## SEDMAT v3.7 Container Scopes (`@`)

Target slides, sheets, tabs, pages, headers/footers, and named regions (`title`, `sub`, `body`, `notes`, `shape=N`):

```rust
use libsedmat::parse_program;

let prog = parse_program(r#"
@1:{z=#0F172A transition=fade}
@slide=2:title:s/.*/{c=navy}New Title/
@2:notes:a/Pause for Q&A./
@"Q4 Budget":s/USD/EUR/g
@$a/{layout=two-col}Next Steps/
@3{move=1}
"#)?;
```

## IntelliSense, Diagnostics & Syntax Tokenizer

`libsedmat-rs` includes a UI-agnostic, zero-dependency IntelliSense engine for editor command bars, TUIs, CLIs, and LSPs:

```rust
use libsedmat::{extract_diagnostic, get_completions, tokenize, ContextInfo, Profile};

// 1. Cursor-aware completions with capability profiles & host extensions
let ctx = ContextInfo {
    mode: Profile::Slides,
    active_container: Some(2),
    ..Default::default()
};
let res = get_completions("s/foo/{c=re", 11, &ctx);
// res.suggestions[0].label == "red", swatch == Some("#FF0000")
// res.ghost_suffix == Some("d")

// 2. Non-throwing syntax tokenizer
let tokens = tokenize("@2:title:s/foo/{b c=red}bar/gi");

// 3. Structured diagnostics from parser error messages
let diag = extract_diagnostic("SEDMAT error at offset 9: Unclosed brace — add '}'");
```

## Implementing a Custom Backend

Implement the `libsedmat::Backend` trait (`project`, `apply_text_plan`, `apply_address_mutations`, `apply_transliterate`, and optional `apply_container_op`) to connect SEDMAT to any document target. All `start_index`/`end_index` coordinates are UTF-16 code unit offsets.

## Guide for AI Agents

- Inspect first via `backend.project()` → `format_structure(seg)` / `format_numbered(seg)` for exact 1-based addressing.
- Validate scripts with `ExecuteOptions { dry_run: true, .. }` before applying.
- Honor `BraceExpression.no_reset` (`{!0}`) for additive formatting vs. implicit reset default.
- Conformance suite: `testdata/conformance/v3.6.json` (shared across Go, TS, Rust, Python, and C++).

## Examples

All examples have **zero external dependencies** (Rust `std` only):

- **[`examples/lsp_server.rs`](./examples/lsp_server.rs)** — SEDMAT Language Server (LSP over stdio) with live diagnostics, profile-aware completions + color swatches, and semantic tokens:
  ```bash
  cargo run --example lsp_server
  ```
- **[`examples/doc_playground.rs`](./examples/doc_playground.rs)** — Launch-a-web Document Playground (`http://127.0.0.1:4173`) with live syntax highlighting, IntelliSense dropdown, color swatches, ghost text, dry-run preview, and numbered document view:
  ```bash
  cargo run --example doc_playground
  ```
- **[`examples/slide_deck.rs`](./examples/slide_deck.rs)** — Launch-a-web Slide Deck Studio (`http://127.0.0.1:4174`) demonstrating SEDMAT v3.7 `@` container scopes and Slides profile completions:
  ```bash
  cargo run --example slide_deck
  ```

## Development

```bash
make check   # cargo test --offline
```

## Related

- **SEDMAT Website & Docs**: [https://sedmat.org](https://sedmat.org)
- **SEDMAT Specification**: [https://github.com/visionik/sedmat](https://github.com/visionik/sedmat)
- **Sibling Libraries**: [`libsedmat-go`](https://github.com/visionik/libsedmat-go) · [`libsedmat-ts`](https://github.com/visionik/libsedmat-ts) · [`libsedmat-py`](https://github.com/visionik/libsedmat-py) · [`libsedmat-cpp`](https://github.com/visionik/libsedmat-cpp)

## License

MIT — see [LICENSE](./LICENSE).
