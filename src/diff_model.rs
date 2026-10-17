use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

pub const ROW_LIMIT: usize = 2000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Section,
    File,
    Hunk,
    Context,
    Added,
    Removed,
    Note,
}
#[derive(Clone, Debug)]
pub struct DiffRow {
    pub kind: RowKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
    pub highlights: Vec<Range<usize>>,
}
impl DiffRow {
    pub fn label(kind: RowKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            old_line: None,
            new_line: None,
            highlights: Vec::new(),
        }
    }
    fn code(kind: RowKind, number: usize, text: &str, highlights: Vec<Range<usize>>) -> Self {
        Self {
            kind,
            old_line: (kind == RowKind::Removed).then_some(number + 1),
            new_line: (kind == RowKind::Added).then_some(number + 1),
            text: text.into(),
            highlights,
        }
    }
}
#[derive(Default)]
pub struct RenderedDiff {
    pub text: String,
    pub rows: Vec<DiffRow>,
}
impl RenderedDiff {
    pub fn section(&mut self, title: &str, mut diff: RenderedDiff) {
        if !diff.rows.is_empty() {
            self.rows.push(DiffRow::label(RowKind::Section, title));
            self.rows.append(&mut diff.rows);
        }
    }
    pub fn limit(&mut self) {
        for row in &mut self.rows {
            if row.text.len() > 8192 {
                let mut end = 8192;
                while !row.text.is_char_boundary(end) {
                    end -= 1;
                }
                row.text.truncate(end);
                row.highlights = row
                    .highlights
                    .iter()
                    .filter(|range| range.start < end)
                    .map(|range| range.start..range.end.min(end))
                    .collect();
                row.text.push_str(" … [line truncated]");
            }
            if row.text.contains('\t') {
                let mut expanded = String::new();
                let mut positions = vec![0; row.text.len() + 1];
                let mut column = 0;
                for (byte, character) in row.text.char_indices() {
                    positions[byte] = expanded.len();
                    if character == '\t' {
                        let spaces = 4 - column % 4;
                        expanded.extend(std::iter::repeat_n(' ', spaces));
                        column += spaces;
                    } else {
                        expanded.push(character);
                        column += 1;
                    }
                }
                positions[row.text.len()] = expanded.len();
                row.highlights = row
                    .highlights
                    .iter()
                    .map(|range| positions[range.start]..positions[range.end])
                    .collect();
                row.text = expanded;
            }
        }
        if self.rows.len() > ROW_LIMIT {
            self.rows.truncate(ROW_LIMIT);
            self.rows.push(DiffRow::label(
                RowKind::Note,
                "Preview limited to 2,000 lines.",
            ));
        }
    }
}

