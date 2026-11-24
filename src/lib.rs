//! Zero-dependency Rust library implementing SEDMAT v3.6 / v3.7.
//!
//! See <https://sedmat.org> for the full SEDMAT language specification.

pub mod brace;
pub mod error;
pub mod intellisense;
pub mod memory;
pub mod parser;
pub mod planner;
pub mod regex_lite;
pub mod types;

pub use brace::{
    brace_expression_has_any_format, has_brace_formatting, merge_brace_spans,
    parse_brace_expression, parse_brace_replacement, resolve_color,
};
pub use error::{Result, SedmatError};
pub use intellisense::{
    extract_diagnostic, get_completions, tokenize, ContextInfo, Diagnostic, FlagDefinition,
    IntelliSenseExtensions, IntelliSenseResult, Profile, SignaturePart, SnippetOption, Suggestion,
    SuggestionKind, Token, TokenKind, ValueOption,
};
pub use memory::{Executor, MemoryDocument};
pub use parser::{enrich, parse, parse_address, parse_container_scope, parse_expression, parse_program};
pub use planner::{
    address_elements, format_numbered, format_structure, plan_addressed_append,
    plan_addressed_delete, plan_addressed_insert, plan_paragraph_substitutions, resolve_address,
    select_segment,
};
pub use types::*;
