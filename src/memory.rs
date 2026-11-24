use crate::error::{Result, SedmatError};
use crate::parser::{enrich, parse_program};
use crate::planner::{
    address_elements, plan_addressed_append, plan_addressed_delete, plan_addressed_insert,
    plan_paragraph_substitutions, resolve_address, select_segment, utf16_len,
};
use crate::regex_lite::SedRegex;
use crate::types::{
    AddressElement, AddressMutation, Backend, Command, ContainerScope, DocumentBlock,
    DocumentBlockKind, DocumentParagraph, DocumentProjection, DocumentSegment, ExecuteOptions,
    ExecutionReport, Expression, FormatIntent, Program, TextPlan,
};

#[derive(Debug, Clone)]
pub struct MemoryParagraph {
    pub text: String,
    pub named_style: String,
}

#[derive(Debug, Clone)]
pub struct MemoryTab {
    pub id: String,
    pub title: String,
    pub background: Option<String>,
    pub notes: Option<String>,
    pub transition: Option<String>,
    pub layout: Option<String>,
    pub hidden: bool,
    pub paragraphs: Vec<MemoryParagraph>,
    pub formatting: Vec<FormatIntent>,
}

/// Pure-Rust in-memory document implementing `Backend` for deterministic tests.
#[derive(Debug, Clone)]
pub struct MemoryDocument {
    pub id: String,
    pub revision_id: String,
    pub tabs: Vec<MemoryTab>,
}

impl MemoryDocument {
    pub fn new(paragraphs: &[&str]) -> Self {
        Self {
            id: "mem-doc-1".to_string(),
            revision_id: "rev-1".to_string(),
            tabs: vec![MemoryTab {
                id: String::new(),
                title: "Default".to_string(),
                background: None,
                notes: None,
                transition: None,
                layout: None,
                hidden: false,
                paragraphs: paragraphs
                    .iter()
                    .map(|p| MemoryParagraph {
                        text: (*p).to_string(),
                        named_style: "NORMAL_TEXT".to_string(),
                    })
                    .collect(),
                formatting: Vec::new(),
            }],
        }
    }

    pub fn add_tab(&mut self, id: &str, title: &str, paragraphs: &[&str]) {
        self.tabs.push(MemoryTab {
            id: id.to_string(),
            title: title.to_string(),
            background: None,
            notes: None,
            transition: None,
            layout: None,
            hidden: false,
            paragraphs: paragraphs
                .iter()
                .map(|p| MemoryParagraph {
                    text: (*p).to_string(),
                    named_style: "NORMAL_TEXT".to_string(),
                })
                .collect(),
            formatting: Vec::new(),
        });
    }

    pub fn paragraph_texts(&self, tab_id: &str) -> Vec<String> {
        self.find_tab(tab_id)
            .map(|t| t.paragraphs.iter().map(|p| p.text.clone()).collect())
            .unwrap_or_default()
    }

    fn find_tab(&self, tab_id: &str) -> Result<&MemoryTab> {
        if tab_id.is_empty() {
            return self
                .tabs
                .first()
                .ok_or_else(|| SedmatError::new("document has no tabs"));
        }
        self.tabs
            .iter()
            .find(|t| t.id == tab_id || t.title.eq_ignore_ascii_case(tab_id))
            .ok_or_else(|| SedmatError::new(format!("tab \"{tab_id}\" not found")))
    }

    fn find_tab_mut(&mut self, tab_id: &str) -> Result<&mut MemoryTab> {
        if tab_id.is_empty() {
            return self
                .tabs
                .first_mut()
                .ok_or_else(|| SedmatError::new("document has no tabs"));
        }
        self.tabs
            .iter_mut()
            .find(|t| t.id == tab_id || t.title.eq_ignore_ascii_case(tab_id))
            .ok_or_else(|| SedmatError::new(format!("tab \"{tab_id}\" not found")))
    }

    fn resolve_container_indices(&self, scope: &ContainerScope) -> Result<Vec<usize>> {
        if scope.all {
            return Ok((0..self.tabs.len()).collect());
        }
        if let Some(ref name) = scope.name {
            let idx = self
                .tabs
                .iter()
                .position(|t| t.id == *name || t.title.eq_ignore_ascii_case(name))
                .ok_or_else(|| SedmatError::new(format!("container \"{name}\" not found")))?;
            return Ok(vec![idx]);
        }
        if let Some(ref addr) = scope.address {
            let total = self.tabs.len() as i32;
            if total == 0 {
                return Err(SedmatError::new("document has no containers"));
            }
            let start = if addr.start == -1 { total } else { addr.start };
            let end = if !addr.has_range {
                start
            } else if addr.end == -1 {
                total
            } else {
                addr.end
            };
            if start < 1 || start > total || end < 1 || end > total || end < start {
                return Err(SedmatError::new("container address out of range"));
            }
            return Ok(((start - 1) as usize..end as usize).collect());
        }
        Ok(if self.tabs.is_empty() { vec![] } else { vec![0] })
    }
}

