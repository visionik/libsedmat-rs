use crate::error::{Result, SedmatError};
use crate::regex_lite::{expand_replacement, SedRegex};
use crate::types::{
    Address, AddressElement, AddressElementKind, AddressMutation, DocumentBlockKind,
    DocumentParagraph, DocumentProjection, DocumentSegment, Expression, FormatIntent, TextEdit,
    TextPlan,
};

pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn address_elements(seg: &DocumentSegment) -> Vec<AddressElement> {
    let mut out = Vec::new();
    let mut num = 0usize;
    for block in &seg.blocks {
        if block.kind == DocumentBlockKind::SectionBreak {
            continue;
        }
        num += 1;
        match block.kind {
            DocumentBlockKind::Paragraph => {
                let text = block
                    .item_index
                    .and_then(|i| seg.paragraphs.get(i))
                    .map(|p| p.text.trim_end_matches('\n').to_string())
                    .unwrap_or_default();
                out.push(AddressElement {
                    number: num,
                    kind: AddressElementKind::Paragraph,
                    text,
                    start_index: block.start_index,
                    end_index: block.end_index,
                });
            }
            DocumentBlockKind::Table => {
                let text = block
                    .item_index
                    .and_then(|i| seg.tables.get(i))
                    .map(table_preview)
                    .unwrap_or_else(|| "[empty table]".to_string());
                out.push(AddressElement {
                    number: num,
                    kind: AddressElementKind::Table,
                    text,
                    start_index: block.start_index,
                    end_index: block.end_index,
                });
            }
            DocumentBlockKind::TableOfContents => {
                out.push(AddressElement {
                    number: num,
                    kind: AddressElementKind::Toc,
                    text: "[table of contents]".to_string(),
                    start_index: block.start_index,
                    end_index: block.end_index,
                });
            }
            DocumentBlockKind::SectionBreak => unreachable!(),
        }
    }
    out
}

pub fn select_segment<'a>(proj: &'a DocumentProjection, tab_id: &str) -> Result<&'a DocumentSegment> {
    if !tab_id.is_empty() {
        return proj
            .tabs
            .iter()
            .find(|t| t.tab_id == tab_id || t.title.eq_ignore_ascii_case(tab_id))
            .ok_or_else(|| SedmatError::new(format!("tab \"{tab_id}\" not found")));
    }
    if let Some(ref legacy) = proj.legacy {
        return Ok(legacy);
    }
    proj.tabs
        .first()
        .ok_or_else(|| SedmatError::new("document has no content"))
}

pub fn format_structure(seg: &DocumentSegment) -> String {
    let mut out = String::new();
    let mut num = 0usize;
    for block in &seg.blocks {
        if block.kind == DocumentBlockKind::SectionBreak {
            continue;
        }
        num += 1;
        match block.kind {
            DocumentBlockKind::Paragraph => {
                let (style, text) = block
                    .item_index
                    .and_then(|i| seg.paragraphs.get(i))
                    .map(|p| {
                        let s = if p.named_style.is_empty() {
                            "NORMAL_TEXT"
                        } else {
                            p.named_style.as_str()
                        };
                        (s, p.text.trim_end_matches('\n'))
                    })
                    .unwrap_or(("NORMAL_TEXT", ""));
                out.push_str(&format!("[{num}] paragraph ({style}): {:?}\n", text));
            }
            DocumentBlockKind::Table => {
                if let Some(t) = block.item_index.and_then(|i| seg.tables.get(i)) {
                    let rows = if !t.rows.is_empty() { t.rows.len() } else { t.declared_rows };
                    let cols = t
                        .rows
                        .first()
                        .map(|r| r.cells.len())
                        .unwrap_or(t.declared_columns);
                    out.push_str(&format!(
                        "[{num}] table ({rows}x{cols}): {:?}\n",
                        table_preview(t)
                    ));
                }
            }
            DocumentBlockKind::TableOfContents => {
                out.push_str(&format!("[{num}] toc: \"[table of contents]\"\n"));
            }
            DocumentBlockKind::SectionBreak => unreachable!(),
        }
    }
    out
}

pub fn format_numbered(seg: &DocumentSegment) -> String {
    let mut out = String::new();
    for el in address_elements(seg) {
        out.push_str(&format!("[{}] {}\n", el.number, el.text));
    }
    out
}

fn table_preview(t: &crate::types::DocumentTable) -> String {
    let Some(first_row) = t.rows.first() else {
        return "[empty table]".to_string();
    };
    let preview = first_row
        .cells
        .iter()
        .map(|c| c.text.trim_end_matches('\n').trim())
        .collect::<Vec<_>>()
        .join(" | ");
    if preview.len() > 60 {
        format!("{}...", &preview[..57])
    } else {
        preview
    }
}

