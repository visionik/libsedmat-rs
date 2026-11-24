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

## Implementing a Custom Backend

Implement the `libsedmat::Backend` trait (`project`, `apply_text_plan`, `apply_address_mutations`, `apply_transliterate`, and optional `apply_container_op`) to connect SEDMAT to any document target. All `start_index`/`end_index` coordinates are UTF-16 code unit offsets.

## Guide for AI Agents

- Inspect first via `backend.project()` → `format_structure(seg)` / `format_numbered(seg)` for exact 1-based addressing.
- Validate scripts with `ExecuteOptions { dry_run: true, .. }` before applying.
- Honor `BraceExpression.no_reset` (`{!0}`) for additive formatting vs. implicit reset default.
- Conformance suite: `testdata/conformance/v3.6.json` (shared across Go, TS, Rust, Python, and C++).

## Development

```bash
make check   # cargo test --offline
```

## License

MIT — see [LICENSE](./LICENSE).
