//! Cursor movement, row visibility, span arithmetic over the tree, and the
//! structural array operations (append / delete) built on top of them.

use serde_json::Value;

use crate::model::SchemaNode;

use super::FormState;
use super::schema::push_array_item;
use super::types::{RowKind, RowState};

impl FormState {
    /// Rows hidden because an ancestor header is Excluded/Null or collapsed.
    pub fn hidden_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; self.rows.len()];
        let mut i = 0;
        while i < self.rows.len() {
            let row = &self.rows[i];
            let header = matches!(row.kind, RowKind::ObjectHeader | RowKind::ArrayHeader);
            let hide_children = header
                && (row.collapsed
                    || matches!(
                        row.state,
                        RowState::Excluded | RowState::Null | RowState::Empty
                    ));
            if hide_children {
                let end = self.span_end(i);
                for slot in mask.iter_mut().take(end).skip(i + 1) {
                    *slot = true;
                }
                i = end;
            } else {
                i += 1;
            }
        }
        mask
    }

    pub fn move_cursor(&mut self, delta: i64) {
        let hidden = self.hidden_mask();
        let mut i = self.cursor as i64;
        loop {
            i += delta;
            if i < 0 || i >= self.rows.len() as i64 {
                return; // stay put at edges
            }
            let idx = i as usize;
            if self.rows[idx].interactive() && !hidden[idx] {
                self.cursor = idx;
                return;
            }
        }
    }

    pub(super) fn clamp_cursor_to_interactive(&mut self, direction: i64) {
        let hidden = self.hidden_mask();
        if self
            .rows
            .get(self.cursor)
            .map(|r| r.interactive() && !hidden[self.cursor])
            .unwrap_or(false)
        {
            return;
        }
        self.cursor = 0;
        if !self.rows.is_empty() && (!self.rows[0].interactive() || hidden[0]) {
            self.move_cursor(direction);
        }
    }

    /// Index one past the last descendant of the row at `i`.
    pub fn span_end(&self, i: usize) -> usize {
        let row = &self.rows[i];
        if !matches!(row.kind, RowKind::ObjectHeader | RowKind::ArrayHeader) {
            return i + 1;
        }
        let mut j = i + 1;
        while j < self.rows.len()
            && self.rows[j].section == row.section
            && self.rows[j].kind != RowKind::SectionHeader
            && self.rows[j].depth > row.depth
        {
            j += 1;
        }
        j
    }

    /// Direct child row indices of the header at `i` (depth == header+1).
    pub(super) fn direct_children(&self, i: usize) -> Vec<usize> {
        let depth = self.rows[i].depth + 1;
        (i + 1..self.span_end(i))
            .filter(|&j| self.rows[j].depth == depth)
            .collect()
    }

    /// For a row inside an array, find (array_header_idx, item_root_idx).
    fn enclosing_array_item(&self, i: usize) -> Option<(usize, usize)> {
        // Walk backwards for the nearest ArrayHeader ancestor.
        for header in (0..=i).rev() {
            if self.rows[header].kind == RowKind::ArrayHeader
                && self.span_end(header) > i
                && self.rows[header].depth < self.rows[i].depth
            {
                let item_root = self
                    .direct_children(header)
                    .into_iter()
                    .take_while(|&j| j <= i)
                    .last()?;
                return Some((header, item_root));
            }
        }
        None
    }

    /// Append an item to the array header at `i`.
    pub fn array_append(&mut self, i: usize) {
        let RowKind::ArrayHeader = self.rows[i].kind else {
            return;
        };
        let SchemaNode::Array { item, .. } = self.rows[i].schema.clone() else {
            return;
        };
        let end = self.span_end(i);
        let count = self.direct_children(i).len();
        let depth = self.rows[i].depth + 1;
        let mut new_rows = Vec::new();
        push_array_item(&mut new_rows, &item, count, depth, None);
        self.rows.splice(end..end, new_rows);
        if matches!(
            self.rows[i].state,
            RowState::Empty | RowState::Excluded | RowState::Null
        ) {
            self.rows[i].state = RowState::Filled(Value::Bool(true));
        }
    }

    /// Delete the array item containing row `i` (the row itself if it is an
    /// item root, else its nearest item-root ancestor).
    pub fn array_delete(&mut self, i: usize) {
        let Some((header, item_root)) = self.enclosing_array_item(i) else {
            return;
        };
        let end = self.span_end(item_root);
        self.rows.drain(item_root..end);
        // Renumber labels of the remaining items.
        for (n, idx) in self.direct_children(header).into_iter().enumerate() {
            self.rows[idx].label = format!("[{n}]");
        }
        self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
        self.clamp_cursor_to_interactive(-1);
    }
}