fn project_tab(tab: &MemoryTab) -> DocumentSegment {
    let mut blocks = Vec::new();
    let mut paragraphs = Vec::new();
    let mut cursor = 1usize;
    for (idx, p) in tab.paragraphs.iter().enumerate() {
        let full = format!("{}\n", p.text);
        let start = cursor;
        let end = cursor + utf16_len(&full);
        paragraphs.push(DocumentParagraph {
            text: full,
            start_index: start,
            end_index: end,
            named_style: p.named_style.clone(),
            bullet_list_id: None,
            bullet_nesting_level: 0,
        });
        blocks.push(DocumentBlock {
            kind: DocumentBlockKind::Paragraph,
            start_index: start,
            end_index: end,
            item_index: Some(idx),
        });
        cursor = end;
    }
    DocumentSegment {
        tab_id: tab.id.clone(),
        title: tab.title.clone(),
        body_start_index: 1,
        body_end_index: cursor,
        blocks,
        paragraphs,
        tables: Vec::new(),
    }
}

fn splice_utf16(s: &str, start: usize, end: usize, insert: &str) -> Result<String> {
    let units: Vec<u16> = s.encode_utf16().collect();
    if start > end || end > units.len() {
        return Err(SedmatError::new("utf16 splice out of bounds"));
    }
    let mut out = Vec::with_capacity(units.len() - (end - start) + utf16_len(insert));
    out.extend_from_slice(&units[..start]);
    out.extend(insert.encode_utf16());
    out.extend_from_slice(&units[end..]);
    String::from_utf16(&out).map_err(|_| SedmatError::new("invalid utf16 result"))
}

impl Backend for MemoryDocument {
    fn project(&self) -> Result<DocumentProjection> {
        let mut tabs = Vec::with_capacity(self.tabs.len());
        let mut legacy = None;
        for (i, tab) in self.tabs.iter().enumerate() {
            let seg = project_tab(tab);
            if i == 0 && tab.id.is_empty() {
                legacy = Some(seg.clone());
            }
            tabs.push(seg);
        }
        Ok(DocumentProjection {
            document_id: self.id.clone(),
            revision_id: self.revision_id.clone(),
            legacy,
            tabs,
        })
    }

    fn apply_text_plan(&mut self, tab_id: &str, plan: &TextPlan) -> Result<()> {
        let tab = self.find_tab_mut(tab_id)?;
        let mut edits = plan.text_edits.clone();
        edits.sort_by(|a, b| b.start_index.cmp(&a.start_index));
        for edit in edits {
            let seg = project_tab(tab);
            for (i, p) in seg.paragraphs.iter().enumerate() {
                if edit.start_index >= p.start_index && edit.end_index <= p.end_index {
                    let updated = splice_utf16(
                        &p.text,
                        edit.start_index - p.start_index,
                        edit.end_index - p.start_index,
                        &edit.insert_text,
                    )?;
                    let trimmed = updated.strip_suffix('\n').unwrap_or(&updated);
                    let parts: Vec<&str> = trimmed.split('\n').collect();
                    let style = tab.paragraphs[i].named_style.clone();
                    let new_paras: Vec<MemoryParagraph> = parts
                        .into_iter()
                        .map(|t| MemoryParagraph {
                            text: t.to_string(),
                            named_style: style.clone(),
                        })
                        .collect();
                    tab.paragraphs.splice(i..=i, new_paras);
                    break;
                }
            }
        }
        tab.formatting.extend(plan.formatting.clone());
        Ok(())
    }