// All parsing, alignment and word matching happens on the feature worker.
pub fn from_difft(json: &str, path: &str, old: &[u8], new: &[u8]) -> Result<Vec<DiffRow>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let status = value["status"].as_str().ok_or("Missing difft status")?;
    if status == "unchanged" {
        if old != new {
            return Err("Formatting-only changes need the line diff".into());
        }
        return Ok(Vec::new());
    }
    if !matches!(status, "changed" | "created" | "deleted") {
        return Err("Unsupported difft status".into());
    }
    let mut rows = vec![DiffRow::label(
        RowKind::File,
        format!(
            "{path}  ·  {}",
            value["language"].as_str().unwrap_or("Text")
        ),
    )];
    if old.contains(&0) || new.contains(&0) {
        rows.push(DiffRow::label(RowKind::Note, "Binary file changed."));
        return Ok(rows);
    }
    let old_text = std::str::from_utf8(old).map_err(|e| e.to_string())?;
    let new_text = std::str::from_utf8(new).map_err(|e| e.to_string())?;
    let lhs: Vec<_> = old_text.lines().collect();
    let rhs: Vec<_> = new_text.lines().collect();
    if status == "created" || status == "deleted" {
        let (lines, kind) = if status == "created" {
            (&rhs, RowKind::Added)
        } else {
            (&lhs, RowKind::Removed)
        };
        for (i, line) in lines.iter().take(ROW_LIMIT).enumerate() {
            rows.push(DiffRow::code(
                kind,
                i,
                line,
                if line.is_empty() {
                    vec![]
                } else {
                    vec![0..line.len()]
                },
            ));
        }
        if lines.len() > ROW_LIMIT {
            rows.push(DiffRow::label(
                RowKind::Note,
                "Preview limited to 2,000 lines.",
            ));
        }
        return Ok(rows);
    }
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    for entry in value["chunks"]
        .as_array()
        .ok_or("Missing difft chunks")?
        .iter()
        .flat_map(|c| c.as_array().into_iter().flatten())
    {
        for (side, map, lines) in [("lhs", &mut left, &lhs), ("rhs", &mut right, &rhs)] {
            if entry[side].is_null() {
                continue;
            }
            let number = entry[side]["line_number"]
                .as_u64()
                .ok_or("Invalid line number")? as usize;
            let line = lines.get(number).ok_or("Line outside source")?;
            let mut ranges = Vec::new();
            for change in entry[side]["changes"]
                .as_array()
                .ok_or("Invalid difft ranges")?
            {
                let start = change["start"].as_u64().ok_or("Invalid start")? as usize;
                let end = change["end"].as_u64().ok_or("Invalid end")? as usize;
                if start > end
                    || end > line.len()
                    || !line.is_char_boundary(start)
                    || !line.is_char_boundary(end)
                {
                    return Err("Invalid UTF-8 range".into());
                }
                if start < end {
                    ranges.push(start..end);
                }
            }
            map.insert(number, ranges);
        }
    }
    let alignment = value["aligned_lines"]
        .as_array()
        .ok_or("Missing line alignment")?;
    let mut visible = BTreeSet::new();
    for (i, pair) in alignment.iter().enumerate() {
        if pair[0]
            .as_u64()
            .is_some_and(|n| left.contains_key(&(n as usize)))
            || pair[1]
                .as_u64()
                .is_some_and(|n| right.contains_key(&(n as usize)))
        {
            visible.extend(i.saturating_sub(3)..(i + 4).min(alignment.len()));
        }
    }
    let mut previous = None;
    for i in visible {
        if rows.len() >= ROW_LIMIT {
            rows.push(DiffRow::label(
                RowKind::Note,
                "Preview limited to 2,000 lines.",
            ));
            break;
        }
        let pair = &alignment[i];
        let l = pair[0]
            .as_u64()
            .map(|n| n as usize)
            .filter(|n| *n < lhs.len());
        let r = pair[1]
            .as_u64()
            .map(|n| n as usize)
            .filter(|n| *n < rhs.len());
        if previous.is_none_or(|p| i != p + 1) {
            rows.push(DiffRow::label(
                RowKind::Hunk,
                format!(
                    "@@ old {} · new {} @@",
                    l.map(|n| n + 1).unwrap_or(0),
                    r.map(|n| n + 1).unwrap_or(0)
                ),
            ));
        }
        previous = Some(i);
        let changed =
            l.is_some_and(|n| left.contains_key(&n)) || r.is_some_and(|n| right.contains_key(&n));
        if changed {
            let (mut l_ranges, mut r_ranges) = (
                l.and_then(|n| left.get(&n)).cloned().unwrap_or_default(),
                r.and_then(|n| right.get(&n)).cloned().unwrap_or_default(),
            );
            if value["language"].as_str() == Some("Text")
                && let (Some(l), Some(r)) = (l, r)
            {
                (l_ranges, r_ranges) = word_ranges(lhs[l], rhs[r]);
            }
            if let Some(n) = l {
                rows.push(DiffRow::code(
                    RowKind::Removed,
                    n,
                    lhs[n],
                    normalize(l_ranges),
                ));
            }
            if let Some(n) = r {
                rows.push(DiffRow::code(
                    RowKind::Added,
                    n,
                    rhs[n],
                    normalize(r_ranges),
                ));
            }
        } else if let Some(n) = r {
            rows.push(DiffRow {
                kind: RowKind::Context,
                old_line: l.map(|n| n + 1),
                new_line: Some(n + 1),
                text: rhs[n].into(),
                highlights: vec![],
            });
        }
    }
    Ok(rows)
}