pub fn resolve_address(address: &Address, elements: &[AddressElement]) -> Result<Vec<AddressElement>> {
    if elements.is_empty() {
        return Err(SedmatError::new("document has no paragraphs"));
    }
    let last = elements.len() as i32;
    let start = if address.start == -1 { last } else { address.start };
    if start < 1 || start > last {
        return Err(SedmatError::new(format!(
            "address {start} out of range (document has {last} paragraphs)"
        )));
    }
    if !address.has_range {
        return Ok(vec![elements[(start - 1) as usize].clone()]);
    }
    let end = if address.end == 0 {
        start
    } else if address.end == -1 {
        last
    } else {
        address.end
    };
    if end < 1 || end > last {
        return Err(SedmatError::new(format!(
            "address end {end} out of range (document has {last} paragraphs)"
        )));
    }
    if end < start {
        return Err(SedmatError::new(format!(
            "address range {start},{end} is reversed"
        )));
    }
    Ok(elements[(start - 1) as usize..end as usize].to_vec())
}

fn is_contiguous_terminal_range(elements: &[AddressElement], targets: &[AddressElement]) -> bool {
    if targets.len() < 2 || targets.last().map(|t| t.number) != Some(elements.len()) {
        return false;
    }
    for w in targets.windows(2) {
        if w[1].number != w[0].number + 1 {
            return false;
        }
    }
    true
}

pub fn plan_addressed_delete(elements: &[AddressElement], targets: &[AddressElement]) -> Vec<AddressMutation> {
    if targets.is_empty() {
        return Vec::new();
    }
    if is_contiguous_terminal_range(elements, targets) {
        return vec![AddressMutation {
            start_index: targets[0].start_index,
            end_index: targets.last().unwrap().end_index - 1,
            insert_text: None,
        }];
    }
    let mut mutations = Vec::with_capacity(targets.len());
    for target in targets {
        let mut start = target.start_index;
        let mut end = target.end_index;
        let is_last = target.number == elements.len();
        if is_last && target.number > 1 {
            start = elements[target.number - 2].end_index - 1;
            end = target.end_index - 1;
        } else if is_last && target.number == 1 {
            if target.start_index >= target.end_index.saturating_sub(1) {
                continue;
            }
            end = target.end_index - 1;
        }
        mutations.push(AddressMutation {
            start_index: start,
            end_index: end,
            insert_text: None,
        });
    }
    mutations
}

pub fn plan_addressed_insert(targets: &[AddressElement], replacement: &str) -> Vec<AddressMutation> {
    let mut text = replacement.replace("\\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    targets
        .iter()
        .map(|t| AddressMutation {
            start_index: t.start_index,
            end_index: t.start_index,
            insert_text: Some(text.clone()),
        })
        .collect()
}

pub fn plan_addressed_append(targets: &[AddressElement], replacement: &str) -> Vec<AddressMutation> {
    let raw = replacement.replace("\\n", "\n");
    let text = if let Some(stripped) = raw.strip_suffix('\n') {
        format!("\n{stripped}")
    } else {
        format!("\n{raw}")
    };
    targets
        .iter()
        .map(|t| AddressMutation {
            start_index: t.end_index.saturating_sub(1),
            end_index: t.end_index.saturating_sub(1),
            insert_text: Some(text.clone()),
        })
        .collect()
}

pub fn plan_paragraph_substitutions(
    paragraphs: &[DocumentParagraph],
    expr: &Expression,
) -> Result<TextPlan> {
    let re = SedRegex::compile(&expr.pattern)?;
    let mut plan = TextPlan::default();
    let mut text_delta: isize = 0;

    for para in paragraphs {
        if para.text.is_empty() {
            continue;
        }
        let limit = if expr.nth_match > 0 {
            Some(expr.nth_match)
        } else if expr.global {
            None
        } else {
            Some(1)
        };
        let mut matches = re.find_all(&para.text, limit);
        if expr.nth_match > 0 {
            if matches.len() < expr.nth_match {
                continue;
            }
            matches = vec![matches[expr.nth_match - 1].clone()];
        }
        let chars: Vec<char> = para.text.chars().collect();
        for m in matches {
            let old_text: String = chars[m.start..m.end].iter().collect();
            let mut expanded = expand_replacement(&expr.replacement, &chars, &m);
            if expanded.is_empty()
                && expr.replacement.is_empty()
                && expr.brace.as_ref().is_some_and(|b| {
                    !b.has_break && !b.has_toc && b.img_ref.is_none() && b.table_ref.is_none()
                })
            {
                expanded = old_text.clone();
            }
            let prefix_u16: usize = chars[..m.start].iter().map(|c| c.len_utf16()).sum();
            let match_u16: usize = chars[m.start..m.end].iter().map(|c| c.len_utf16()).sum();
            let start = para.start_index + prefix_u16;
            let end = start + match_u16;

            plan.match_count += 1;
            plan.text_edits.push(TextEdit {
                start_index: start,
                end_index: end,
                insert_text: expanded.clone(),
            });
            let ins_u16 = utf16_len(&expanded);
            if expr.brace.is_some() && !expanded.is_empty() {
                let f_start = (start as isize + text_delta) as usize;
                plan.formatting.push(FormatIntent {
                    start_index: f_start,
                    end_index: f_start + ins_u16,
                    formats: Vec::new(),
                    brace: expr.brace.clone(),
                });
            }
            text_delta += ins_u16 as isize - match_u16 as isize;
        }
    }
    Ok(plan)
}
