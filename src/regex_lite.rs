use crate::error::{Result, SedmatError};

#[derive(Debug, Clone)]
enum Atom {
    Literal(char),
    Dot,
    Digit,
    Word,
    Space,
    Class { negated: bool, ranges: Vec<(char, char)> },
    AnchorStart,
    AnchorEnd,
    Group(usize, Vec<Vec<Node>>),
}

#[derive(Debug, Clone)]
struct Node {
    atom: Atom,
    min: usize,
    max: Option<usize>, // None = unbounded
}

/// Lightweight pure-Rust regex engine supporting ERE subset used in SEDMAT
/// (`(?i)`, `(?m)`, `^`, `$`, `.`, `\d`, `\w`, `\s`, `[a-z]`, `(...)`, `|`, `*`, `+`, `?`).
#[derive(Debug, Clone)]
pub struct SedRegex {
    case_insensitive: bool,
    multiline: bool,
    branches: Vec<Vec<Node>>,
    num_groups: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub start: usize, // char index
    pub end: usize,   // char index
    pub groups: Vec<Option<(usize, usize)>>,
}

impl SedRegex {
    pub fn compile(pattern: &str) -> Result<Self> {
        let mut s = pattern;
        let mut case_insensitive = false;
        let mut multiline = false;
        loop {
            if let Some(rest) = s.strip_prefix("(?i)") {
                case_insensitive = true;
                s = rest;
            } else if let Some(rest) = s.strip_prefix("(?m)") {
                multiline = true;
                s = rest;
            } else if let Some(rest) = s.strip_prefix("(?im)").or_else(|| s.strip_prefix("(?mi)")) {
                case_insensitive = true;
                multiline = true;
                s = rest;
            } else {
                break;
            }
        }
        let chars: Vec<char> = s.chars().collect();
        let mut pos = 0;
        let mut num_groups = 0;
        let branches = parse_alt(&chars, &mut pos, &mut num_groups, false)?;
        Ok(Self {
            case_insensitive,
            multiline,
            branches,
            num_groups,
        })
    }

    pub fn is_match(&self, text: &str) -> bool {
        !self.find_all(text, Some(1)).is_empty()
    }

    pub fn find_all(&self, text: &str, limit: Option<usize>) -> Vec<Match> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut idx = 0;
        while idx <= chars.len() {
            let mut groups = vec![None; self.num_groups + 1];
            if let Some(end) = self.match_alt(&self.branches, &chars, idx, &mut groups) {
                groups[0] = Some((idx, end));
                out.push(Match {
                    start: idx,
                    end,
                    groups,
                });
                if let Some(lim) = limit {
                    if out.len() >= lim {
                        break;
                    }
                }
                idx = if end > idx { end } else { idx + 1 };
            } else {
                idx += 1;
            }
        }
        out
    }

    fn match_alt(
        &self,
        branches: &[Vec<Node>],
        chars: &[char],
        pos: usize,
        groups: &mut Vec<Option<(usize, usize)>>,
    ) -> Option<usize> {
        for seq in branches {
            let mut g_copy = groups.clone();
            if let Some(end) = self.match_seq(seq, 0, chars, pos, &mut g_copy) {
                *groups = g_copy;
                return Some(end);
            }
        }
        None
    }

    fn match_seq(
        &self,
        seq: &[Node],
        ni: usize,
        chars: &[char],
        pos: usize,
        groups: &mut Vec<Option<(usize, usize)>>,
    ) -> Option<usize> {
        if ni >= seq.len() {
            return Some(pos);
        }
        let node = &seq[ni];
        // Collect all valid match end positions for repeated atom (greedy: try longest first)
        let mut ends: Vec<(usize, Vec<Option<(usize, usize)>>)> = Vec::new();
        let mut cur_pos = pos;
        let mut cur_groups = groups.clone();
        if node.min == 0 {
            ends.push((cur_pos, cur_groups.clone()));
        }
        let mut count = 0;
        while node.max.map_or(true, |m| count < m) {
            let mut next_g = cur_groups.clone();
            if let Some(next_pos) = self.match_atom(&node.atom, chars, cur_pos, &mut next_g) {
                count += 1;
                if next_pos == cur_pos && count > node.min {
                    break;
                }
                cur_pos = next_pos;
                cur_groups = next_g;
                if count >= node.min {
                    ends.push((cur_pos, cur_groups.clone()));
                }
            } else {
                break;
            }
        }
        for (end_pos, mut g) in ends.into_iter().rev() {
            if let Some(final_pos) = self.match_seq(seq, ni + 1, chars, end_pos, &mut g) {
                *groups = g;
                return Some(final_pos);
            }
        }
        None
    }

    fn match_atom(
        &self,
        atom: &Atom,
        chars: &[char],
        pos: usize,
        groups: &mut Vec<Option<(usize, usize)>>,
    ) -> Option<usize> {
        match atom {
            Atom::AnchorStart => {
                if pos == 0 || (self.multiline && pos > 0 && chars[pos - 1] == '\n') {
                    Some(pos)
                } else {
                    None
                }
            }
            Atom::AnchorEnd => {
                if pos == chars.len() || (self.multiline && chars[pos] == '\n') {
                    Some(pos)
                } else {
                    None
                }
            }
            Atom::Group(gid, branches) => {
                let end = self.match_alt(branches, chars, pos, groups)?;
                if *gid < groups.len() {
                    groups[*gid] = Some((pos, end));
                }
                Some(end)
            }
            _ => {
                let ch = *chars.get(pos)?;
                let matched = match atom {
                    Atom::Literal(c) => {
                        if self.case_insensitive {
                            ch.to_lowercase().eq(c.to_lowercase())
                        } else {
                            ch == *c
                        }
                    }
                    Atom::Dot => ch != '\n',
                    Atom::Digit => ch.is_ascii_digit(),
                    Atom::Word => ch.is_ascii_alphanumeric() || ch == '_',
                    Atom::Space => ch.is_whitespace(),
                    Atom::Class { negated, ranges } => {
                        let in_set = ranges.iter().any(|&(lo, hi)| {
                            if self.case_insensitive {
                                let c = ch.to_ascii_lowercase();
                                c >= lo.to_ascii_lowercase() && c <= hi.to_ascii_lowercase()
                            } else {
                                ch >= lo && ch <= hi
                            }
                        });
                        if *negated { !in_set } else { in_set }
                    }
                    Atom::AnchorStart | Atom::AnchorEnd | Atom::Group(_, _) => unreachable!(),
                };
                if matched { Some(pos + 1) } else { None }
            }
        }
    }
}