fn normalize(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(last) = out.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }
        out.push(range);
    }
    out
}
pub fn word_ranges(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    if old.len() + new.len() > 8192 {
        return (vec![0..old.len()], vec![0..new.len()]);
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut l = 0;
    let mut r = 0;
    for change in similar::TextDiff::from_words(old, new).iter_all_changes() {
        let len = change.value().len();
        match change.tag() {
            similar::ChangeTag::Delete => {
                left.push(l..l + len);
                l += len;
            }
            similar::ChangeTag::Insert => {
                right.push(r..r + len);
                r += len;
            }
            similar::ChangeTag::Equal => {
                l += len;
                r += len;
            }
        }
    }
    (normalize(left), normalize(right))
}

pub fn from_unified(text: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut old = 0;
    let mut new = 0;
    let mut in_hunk = false;
    for line in text.lines() {
        if rows.len() >= ROW_LIMIT {
            rows.push(DiffRow::label(
                RowKind::Note,
                "Preview limited to 2,000 lines.",
            ));
            break;
        }
        if line.starts_with("diff --git ") {
            in_hunk = false;
            rows.push(DiffRow::label(
                RowKind::File,
                line.trim_start_matches("diff --git "),
            ));
            continue;
        }
        if line.starts_with("@@ ") {
            let mut parts = line.split_whitespace();
            parts.next();
            old = parts
                .next()
                .and_then(|p| p.trim_start_matches('-').split(',').next()?.parse().ok())
                .unwrap_or(0);
            new = parts
                .next()
                .and_then(|p| p.trim_start_matches('+').split(',').next()?.parse().ok())
                .unwrap_or(0);
            in_hunk = true;
            rows.push(DiffRow::label(RowKind::Hunk, line));
            continue;
        }
        if in_hunk && let Some(prefix) = line.chars().next() {
            let kind = match prefix {
                '-' => RowKind::Removed,
                '+' => RowKind::Added,
                ' ' => RowKind::Context,
                _ => RowKind::Note,
            };
            if kind != RowKind::Note {
                let content = line[1..].to_owned();
                rows.push(DiffRow {
                    kind,
                    old_line: (kind != RowKind::Added).then_some(old),
                    new_line: (kind != RowKind::Removed).then_some(new),
                    text: content,
                    highlights: vec![],
                });
                if kind != RowKind::Added {
                    old += 1;
                }
                if kind != RowKind::Removed {
                    new += 1;
                }
                continue;
            }
        }
        if !line.is_empty()
            && !line.starts_with("index ")
            && !line.starts_with("--- ")
            && !line.starts_with("+++ ")
            && line != "No changes."
        {
            rows.push(DiffRow::label(RowKind::Note, line));
        }
    }
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind != RowKind::Removed {
            i += 1;
            continue;
        }
        let first = i;
        while i < rows.len() && rows[i].kind == RowKind::Removed {
            i += 1;
        }
        let added = i;
        while i < rows.len() && rows[i].kind == RowKind::Added {
            i += 1;
        }
        for offset in 0..(added - first).min(i - added) {
            let (left, right) = word_ranges(&rows[first + offset].text, &rows[added + offset].text);
            rows[first + offset].highlights = left;
            rows[added + offset].highlights = right;
        }
        for row in &mut rows[first..i] {
            if row.highlights.is_empty() && !row.text.is_empty() {
                row.highlights.push(0..row.text.len());
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_has_real_line_numbers_and_word_highlights() {
        let rows = from_unified(
            "diff --git a/test.rs b/test.rs\n--- a/test.rs\n+++ b/test.rs\n@@ -8,2 +8,2 @@\n-let 名称 = old_value + keep;\n+let 名称 = new_value + keep;\n unchanged\n",
        );
        let removed = rows.iter().find(|r| r.kind == RowKind::Removed).unwrap();
        let added = rows.iter().find(|r| r.kind == RowKind::Added).unwrap();
        assert_eq!((removed.old_line, removed.new_line), (Some(8), None));
        assert_eq!((added.old_line, added.new_line), (None, Some(8)));
        assert_eq!(&removed.text[removed.highlights[0].clone()], "old_value");
        assert_eq!(&added.text[added.highlights[0].clone()], "new_value");
        let context = rows.iter().find(|r| r.kind == RowKind::Context).unwrap();
        assert_eq!((context.old_line, context.new_line), (Some(9), Some(9)));
    }
    #[test]
    fn difft_maps_structural_ranges_and_context_to_source_lines() {
        let json = serde_json::json!({"status":"changed", "language":"Rust", "aligned_lines":[[0,0],[1,1]], "chunks":[[{"lhs":{"line_number":0,"changes":[{"start":4,"end":7}]},"rhs":{"line_number":0,"changes":[{"start":4,"end":7}]}}]]});
        let rows = from_difft(
            &json.to_string(),
            "a.rs",
            b"let old = 1;\nkeep\n",
            b"let new = 1;\nkeep\n",
        )
        .unwrap();
        let old = rows.iter().find(|r| r.kind == RowKind::Removed).unwrap();
        let new = rows.iter().find(|r| r.kind == RowKind::Added).unwrap();
        assert_eq!(old.old_line, Some(1));
        assert_eq!(new.new_line, Some(1));
        assert_eq!(&old.text[old.highlights[0].clone()], "old");
        assert_eq!(&new.text[new.highlights[0].clone()], "new");
        assert!(
            rows.iter()
                .any(|r| r.kind == RowKind::Context && r.text == "keep")
        );
    }
    #[test]
    fn empty_sections_are_omitted_and_tabs_preserve_highlight_ranges() {
        let mut document = RenderedDiff::default();
        document.section(
            "Staged",
            RenderedDiff {
                text: "No changes.".into(),
                rows: vec![],
            },
        );
        assert!(document.rows.is_empty());
        document.section(
            "Working tree",
            RenderedDiff {
                text: String::new(),
                rows: vec![DiffRow::code(RowKind::Added, 0, "\t名称", vec![1..7])],
            },
        );
        document.limit();
        let row = &document.rows[1];
        assert_eq!(row.text, "    名称");
        assert_eq!(&row.text[row.highlights[0].clone()], "名称");
    }
    #[test]
    fn malformed_difft_ranges_are_rejected_and_binary_files_are_notes() {
        let invalid = r#"{"status":"changed","language":"Rust","chunks":[[{"lhs":{"line_number":0,"changes":[{"start":0,"end":1}]}}]],"aligned_lines":[[0,null]]}"#;
        assert!(from_difft(invalid, "a.rs", "名称".as_bytes(), b"").is_err());
        let rows = from_difft(r#"{"status":"created"}"#, "a.bin", b"", &[0, 1]).unwrap();
        assert_eq!(rows[1].kind, RowKind::Note);
        assert!(rows[1].text.contains("Binary"));
    }
}
