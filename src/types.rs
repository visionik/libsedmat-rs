use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Substitute,
    Delete,
    Append,
    Insert,
    Transliterate,
    Brace,
}

impl Command {
    pub fn as_str(self) -> &'static str {
        match self {
            Command::Substitute => "s",
            Command::Delete => "d",
            Command::Append => "a",
            Command::Insert => "i",
            Command::Transliterate => "y",
            Command::Brace => "brace",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    /// 1-based start index, or -1 for `$`.
    pub start: i32,
    /// 1-based end index, -1 for `$`, or 0 for single address.
    pub end: i32,
    pub has_range: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerScope {
    pub container_type: Option<String>,
    pub address: Option<Address>,
    pub all: bool,
    pub name: Option<String>,
    pub region: Option<String>,
    pub container_op: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    pub flag: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BraceExpression {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub code: Option<bool>,
    pub sup: Option<bool>,
    pub sub: Option<bool>,
    pub small_caps: Option<bool>,

    pub text: Option<String>,
    pub color: Option<String>,
    pub bg: Option<String>,
    pub font: Option<String>,
    pub size: Option<f64>,
    pub url: Option<String>,
    pub heading: Option<String>,
    pub leading: Option<f64>,
    pub align: Option<String>,
    pub opacity: Option<f64>,
    pub indent: Option<i32>,
    pub kerning: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub spacing_set: bool,
    pub spacing_above: f64,
    pub spacing_below: f64,
    pub effect: Option<String>,
    pub cols: Option<i32>,

    pub reset: bool,
    pub no_reset: bool,
    pub has_break: bool,
    pub break_kind: String,
    pub comment: Option<String>,
    pub bookmark: Option<String>,
    pub check: Option<bool>,
    pub has_toc: bool,
    pub toc: i32,
    pub img_ref: Option<String>,
    pub table_ref: Option<String>,

    // v3.7 slide & shape flags
    pub layout: Option<String>,
    pub zone: Option<String>,
    pub shape: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub notes: Option<String>,
    pub transition: Option<String>,
    pub move_to: Option<i32>,
    pub dup: bool,
    pub hide: bool,

    pub inline_spans: Vec<InlineSpan>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BraceSpan {
    pub start: usize,
    pub end: usize,
    pub is_global: bool,
    pub expr: BraceExpression,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCreateSpec {
    pub rows: usize,
    pub columns: usize,
    pub header: bool,
    pub cells: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub command: Command,
    pub pattern: String,
    pub replacement: String,
    pub global: bool,
    pub nth_match: usize,
    pub flags: String,
    pub address: Option<Address>,
    pub container: Option<ContainerScope>,
    pub brace: Option<BraceExpression>,
    pub brace_spans: Vec<BraceSpan>,
    pub table_create: Option<TableCreateSpec>,
    pub enriched: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

// --- Document Projection ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentBlockKind {
    Paragraph,
    Table,
    TableOfContents,
    SectionBreak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBlock {
    pub kind: DocumentBlockKind,
    pub start_index: usize,
    pub end_index: usize,
    pub item_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentParagraph {
    pub text: String,
    pub start_index: usize,
    pub end_index: usize,
    pub named_style: String,
    pub bullet_list_id: Option<String>,
    pub bullet_nesting_level: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTableCell {
    pub text: String,
    pub start_index: usize,
    pub end_index: usize,
    pub text_start_index: usize,
    pub text_end_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTableRow {
    pub cells: Vec<DocumentTableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTable {
    pub start_index: usize,
    pub end_index: usize,
    pub declared_rows: usize,
    pub declared_columns: usize,
    pub rows: Vec<DocumentTableRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentSegment {
    pub tab_id: String,
    pub title: String,
    pub body_start_index: usize,
    pub body_end_index: usize,
    pub blocks: Vec<DocumentBlock>,
    pub paragraphs: Vec<DocumentParagraph>,
    pub tables: Vec<DocumentTable>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentProjection {
    pub document_id: String,
    pub revision_id: String,
    pub legacy: Option<DocumentSegment>,
    pub tabs: Vec<DocumentSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressElementKind {
    Paragraph,
    Table,
    Toc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressElement {
    pub number: usize,
    pub kind: AddressElementKind,
    pub text: String,
    pub start_index: usize,
    pub end_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressMutation {
    pub start_index: usize,
    pub end_index: usize,
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_index: usize,
    pub end_index: usize,
    pub insert_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatIntent {
    pub start_index: usize,
    pub end_index: usize,
    pub formats: Vec<String>,
    pub brace: Option<BraceExpression>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextPlan {
    pub match_count: usize,
    pub text_edits: Vec<TextEdit>,
    pub formatting: Vec<FormatIntent>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecuteOptions {
    pub dry_run: bool,
    pub tab_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionReport {
    pub dry_run: bool,
    pub total_matches: usize,
}

/// Target-agnostic backend trait for executing SEDMAT programs.
pub trait Backend {
    fn project(&self) -> Result<DocumentProjection>;
    fn apply_text_plan(&mut self, tab_id: &str, plan: &TextPlan) -> Result<()>;
    fn apply_address_mutations(&mut self, tab_id: &str, mutations: &[AddressMutation]) -> Result<()>;
    fn apply_transliterate(
        &mut self,
        tab_id: &str,
        source: &str,
        dest: &str,
        targets: &[AddressElement],
    ) -> Result<usize>;
    fn apply_container_op(&mut self, _scope: &ContainerScope, _expr: &Expression) -> Result<usize> {
        Ok(0)
    }
}