    fn apply_address_mutations(&mut self, tab_id: &str, mutations: &[AddressMutation]) -> Result<()> {
        let tab = self.find_tab_mut(tab_id)?;
        let mut ordered = mutations.to_vec();
        ordered.sort_by(|a, b| {
            b.start_index
                .cmp(&a.start_index)
                .then_with(|| b.end_index.cmp(&a.end_index))
        });

        for m in ordered {
            let seg = project_tab(tab);
            if let Some(ref ins) = m.insert_text {
                if m.end_index == 0 || m.end_index == m.start_index {
                    let is_append = ins.starts_with('\n');
                    let cleaned = ins.strip_prefix('\n').unwrap_or(ins);
                    let cleaned = cleaned.strip_suffix('\n').unwrap_or(cleaned);
                    let new_paras: Vec<MemoryParagraph> = cleaned
                        .split('\n')
                        .map(|t| MemoryParagraph {
                            text: t.to_string(),
                            named_style: "NORMAL_TEXT".to_string(),
                        })
                        .collect();
                    let mut inserted = false;
                    for (i, b) in seg.blocks.iter().enumerate() {
                        if is_append && (m.start_index == b.end_index - 1 || m.start_index == b.end_index) {
                            tab.paragraphs.splice(i + 1..i + 1, new_paras.clone());
                            inserted = true;
                            break;
                        }
                        if !is_append && m.start_index == b.start_index {
                            tab.paragraphs.splice(i..i, new_paras.clone());
                            inserted = true;
                            break;
                        }
                    }
                    if !inserted {
                        tab.paragraphs.extend(new_paras);
                    }
                    continue;
                }
            }

            // Delete
            if seg.blocks.len() == 1
                && m.start_index >= seg.blocks[0].start_index
                && m.end_index <= seg.blocks[0].end_index
            {
                tab.paragraphs[0].text.clear();
                continue;
            }
            let mut kept = Vec::new();
            for (i, b) in seg.blocks.iter().enumerate() {
                if m.start_index == b.end_index.saturating_sub(1) && m.end_index > b.end_index {
                    kept.push(tab.paragraphs[i].clone());
                    continue;
                }
                if (b.start_index >= m.start_index && b.end_index <= m.end_index)
                    || (b.start_index >= m.start_index
                        && b.end_index.saturating_sub(1) <= m.end_index
                        && i == seg.blocks.len() - 1)
                {
                    continue;
                }
                kept.push(tab.paragraphs[i].clone());
            }
            if kept.is_empty() {
                kept.push(MemoryParagraph {
                    text: String::new(),
                    named_style: "NORMAL_TEXT".to_string(),
                });
            }
            tab.paragraphs = kept;
        }
        Ok(())
    }

    fn apply_transliterate(
        &mut self,
        tab_id: &str,
        source: &str,
        dest: &str,
        targets: &[AddressElement],
    ) -> Result<usize> {
        let tab = self.find_tab_mut(tab_id)?;
        let src: Vec<char> = source.chars().collect();
        let dst: Vec<char> = dest.chars().collect();
        let mut changed = 0;
        for (i, p) in tab.paragraphs.iter_mut().enumerate() {
            let num = i + 1;
            if !targets.is_empty() && !targets.iter().any(|t| t.number == num) {
                continue;
            }
            let mut out = String::with_capacity(p.text.len());
            for ch in p.text.chars() {
                if let Some(pos) = src.iter().position(|&c| c == ch) {
                    out.push(dst[pos]);
                    changed += 1;
                } else {
                    out.push(ch);
                }
            }
            p.text = out;
        }
        Ok(changed)
    }

    fn apply_container_op(&mut self, scope: &ContainerScope, expr: &Expression) -> Result<usize> {
        let indices = self.resolve_container_indices(scope)?;
        match expr.command {
            Command::Delete => {
                for &idx in indices.iter().rev() {
                    self.tabs.remove(idx);
                }
                Ok(indices.len())
            }
            Command::Append | Command::Insert => {
                let title = expr.replacement.trim().to_string();
                let new_tab = MemoryTab {
                    id: format!("tab-{}", self.tabs.len() + 1),
                    title: title.clone(),
                    background: None,
                    notes: None,
                    transition: None,
                    layout: expr.brace.as_ref().and_then(|b| b.layout.clone()),
                    hidden: false,
                    paragraphs: vec![MemoryParagraph {
                        text: title,
                        named_style: "NORMAL_TEXT".to_string(),
                    }],
                    formatting: Vec::new(),
                };
                let anchor = *indices.last().unwrap_or(&0);
                let pos = if expr.command == Command::Append {
                    (anchor + 1).min(self.tabs.len())
                } else {
                    anchor
                };
                self.tabs.insert(pos, new_tab);
                Ok(1)
            }
            Command::Brace => {
                if let Some(ref b) = expr.brace {
                    for &idx in &indices {
                        let tab = &mut self.tabs[idx];
                        if let Some(ref bg) = b.bg { tab.background = Some(bg.clone()); }
                        if let Some(ref n) = b.notes { tab.notes = Some(n.clone()); }
                        if let Some(ref tr) = b.transition { tab.transition = Some(tr.clone()); }
                        if let Some(ref l) = b.layout { tab.layout = Some(l.clone()); }
                        if b.hide { tab.hidden = true; }
                    }
                }
                Ok(indices.len())
            }
            _ => Ok(0),
        }
    }
}

/// Target-agnostic SEDMAT executor.
pub struct Executor<'a, B: Backend> {
    backend: &'a mut B,
}