pub fn expand_replacement(template: &str, text_chars: &[char], m: &Match) -> String {
    let slice_group = |idx: usize| -> String {
        m.groups
            .get(idx)
            .and_then(|g| *g)
            .map(|(s, e)| text_chars[s..e].iter().collect())
            .unwrap_or_default()
    };
    let t: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < t.len() {
        let ch = t[i];
        if ch == '\\' && i + 1 < t.len() {
            let next = t[i + 1];
            match next {
                '&' => {
                    out.push('&');
                    i += 2;
                    continue;
                }
                'n' => {
                    out.push('\n');
                    i += 2;
                    continue;
                }
                '1'..='9' => {
                    out.push_str(&slice_group((next as usize) - ('0' as usize)));
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        if ch == '&' {
            out.push_str(&slice_group(0));
            i += 1;
            continue;
        }
        if ch == '$' && i + 1 < t.len() {
            let next = t[i + 1];
            if next == '$' {
                out.push('$');
                i += 2;
                continue;
            }
            if ('0'..='9').contains(&next) {
                out.push_str(&slice_group((next as usize) - ('0' as usize)));
                i += 2;
                continue;
            }
            if next == '{' {
                if let Some(close) = t[i + 2..].iter().position(|&c| c == '}') {
                    let num_str: String = t[i + 2..i + 2 + close].iter().collect();
                    if let Ok(idx) = num_str.parse::<usize>() {
                        out.push_str(&slice_group(idx));
                        i = i + 2 + close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn parse_alt(chars: &[char], pos: &mut usize, num_groups: &mut usize, in_group: bool) -> Result<Vec<Vec<Node>>> {
    let mut branches = Vec::new();
    loop {
        branches.push(parse_seq(chars, pos, num_groups, in_group)?);
        if *pos < chars.len() && chars[*pos] == '|' {
            *pos += 1;
        } else {
            break;
        }
    }
    Ok(branches)
}

fn parse_seq(chars: &[char], pos: &mut usize, num_groups: &mut usize, in_group: bool) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    while *pos < chars.len() {
        let ch = chars[*pos];
        if ch == '|' || (in_group && ch == ')') {
            break;
        }
        let atom = match ch {
            '^' => {
                *pos += 1;
                Atom::AnchorStart
            }
            '$' => {
                *pos += 1;
                Atom::AnchorEnd
            }
            '.' => {
                *pos += 1;
                Atom::Dot
            }
            '\\' => {
                *pos += 1;
                if *pos >= chars.len() {
                    return Err(SedmatError::new("trailing backslash in regex"));
                }
                let esc = chars[*pos];
                *pos += 1;
                match esc {
                    'd' => Atom::Digit,
                    'w' => Atom::Word,
                    's' => Atom::Space,
                    'n' => Atom::Literal('\n'),
                    't' => Atom::Literal('\t'),
                    other => Atom::Literal(other),
                }
            }
            '[' => parse_class(chars, pos)?,
            '(' => {
                *pos += 1;
                *num_groups += 1;
                let gid = *num_groups;
                let inner = parse_alt(chars, pos, num_groups, true)?;
                if *pos >= chars.len() || chars[*pos] != ')' {
                    return Err(SedmatError::new("unclosed '(' in regex"));
                }
                *pos += 1;
                Atom::Group(gid, inner)
            }
            other => {
                *pos += 1;
                Atom::Literal(other)
            }
        };

        let (min, max) = if *pos < chars.len() {
            match chars[*pos] {
                '*' => {
                    *pos += 1;
                    (0, None)
                }
                '+' => {
                    *pos += 1;
                    (1, None)
                }
                '?' => {
                    *pos += 1;
                    (0, Some(1))
                }
                _ => (1, Some(1)),
            }
        } else {
            (1, Some(1))
        };

        nodes.push(Node { atom, min, max });
    }
    Ok(nodes)
}

fn parse_class(chars: &[char], pos: &mut usize) -> Result<Atom> {
    *pos += 1; // skip '['
    let negated = if *pos < chars.len() && chars[*pos] == '^' {
        *pos += 1;
        true
    } else {
        false
    };
    let mut ranges = Vec::new();
    while *pos < chars.len() && chars[*pos] != ']' {
        let c = if chars[*pos] == '\\' && *pos + 1 < chars.len() {
            *pos += 2;
            chars[*pos - 1]
        } else {
            let c = chars[*pos];
            *pos += 1;
            c
        };
        if *pos + 1 < chars.len() && chars[*pos] == '-' && chars[*pos + 1] != ']' {
            *pos += 1;
            let hi = chars[*pos];
            *pos += 1;
            ranges.push((c, hi));
        } else {
            ranges.push((c, c));
        }
    }
    if *pos >= chars.len() || chars[*pos] != ']' {
        return Err(SedmatError::new("unclosed '[' in regex"));
    }
    *pos += 1;
    Ok(Atom::Class { negated, ranges })
}