impl<'a, B: Backend> Executor<'a, B> {
    pub fn new(backend: &'a mut B) -> Self {
        Self { backend }
    }

    pub fn execute_string(&mut self, script: &str, opts: &ExecuteOptions) -> Result<ExecutionReport> {
        let prog = parse_program(script)?;
        self.execute(prog, opts)
    }

    pub fn execute(&mut self, program: Program, opts: &ExecuteOptions) -> Result<ExecutionReport> {
        let enriched = enrich(program);
        let mut report = ExecutionReport {
            dry_run: opts.dry_run,
            total_matches: 0,
        };

        for expr in &enriched.expressions {
            if let Some(ref scope) = expr.container {
                if scope.container_op || expr.command == Command::Brace {
                    let n = if opts.dry_run {
                        1
                    } else {
                        self.backend.apply_container_op(scope, expr)?
                    };
                    report.total_matches += n;
                    continue;
                }
            }

            let proj = self.backend.project()?;
            let tab_id = resolve_tab_id(&proj, expr, &opts.tab_id)?;
            let seg = select_segment(&proj, &tab_id)?.clone();

            match expr.command {
                Command::Substitute => {
                    let paragraphs = if let Some(ref addr) = expr.address {
                        let targets = resolve_address(addr, &address_elements(&seg))?;
                        targets
                            .into_iter()
                            .map(|t| DocumentParagraph {
                                text: format!("{}\n", t.text),
                                start_index: t.start_index,
                                end_index: t.end_index,
                                named_style: String::new(),
                                bullet_list_id: None,
                                bullet_nesting_level: 0,
                            })
                            .collect()
                    } else {
                        seg.paragraphs.clone()
                    };
                    let plan = plan_paragraph_substitutions(&paragraphs, expr)?;
                    report.total_matches += plan.match_count;
                    if !opts.dry_run && plan.match_count > 0 {
                        self.backend.apply_text_plan(&tab_id, &plan)?;
                    }
                }
                Command::Delete => {
                    let elements = address_elements(&seg);
                    let mut targets = if let Some(ref addr) = expr.address {
                        resolve_address(addr, &elements)?
                    } else {
                        elements.clone()
                    };
                    if !expr.pattern.is_empty() {
                        let re = SedRegex::compile(&expr.pattern)?;
                        targets.retain(|t| re.is_match(&t.text));
                    }
                    let mutations = plan_addressed_delete(&elements, &targets);
                    report.total_matches += targets.len();
                    if !opts.dry_run && !mutations.is_empty() {
                        self.backend.apply_address_mutations(&tab_id, &mutations)?;
                    }
                }
                Command::Append | Command::Insert => {
                    let elements = address_elements(&seg);
                    let targets = if let Some(ref addr) = expr.address {
                        resolve_address(addr, &elements)?
                    } else {
                        let re = SedRegex::compile(&expr.pattern)?;
                        elements.into_iter().filter(|e| re.is_match(&e.text)).collect()
                    };
                    let mutations = if expr.command == Command::Insert {
                        plan_addressed_insert(&targets, &expr.replacement)
                    } else {
                        plan_addressed_append(&targets, &expr.replacement)
                    };
                    report.total_matches += targets.len();
                    if !opts.dry_run && !mutations.is_empty() {
                        self.backend.apply_address_mutations(&tab_id, &mutations)?;
                    }
                }
                Command::Transliterate => {
                    let targets = if let Some(ref addr) = expr.address {
                        resolve_address(addr, &address_elements(&seg))?
                    } else {
                        Vec::new()
                    };
                    if !opts.dry_run {
                        report.total_matches += self.backend.apply_transliterate(
                            &tab_id,
                            &expr.pattern,
                            &expr.replacement,
                            &targets,
                        )?;
                    }
                }
                Command::Brace => {}
            }
        }
        Ok(report)
    }
}

fn resolve_tab_id(proj: &DocumentProjection, expr: &Expression, default_tab: &str) -> Result<String> {
    let Some(ref scope) = expr.container else {
        return Ok(default_tab.to_string());
    };
    if let Some(ref name) = scope.name {
        return Ok(name.clone());
    }
    if let Some(ref addr) = scope.address {
        let total = proj.tabs.len() as i32;
        let idx = if addr.start == -1 { total } else { addr.start };
        if idx < 1 || idx > total {
            return Err(SedmatError::new("container address out of range"));
        }
        let t = &proj.tabs[(idx - 1) as usize];
        return Ok(if !t.tab_id.is_empty() {
            t.tab_id.clone()
        } else {
            t.title.clone()
        });
    }
    Ok(default_tab.to_string())
}
