//! Result grid: spreadsheet-style editing, grid / text / record views, sorting, multi-selection.
use crate::db::{QueryResult, TableMeta};
use crate::icons::{self, Icon};
use crate::theme::pal;
use egui::{Align, CornerRadius, Layout, RichText, Sense, Stroke, StrokeKind, Ui};
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    Text,
    Record,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RowState {
    Clean,
    Modified(Vec<Option<String>>),
    New,
    Deleted,
}

/// Where the rows come from, so that edits can be written back.
#[derive(Clone)]
pub struct EditSource {
    pub conn: String,
    pub schema: String,
    pub table: String,
    pub meta: TableMeta,
    pub qualified: String,
    pub kind: crate::db::Kind,
}

pub enum Undo {
    Cell(usize, usize, Option<String>),
    State(usize, RowState),
    /// the previous N entries were one operation (a block paste)
    Group(usize),
}

/// Aggregates of a set of cells (value panel).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Aggregates {
    pub cells: usize,
    pub nulls: usize,
    pub count: usize,
    pub distinct: usize,
    pub numeric: usize,
    pub sum: Option<f64>,
    pub avg: Option<f64>,
    pub min: Option<String>,
    pub max: Option<String>,
}

pub fn fmt_num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        let t = format!("{:.6}", x);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub struct GridState {
    pub uid: u64,
    pub qr: QueryResult,
    pub title: String,
    pub mode: ViewMode,
    pub sel: Option<(usize, usize)>,
    pub anchor: Option<usize>,
    pub sort: Option<(usize, bool)>, // column, ascending
    pub filter: String,
    pub view_idx: Vec<usize>,
    pub dirty: bool,
    pub editing: Option<(usize, usize, String)>,
    pub row_state: Vec<RowState>,
    pub editable: bool,
    pub edit: Option<EditSource>,
    pub readonly_reason: Option<String>,
    pub numeric: Vec<bool>,
    pub widths: Vec<f32>,
    pub server_sort: bool,
    pub multi_sel: HashSet<usize>,
    /// block of cells selected by drag / Shift+click: ((view position, column), (view position, column))
    pub rect_sel: Option<((usize, usize), (usize, usize))>,
    /// whole columns selected by clicking their titles
    pub sel_cols: std::collections::BTreeSet<usize>,
    pub anchor_col: usize,
    pub col_anchor: Option<usize>,
    pub drag_from: Option<(usize, usize)>,
    pub scroll_to_col: Option<usize>,
    pub undo: Vec<Undo>,
    pub vp_width: f32,
    pub vp_buf: String,
    /// value the buffer was loaded with: when the cell changes elsewhere and the user did not edit the buffer, it follows
    pub vp_orig: String,
    pub vp_stats: bool,
    pub vp_text_h: f32,
    pub vp_stats_col: bool,
    pub vp_for: Option<(usize, usize)>,
    pub scroll_to: Option<usize>,
    pub select_all: bool,
    #[cfg(test)]
    pub cell_rects: Vec<((usize, usize), egui::Rect)>,
}

pub enum GridAction {
    SortBy,
    FilterBy(usize, Option<String>),
    ViewValue(String),
    Changed,
    FetchMore,
}

impl GridState {
    pub fn new(qr: QueryResult, title: &str) -> GridState {
        let n = qr.rows.len();
        let numeric = (0..qr.columns.len())
            .map(|c| {
                let mut seen = false;
                for r in qr.rows.iter().take(100) {
                    if let Some(v) = &r[c] {
                        seen = true;
                        if v.parse::<f64>().is_err() {
                            return false;
                        }
                    }
                }
                seen
            })
            .collect();
        // default width: at most 30 characters (ResultSetPreferences.java:26)
        let widths = (0..qr.columns.len())
            .map(|c| {
                let mut m = qr.columns[c].chars().count() + 3;
                for r in qr.rows.iter().take(60) {
                    m = m.max(r[c].as_ref().map(|s| s.chars().count().min(crate::constants::MAX_DEF_COLUMN_WIDTH)).unwrap_or(6));
                }
                (m.min(crate::constants::MAX_DEF_COLUMN_WIDTH) as f32 * 7.8 + 22.0).max(70.0)
            })
            .collect();
        GridState {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            title: title.to_string(),
            mode: ViewMode::Grid,
            sel: None,
            anchor: None,
            sort: None,
            filter: String::new(),
            view_idx: (0..n).collect(),
            dirty: false,
            editing: None,
            row_state: vec![RowState::Clean; n],
            editable: false,
            edit: None,
            readonly_reason: None,
            numeric,
            widths,
            server_sort: true,
            multi_sel: HashSet::new(),
            rect_sel: None,
            sel_cols: Default::default(),
            anchor_col: 0,
            col_anchor: None,
            drag_from: None,
            scroll_to_col: None,
            undo: Vec::new(),
            vp_width: 280.0,
            vp_buf: String::new(),
            vp_orig: String::new(),
            vp_stats: true,
            vp_text_h: 64.0,
            vp_stats_col: false,
            vp_for: None,
            scroll_to: None,
            select_all: true,
            #[cfg(test)]
            cell_rects: Vec::new(),
            qr,
        }
    }

    pub fn reindex(&mut self) {
        let f = self.filter.to_lowercase();
        let mut idx: Vec<usize> = (0..self.qr.rows.len())
            .filter(|&i| f.is_empty() || self.row_state[i] == RowState::New || self.qr.rows[i].iter().any(|v| v.as_ref().map(|s| s.to_lowercase().contains(&f)).unwrap_or(false)))
            .collect();
        if let (Some((c, asc)), false) = (self.sort, self.server_sort) {
            // sort keys are computed once per row, not once per comparison
            let num = self.numeric.get(c).copied().unwrap_or(false);
            enum Key {
                Null,
                Num(f64),
                Text(String),
            }
            let keys: std::collections::HashMap<usize, Key> = idx
                .iter()
                .map(|&i| {
                    let k = match &self.qr.rows[i][c] {
                        None => Key::Null,
                        Some(v) if num => Key::Num(v.parse::<f64>().unwrap_or(0.0)),
                        Some(v) => Key::Text(v.to_lowercase()),
                    };
                    (i, k)
                })
                .collect();
            idx.sort_by(|a, b| {
                let o = match (&keys[a], &keys[b]) {
                    (Key::Null, Key::Null) => std::cmp::Ordering::Equal,
                    (Key::Null, _) => std::cmp::Ordering::Less,
                    (_, Key::Null) => std::cmp::Ordering::Greater,
                    (Key::Num(x), Key::Num(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    (Key::Text(x), Key::Text(y)) => x.cmp(y),
                    _ => std::cmp::Ordering::Equal,
                };
                if asc {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        self.view_idx = idx;
        self.dirty = false;
    }

    pub fn pending_changes(&self) -> usize {
        self.row_state.iter().filter(|s| **s != RowState::Clean).count()
    }

    fn push_undo(&mut self, u: Undo) {
        self.undo.push(u);
        if self.undo.len() > crate::constants::UNDO_LEVELS {
            self.undo.remove(0);
        }
    }

    pub fn set_cell(&mut self, r: usize, c: usize, v: Option<String>) {
        if self.qr.rows[r][c] == v || self.row_state[r] == RowState::Deleted {
            return;
        }
        self.push_undo(Undo::Cell(r, c, self.qr.rows[r][c].clone()));
        if self.row_state[r] == RowState::Clean {
            self.row_state[r] = RowState::Modified(self.qr.rows[r].clone());
        }
        self.qr.rows[r][c] = v;
        // a modified row whose cells all went back to the original is clean again
        if let RowState::Modified(o) = &self.row_state[r] {
            if *o == self.qr.rows[r] {
                self.row_state[r] = RowState::Clean;
            }
        }
    }

    pub fn undo_last(&mut self) {
        match self.undo.pop() {
            Some(Undo::Cell(r, c, v)) => {
                if r < self.qr.rows.len() {
                    self.qr.rows[r][c] = v;
                    if let RowState::Modified(o) = &self.row_state[r] {
                        if *o == self.qr.rows[r] {
                            self.row_state[r] = RowState::Clean;
                        }
                    }
                }
            }
            Some(Undo::State(r, s)) => {
                if r < self.row_state.len() {
                    self.row_state[r] = s;
                }
            }
            Some(Undo::Group(n)) => {
                for _ in 0..n {
                    self.undo_last();
                }
            }
            None => {}
        }
    }

    pub fn add_row(&mut self, vals: Vec<Option<String>>) {
        let len = self.qr.rows.len();
        let after = crate::constants::NEW_ROWS_AFTER && self.sort.is_none() && self.filter.is_empty();
        let pos = match (after, self.sel) {
            (true, Some((r, _))) if r < len => r + 1,
            _ => len,
        };
        if pos < len {
            // rows below move down: remembered positions are no longer valid
            self.undo.clear();
        }
        self.qr.rows.insert(pos, vals);
        self.row_state.insert(pos, RowState::New);
        self.dirty = true;
        let r = pos;
        self.sel = Some((r, self.sel.map(|s| s.1).unwrap_or(0)));
        self.clear_multi();
        self.scroll_to = Some(r);
        self.filter.clear();
    }

    /// Booleans toggle on click: true -> false -> NULL (when the column allows it) -> true
    /// (SpreadsheetPresentation.java:1499-1510).
    pub fn toggle_bool(&mut self, c: usize, nullable: bool, rows: &[usize]) {
        let style = bool_style(self, c);
        for &r in rows {
            if r >= self.qr.rows.len() {
                continue;
            }
            let next = match parse_bool(&self.qr.rows[r][c]) {
                Some(true) => Some(false),
                Some(false) => {
                    if nullable {
                        None
                    } else {
                        Some(true)
                    }
                }
                None => Some(true),
            };
            self.set_cell(r, c, next.map(|b| format_bool(style, b)));
        }
    }

    /// Rows the user is working on: the multi-selection or the current row.
    pub fn target_rows(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.multi_sel.iter().copied().collect();
        if v.is_empty() {
            if let Some((r, _)) = self.sel {
                v.push(r);
            }
        }
        v.sort();
        v
    }

    /// Delete (or restore) rows; a row that was never saved is simply dropped.
    pub fn toggle_delete(&mut self, rows: &[usize]) {
        let mut drop_rows: Vec<usize> = Vec::new();
        for &r in rows {
            if r >= self.row_state.len() {
                continue;
            }
            match self.row_state[r].clone() {
                RowState::New => drop_rows.push(r),
                RowState::Deleted => {
                    self.push_undo(Undo::State(r, RowState::Deleted));
                    self.row_state[r] = RowState::Clean;
                }
                s => {
                    // a modified row that is deleted keeps its original values for the WHERE clause
                    if let RowState::Modified(o) = &s {
                        self.qr.rows[r] = o.clone();
                    }
                    self.push_undo(Undo::State(r, s));
                    self.row_state[r] = RowState::Deleted;
                }
            }
        }
        drop_rows.sort();
        for r in drop_rows.into_iter().rev() {
            self.qr.rows.remove(r);
            self.row_state.remove(r);
        }
        self.clear_multi();
        self.dirty = true;
        if let Some((r, c)) = self.sel {
            if r >= self.qr.rows.len() {
                self.sel = if self.qr.rows.is_empty() { None } else { Some((self.qr.rows.len() - 1, c)) };
            }
        }
    }

    pub fn revert(&mut self) {
        let mut keep = Vec::new();
        for (i, st) in self.row_state.clone().into_iter().enumerate() {
            match st {
                RowState::New => {}
                RowState::Modified(o) => keep.push((o, RowState::Clean)),
                RowState::Deleted | RowState::Clean => keep.push((self.qr.rows[i].clone(), RowState::Clean)),
            }
        }
        self.qr.rows = keep.iter().map(|k| k.0.clone()).collect();
        self.row_state = keep.into_iter().map(|k| k.1).collect();
        self.editing = None;
        self.undo.clear();
        self.clear_multi();
        if let Some((r, c)) = self.sel {
            if r >= self.qr.rows.len() {
                self.sel = if self.qr.rows.is_empty() { None } else { Some((self.qr.rows.len() - 1, c)) };
            }
        }
        self.dirty = true;
    }

    pub fn tsv(&self, rows: &[usize], header: bool) -> String {
        let mut s = String::new();
        if header {
            s.push_str(&self.qr.columns.join("\t"));
            s.push('\n');
        }
        for &r in rows {
            s.push_str(&self.qr.rows[r].iter().map(|v| v.clone().unwrap_or_default()).collect::<Vec<_>>().join("\t"));
            s.push('\n');
        }
        s
    }

    fn move_sel(&mut self, dr: i32, dc: i32) {
        let Some((r, c)) = self.sel else { return };
        let ncols = self.qr.columns.len() as i32;
        let pos = self.view_idx.iter().position(|&x| x == r).unwrap_or(0) as i32;
        let mut nc = c as i32 + dc;
        let mut np = pos + dr;
        if nc >= ncols {
            nc = 0;
            np += 1;
        } else if nc < 0 {
            nc = ncols - 1;
            np -= 1;
        }
        np = np.clamp(0, (self.view_idx.len() as i32 - 1).max(0));
        if let Some(&nr) = self.view_idx.get(np as usize) {
            self.sel = Some((nr, nc.clamp(0, (ncols - 1).max(0)) as usize));
            self.clear_multi();
            self.scroll_to = Some(nr);
            self.scroll_to_col = Some(nc.clamp(0, (ncols - 1).max(0)) as usize);
        }
    }

    /// Keeps the value-panel buffer in line with the current cell.
    pub fn sync_value_buf(&mut self, r: usize, c: usize) {
        let cur = self.qr.rows[r][c].clone().unwrap_or_default();
        if self.vp_for != Some((r, c)) {
            self.vp_buf = cur.clone();
            self.vp_orig = cur;
            self.vp_for = Some((r, c));
        } else if cur != self.vp_orig {
            if self.vp_buf == self.vp_orig {
                self.vp_buf = cur.clone();
            }
            self.vp_orig = cur;
        }
    }

    /// Count, distinct, sum, average, min and max of the selected cells (or of a whole column).
    pub fn aggregates(&self, whole_column: Option<usize>) -> Aggregates {
        let cells: Vec<(usize, usize)> = match whole_column {
            Some(c) => self.view_idx.iter().map(|&r| (r, c)).collect(),
            None => self.selection_matrix().into_iter().flatten().collect(),
        };
        let mut a = Aggregates { cells: cells.len(), ..Default::default() };
        let mut seen = std::collections::HashSet::new();
        let (mut sum, mut nums) = (0.0f64, 0usize);
        let (mut nmin, mut nmax) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut smin, mut smax): (Option<String>, Option<String>) = (None, None);
        let mut all_numeric = true;
        for (r, c) in cells {
            match self.qr.rows.get(r).and_then(|x| x.get(c)).cloned().flatten() {
                None => a.nulls += 1,
                Some(v) => {
                    a.count += 1;
                    match v.trim().parse::<f64>() {
                        Ok(x) if x.is_finite() => {
                            nums += 1;
                            sum += x;
                            nmin = nmin.min(x);
                            nmax = nmax.max(x);
                        }
                        _ => all_numeric = false,
                    }
                    if smin.as_ref().map(|m| v < *m).unwrap_or(true) {
                        smin = Some(v.clone());
                    }
                    if smax.as_ref().map(|m| v > *m).unwrap_or(true) {
                        smax = Some(v.clone());
                    }
                    seen.insert(v);
                }
            }
        }
        a.distinct = seen.len();
        a.numeric = nums;
        if nums > 0 {
            a.sum = Some(sum);
            a.avg = Some(sum / nums as f64);
        }
        if a.count > 0 {
            if all_numeric {
                a.min = Some(fmt_num(nmin));
                a.max = Some(fmt_num(nmax));
            } else {
                a.min = smin;
                a.max = smax;
            }
        }
        a
    }

    pub fn clear_multi(&mut self) {
        self.multi_sel.clear();
        self.rect_sel = None;
        self.sel_cols.clear();
        self.col_anchor = None;
    }

    /// (first position, last position, first column, last column) of the dragged block
    pub fn rect_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        self.rect_sel.map(|((p0, c0), (p1, c1))| (p0.min(p1), p0.max(p1), c0.min(c1), c0.max(c1)))
    }

    /// Cells of the current selection, row by row (real row index, column), in view order.
    pub fn selection_matrix(&self) -> Vec<Vec<(usize, usize)>> {
        let ncols = self.qr.columns.len();
        if !self.sel_cols.is_empty() {
            return self.view_idx.iter().map(|&r| self.sel_cols.iter().map(|&c| (r, c)).collect()).collect();
        }
        if let Some((p0, p1, c0, c1)) = self.rect_bounds() {
            return (p0..=p1.min(self.view_idx.len().saturating_sub(1))).filter_map(|p| self.view_idx.get(p).map(|&r| (c0..=c1.min(ncols.saturating_sub(1))).map(|c| (r, c)).collect())).collect();
        }
        if self.multi_sel.len() > 1 {
            return self.view_idx.iter().filter(|r| self.multi_sel.contains(r)).map(|&r| (0..ncols).map(|c| (r, c)).collect()).collect();
        }
        match self.sel {
            Some(rc) => vec![vec![rc]],
            None => vec![],
        }
    }

    /// The selection as TSV text, for the clipboard.
    pub fn selection_text(&self) -> Option<String> {
        let m = self.selection_matrix();
        if m.is_empty() {
            return None;
        }
        let lines: Vec<String> = m.iter().map(|row| row.iter().map(|&(r, c)| self.qr.rows.get(r).and_then(|x| x.get(c)).cloned().flatten().unwrap_or_default()).collect::<Vec<_>>().join("\t")).collect();
        Some(lines.join("\n"))
    }

    pub fn is_selected_cell(&self, pos: usize, c: usize) -> bool {
        self.sel_cols.contains(&c) || self.rect_bounds().map(|(p0, p1, c0, c1)| pos >= p0 && pos <= p1 && c >= c0 && c <= c1).unwrap_or(false)
    }

    /// Paste a TSV block. It starts at the top-left cell of the selection; a single value pasted over a
    /// multi-cell selection fills all of it. The whole paste is one undo step.
    pub fn paste(&mut self, text: &str) {
        let Some((r0, c0)) = self.sel else { return };
        let block: Vec<Vec<&str>> = text.trim_end_matches('\n').split('\n').map(|l| l.trim_end_matches('\r').split('\t').collect()).collect();
        let before = self.undo.len();
        let cells = self.selection_matrix();
        if block.len() == 1 && block[0].len() == 1 && cells.iter().map(|r| r.len()).sum::<usize>() > 1 {
            for (r, c) in cells.into_iter().flatten() {
                self.set_cell(r, c, Some(block[0][0].to_string()));
            }
        } else {
            let (pos0, col0) = if !self.sel_cols.is_empty() {
                (0, *self.sel_cols.iter().next().unwrap())
            } else if let Some((p0, _, c0, _)) = self.rect_bounds() {
                (p0, c0)
            } else if self.multi_sel.len() > 1 {
                (self.view_idx.iter().position(|r| self.multi_sel.contains(r)).unwrap_or(0), 0)
            } else {
                (self.view_idx.iter().position(|&x| x == r0).unwrap_or(0), c0)
            };
            for (i, line) in block.iter().enumerate() {
                let Some(&r) = self.view_idx.get(pos0 + i) else { break };
                for (j, v) in line.iter().enumerate() {
                    let c = col0 + j;
                    if c >= self.qr.columns.len() {
                        break;
                    }
                    self.set_cell(r, c, Some(v.to_string()));
                }
            }
        }
        let n = self.undo.len().saturating_sub(before);
        if n > 1 {
            self.undo.push(Undo::Group(n));
        }
    }
}

const ROW_H: f32 = 24.0;

pub fn parse_bool(v: &Option<String>) -> Option<bool> {
    match v.as_deref().map(|s| s.trim().to_lowercase()).as_deref() {
        Some("t") | Some("true") | Some("1") | Some("y") | Some("yes") => Some(true),
        Some("f") | Some("false") | Some("0") | Some("n") | Some("no") => Some(false),
        _ => None,
    }
}

/// How the column writes booleans: 0 = t/f, 1 = true/false, 2 = 1/0.
fn bool_style(g: &GridState, c: usize) -> u8 {
    for r in g.qr.rows.iter().take(50) {
        match r[c].as_deref() {
            Some("t") | Some("f") => return 0,
            Some("true") | Some("false") => return 1,
            Some("1") | Some("0") => return 2,
            _ => {}
        }
    }
    match g.edit.as_ref().map(|e| e.kind) {
        Some(crate::db::Kind::Postgres) => 0,
        Some(crate::db::Kind::Mongo) => 1,
        _ => 2,
    }
}

fn format_bool(style: u8, b: bool) -> String {
    match (style, b) {
        (0, true) => "t".into(),
        (0, false) => "f".into(),
        (1, true) => "true".into(),
        (1, false) => "false".into(),
        (_, true) => "1".into(),
        (_, false) => "0".into(),
    }
}

fn looks_like_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 8 && b[..4].iter().all(|c| c.is_ascii_digit()) && b[4] == b'-' || (b.len() >= 5 && b[2] == b':' && b[0].is_ascii_digit())
}


const VP_MIN_PANEL: f32 = 180.0;
const VP_MIN_GRID: f32 = 120.0;
const VP_HANDLE: f32 = 6.0;

/// Splits `full` into (grid, drag handle, value panel). Returns `None` when the area cannot hold both at their
/// minimum sizes, so that no rectangle ever has a negative size.
pub fn split_value_panel(full: egui::Rect, vp_width: f32) -> Option<(egui::Rect, egui::Rect, egui::Rect)> {
    let (w, h) = (full.width(), full.height());
    if !(w.is_finite() && h.is_finite()) || w < VP_MIN_GRID + VP_MIN_PANEL + VP_HANDLE || h < 60.0 {
        return None;
    }
    let pw = vp_width.clamp(VP_MIN_PANEL, w - VP_MIN_GRID - VP_HANDLE);
    let panel = egui::Rect::from_min_max(egui::pos2(full.right() - pw, full.top()), full.max);
    let grid = egui::Rect::from_min_max(full.min, egui::pos2(full.right() - pw - VP_HANDLE, full.bottom()));
    let handle = egui::Rect::from_min_max(egui::pos2(grid.right(), full.top()), egui::pos2(panel.left(), full.bottom()));
    Some((grid, handle, panel))
}

pub fn show(ui: &mut Ui, g: &mut GridState, null_text: &str, value_panel: bool) -> Vec<GridAction> {
    let mut actions = Vec::new();
    if g.dirty {
        g.reindex();
    }
    // the area is split by hand: the grid gets the left part and the panel the right part, so they never overlap;
    // when the area is too small for both, the panel is left out instead of using negative sizes
    let split = if value_panel && g.mode == ViewMode::Grid { split_value_panel(ui.available_rect_before_wrap(), g.vp_width) } else { None };
    if let Some((grid_rect, handle, panel_rect)) = split {
        let pl = pal(ui.visuals().dark_mode);
        let full = ui.available_rect_before_wrap();
        let hr = ui.interact(handle, egui::Id::new(("vp_handle", g.uid)), Sense::drag());
        if hr.hovered() || hr.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        if hr.dragged() {
            g.vp_width = (g.vp_width - hr.drag_delta().x).clamp(VP_MIN_PANEL, 520.0);
        }
        let _ = full;
        let frame = egui::Frame::new().fill(pl.panel).stroke(Stroke::new(1.0, pl.border)).inner_margin(egui::Margin::same(10)).corner_radius(CornerRadius::same(6));
        ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
            frame.show(ui, |ui| {
                ui.set_min_size(egui::vec2((panel_rect.width() - 22.0).max(0.0), (panel_rect.height() - 22.0).max(0.0)));
                egui::ScrollArea::vertical().id_salt("vp_all").auto_shrink([false, false]).show(ui, |ui| value_panel_ui(ui, g, &mut actions, (panel_rect.width() - 24.0).max(0.0)));
            });
        });
        ui.scope_builder(egui::UiBuilder::new().max_rect(grid_rect), |ui| show_grid(ui, g, null_text, &mut actions));
        return actions;
    }
    match g.mode {
        ViewMode::Grid => show_grid(ui, g, null_text, &mut actions),
        ViewMode::Text => show_text(ui, g),
        ViewMode::Record => show_record(ui, g, null_text),
    }
    actions
}

fn value_panel_ui(ui: &mut Ui, g: &mut GridState, actions: &mut Vec<GridAction>, inner_w: f32) {
    let pl = pal(ui.visuals().dark_mode);
    icons::section(ui, "Valor da célula");
    let Some((r, c)) = g.sel else {
        ui.label(RichText::new("Selecione uma célula").color(pl.weak));
        return;
    };
    if r >= g.qr.rows.len() || c >= g.qr.columns.len() {
        return;
    }
    g.sync_value_buf(r, c);
    let typ = g.edit.as_ref().and_then(|e| e.meta.columns.iter().find(|m| m.name.eq_ignore_ascii_case(&g.qr.columns[c])).map(|m| m.typ.clone())).unwrap_or_default();
    ui.label(RichText::new(&g.qr.columns[c]).strong());
    if !typ.is_empty() {
        ui.label(RichText::new(typ).small().color(pl.weak));
    }
    let editable = g.editable && g.row_state[r] != RowState::Deleted;
    // a read-only value must stay fully readable: `interactive(false)` instead of a faded, disabled box
    // every part follows the panel width
    let w = inner_w.max(120.0);
    ui.set_max_width(w);
    egui::ScrollArea::vertical().id_salt("vp_text").max_height(g.vp_text_h).min_scrolled_height(g.vp_text_h).auto_shrink([false, false]).show(ui, |ui| {
        ui.add_sized([(w - 8.0).max(60.0), (g.vp_text_h - 6.0).max(30.0)], egui::TextEdit::multiline(&mut g.vp_buf).font(egui::TextStyle::Monospace).interactive(editable));
    });
    // grip under the text box: drag to make it taller or shorter
    let (grip, gr) = ui.allocate_exact_size(egui::vec2(w, 9.0), Sense::drag());
    if gr.hovered() || gr.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if gr.dragged() {
        g.vp_text_h = (g.vp_text_h + gr.drag_delta().y).clamp(36.0, 600.0);
    }
    for dy in [-1.5, 1.5] {
        ui.painter().line_segment([egui::pos2(grip.center().x - 14.0, grip.center().y + dy), egui::pos2(grip.center().x + 14.0, grip.center().y + dy)], Stroke::new(1.0, pl.weak));
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(editable, egui::Button::new("Aplicar")).clicked() {
            let v = g.vp_buf.clone();
            g.set_cell(r, c, Some(v));
            actions.push(GridAction::Changed);
        }
        if ui.add_enabled(editable, egui::Button::new("NULL")).clicked() {
            g.set_cell(r, c, None);
            g.vp_for = None;
            actions.push(GridAction::Changed);
        }
        if ui.button("Copiar").clicked() {
            ui.ctx().copy_text(g.vp_buf.clone());
        }
    });
    ui.separator();
    let open = g.vp_stats;
    if ui.selectable_label(open, if open { "▼ Estatísticas" } else { "▶ Estatísticas" }).clicked() {
        g.vp_stats = !g.vp_stats;
    }
    if g.vp_stats {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut g.vp_stats_col, false, "Seleção");
            ui.selectable_value(&mut g.vp_stats_col, true, "Coluna toda");
        });
        let whole = if g.vp_stats_col { Some(c) } else { None };
        let a = g.aggregates(whole);
        let rows: Vec<(&str, String)> = vec![
            ("Células", a.cells.to_string()),
            ("Count", a.count.to_string()),
            ("Distinct", a.distinct.to_string()),
            ("Nulos", a.nulls.to_string()),
            ("Sum", a.sum.map(fmt_num).unwrap_or_else(|| "—".into())),
            ("Avg", a.avg.map(fmt_num).unwrap_or_else(|| "—".into())),
            ("Min", a.min.clone().unwrap_or_else(|| "—".into())),
            ("Max", a.max.clone().unwrap_or_else(|| "—".into())),
        ];
        egui::Grid::new(("vp_stats_grid", g.uid)).num_columns(4).min_col_width(((w - 30.0) / 4.0).max(40.0)).spacing([10.0, 3.0]).show(ui, |ui| {
            for (i, (k, v)) in rows.iter().enumerate() {
                ui.label(RichText::new(*k).color(pl.weak));
                ui.add(egui::Label::new(RichText::new(v).monospace()).truncate());
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        if ui.small_button("Copiar estatísticas").clicked() {
            ui.ctx().copy_text(rows.iter().map(|(k, v)| format!("{}\t{}", k, v)).collect::<Vec<_>>().join("\n"));
        }
    }
}

fn show_text(ui: &mut Ui, g: &GridState) {
    let opts = crate::export::ExportOptions { format: crate::export::Format::Txt, show_nulls: true, ..Default::default() };
    let mut sub = QueryResult { columns: g.qr.columns.clone(), ..Default::default() };
    let cut = crate::constants::TEXT_MAX_COLUMN_SIZE;
    sub.rows = g.view_idx.iter().map(|&i| g.qr.rows[i].iter().map(|v| v.as_ref().map(|s| if s.chars().count() > cut { s.chars().take(cut).collect::<String>() } else { s.clone() })).collect()).collect();
    let mut t = crate::export::render(&sub, &opts, crate::db::Kind::Sqlite);
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.add(egui::TextEdit::multiline(&mut t).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).interactive(true));
    });
}

fn show_record(ui: &mut Ui, g: &mut GridState, null_text: &str) {
    let pl = pal(ui.visuals().dark_mode);
    let Some((r, _)) = g.sel.or(if g.qr.rows.is_empty() { None } else { Some((0, 0)) }) else {
        ui.label("Sem dados");
        return;
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new(("record_grid", g.uid)).striped(true).num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
            for (c, name) in g.qr.columns.iter().enumerate() {
                ui.label(RichText::new(name).strong().color(pl.weak));
                match &g.qr.rows[r][c] {
                    Some(v) => {
                        ui.label(v);
                    }
                    None => {
                        ui.label(RichText::new(null_text).weak().italics());
                    }
                }
                ui.end_row();
            }
        });
    });
}

/// Longest text a grid cell lays out; the full value stays in the data and in the value panel.
const CELL_DISPLAY_CHARS: usize = 300;

/// Text drawn in a cell: newlines and tabs flattened, cut to `CELL_DISPLAY_CHARS` before layout so that
/// huge values (JSON, text blobs) are not copied, hashed and laid out in full on every frame.
fn cell_display_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(CELL_DISPLAY_CHARS + 4));
    for (n, ch) in s.chars().enumerate() {
        if n >= CELL_DISPLAY_CHARS {
            out.push('…');
            break;
        }
        out.push(match ch {
            '\n' => '¶',
            '\t' => ' ',
            c => c,
        });
    }
    out
}

/// The table only scrolls vertically by itself: the horizontal scroll bar comes from this wrapper.
fn show_grid(ui: &mut Ui, g: &mut GridState, null_text: &str, actions: &mut Vec<GridAction>) {
    egui::ScrollArea::horizontal().id_salt(("grid_hscroll", g.uid)).auto_shrink([false, false]).show(ui, |ui| show_grid_table(ui, g, null_text, actions));
}

fn show_grid_table(ui: &mut Ui, g: &mut GridState, null_text: &str, actions: &mut Vec<GridAction>) {
    let pl = pal(ui.visuals().dark_mode);
    let ncols = g.qr.columns.len();
    if ncols == 0 {
        return;
    }
    let nrows = g.view_idx.len();
    let select_all_flag = g.select_all;
    let mut clicked: Option<(usize, usize, egui::Modifiers)> = None;
    let mut row_clicked: Option<(usize, egui::Modifiers)> = None;
    let mut dbl: Option<(usize, usize)> = None;
    let mut ctx_actions: Vec<GridAction> = Vec::new();
    let mut commit_edit: Option<(usize, usize, String, i32, i32)> = None;
    let mut cancel_edit = false;
    let mut header_click: Option<(usize, egui::Modifiers)> = None;
    let mut sort_click: Option<usize> = None;
    let mut row_scroll_rect: Option<egui::Rect> = None;
    let mut frame_cells: Vec<(usize, usize, egui::Rect)> = Vec::new();
    let mut drag_start: Option<(usize, usize)> = None;
    let scroll_col = g.scroll_to_col.take();
    let has_types = g.edit.is_some();
    let header_h = if has_types { 40.0 } else { 30.0 };
    let col_meta: Vec<Option<(String, bool)>> = (0..ncols)
        .map(|c| g.edit.as_ref().and_then(|e| e.meta.columns.iter().find(|m| m.name.eq_ignore_ascii_case(&g.qr.columns[c])).map(|m| (m.typ.clone(), m.pk))))
        .collect();

    let is_bool: Vec<bool> = (0..ncols).map(|c| col_meta[c].as_ref().map(|(t, _)| { let t = t.to_lowercase(); t == "bool" || t == "boolean" || t == "bit" }).unwrap_or(false)).collect();
    let nullable_col: Vec<bool> = (0..ncols)
        .map(|c| g.edit.as_ref().and_then(|e| e.meta.columns.iter().find(|m| m.name.eq_ignore_ascii_case(&g.qr.columns[c])).map(|m| m.nullable)).unwrap_or(true))
        .collect();
    // numbers and date/times are right-justified (RESULT_SET_RIGHT_JUSTIFY_NUMBERS / _DATETIME = true)
    let right_align: Vec<bool> = (0..ncols)
        .map(|c| {
            g.numeric[c]
                || col_meta[c].as_ref().map(|(t, _)| { let t = t.to_lowercase(); t.contains("date") || t.contains("time") }).unwrap_or(false)
                || g.qr.rows.iter().take(20).filter_map(|r| r[c].as_deref()).next().map(looks_like_date).unwrap_or(false)
        })
        .collect();
    let mut bool_toggle: Option<(usize, usize)> = None;
    let mut last_seen = false;
    let mut tb = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .auto_shrink([false, false])
        .sense(Sense::click_and_drag())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(48.0));
    for c in 0..ncols {
        tb = tb.column(Column::initial(g.widths[c]).at_least(48.0).clip(true));
    }
    if let Some(r) = g.scroll_to.take() {
        if let Some(pos) = g.view_idx.iter().position(|&x| x == r) {
            tb = tb.scroll_to_row(pos, None);
        }
    }
    tb.header(header_h, |mut h| {
        h.col(|ui| {
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, 0.0, pl.header);
            ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, pl.border));
        });
        for c in 0..ncols {
            h.col(|ui| {
                let rect = ui.max_rect();
                let pt = ui.painter();
                pt.rect_filled(rect, 0.0, if g.sel_cols.contains(&c) { pl.accent_soft } else { pl.header });
                pt.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, pl.border));
                pt.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, pl.grid_line));
                let resp = ui.interact(rect, ui.id().with(("hdr", c)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, pl.hover);
                }
                let meta = &col_meta[c];
                ui.vertical(|ui| {
                    ui.set_max_width((rect.width() - 28.0).max(20.0));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add_space(if has_types { 3.0 } else { 0.0 });
                    ui.horizontal(|ui| {
                        if let Some((_, true)) = meta {
                            let s = icons::icon_size(ui) - 3.0;
                            let (ir, _) = ui.allocate_exact_size(egui::vec2(s, s), Sense::hover());
                            icons::paint(ui.painter(), ir, Icon::Key, pl.warn);
                        }
                        ui.add(egui::Label::new(RichText::new(&g.qr.columns[c]).strong()).truncate().selectable(false));
                    });
                    if let Some((t, _)) = meta {
                        ui.add(egui::Label::new(RichText::new(t).small().color(pl.weak)).truncate().selectable(false));
                    }
                });
// sort button: its own widget on top of the title area
                let btn = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), egui::vec2(20.0, 22.0));
                let bresp = ui.interact(btn, ui.id().with(("sort", c)), Sense::click());
                let state = match g.sort {
                    Some((sc, asc)) if sc == c => Some(asc),
                    _ => None,
                };
                if bresp.hovered() {
                    ui.painter().rect_filled(btn, CornerRadius::same(4), pl.border);
                }
                let (cx, cy) = (btn.center().x, btn.center().y);
                let up = vec![egui::pos2(cx - 4.0, cy - 1.5), egui::pos2(cx + 4.0, cy - 1.5), egui::pos2(cx, cy - 6.0)];
                let down = vec![egui::pos2(cx - 4.0, cy + 1.5), egui::pos2(cx + 4.0, cy + 1.5), egui::pos2(cx, cy + 6.0)];
                let faint = pl.weak.gamma_multiply(0.5);
                ui.painter().add(egui::Shape::convex_polygon(up, if state == Some(true) { pl.accent } else { faint }, Stroke::NONE));
                ui.painter().add(egui::Shape::convex_polygon(down, if state == Some(false) { pl.accent } else { faint }, Stroke::NONE));
                if bresp.clicked() {
                    sort_click = Some(c);
                } else if resp.clicked() {
                    header_click = Some((c, ui.ctx().input(|i| i.modifiers)));
                }
                bresp.on_hover_text("Ordenar por esta coluna (crescente → decrescente → nenhuma)");
                resp.on_hover_text(format!("{} — clique para selecionar a coluna", g.qr.columns[c]));
            });
        }
    })
    .body(|body| {
        body.rows(ROW_H, nrows, |mut row| {
            let vi = row.index();
            if vi + 1 == nrows {
                last_seen = true;
            }
            let r = g.view_idx[vi];
            let st = g.row_state[r].clone();
            let in_sel = g.sel.map(|s| s.0 == r).unwrap_or(false) || g.multi_sel.contains(&r);
            // row number / state column
            let (hrect, hresp) = row.col(|ui| {
                let rect = ui.max_rect();
                let pt = ui.painter();
                pt.rect_filled(rect, 0.0, if in_sel { pl.accent_soft } else { pl.header });
                pt.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, pl.border));
                let (t, col) = match st {
                    RowState::New => ("+".to_string(), pl.ok),
                    RowState::Deleted => ("×".to_string(), pl.err),
                    RowState::Modified(_) => (format!("{}", r + 1 + g.qr.offset), pl.warn),
                    RowState::Clean => (format!("{}", r + 1 + g.qr.offset), pl.weak),
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    ui.label(RichText::new(t).small().color(col));
                });
            });
            let _ = hrect;
            if hresp.clicked() {
                row_clicked = Some((r, hresp.ctx.input(|i| i.modifiers)));
            }
            for c in 0..ncols {
                let mut commit: Option<(String, i32, i32)> = None;
                let mut cancel = false;
                let editing_here = matches!(&g.editing, Some((er, ec, _)) if *er == r && *ec == c);
                let is_sel = g.sel == Some((r, c));
                let (rect, resp) = row.col(|ui| {
                    let rect = ui.max_rect().expand2(egui::vec2(4.0, 1.0));
                    let pt = ui.painter();
                    // background by state
                    let changed = matches!(&st, RowState::Modified(o) if o[c] != g.qr.rows[r][c]);
                    let bg = match &st {
                        RowState::New => Some(pl.cell_new),
                        RowState::Deleted => Some(pl.cell_del),
                        _ if changed => Some(pl.cell_mod),
                        _ => None,
                    };
                    if let Some(bg) = bg {
                        pt.rect_filled(rect, 0.0, bg);
                    }
                    if g.is_selected_cell(vi, c) && !is_sel {
                        pt.rect_filled(rect, 0.0, pl.accent_soft.gamma_multiply(0.8));
                    } else if in_sel && !is_sel && g.rect_sel.is_none() && g.sel_cols.is_empty() {
                        pt.rect_filled(rect, 0.0, pl.accent_soft.gamma_multiply(0.55));
                    }
                    pt.line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, pl.grid_line));
                    pt.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, pl.grid_line));
                    if is_sel {
                        pt.rect_filled(rect, 0.0, pl.accent_soft);
                        pt.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.6, pl.accent), StrokeKind::Inside);
                    }
                    if editing_here {
                        // keys are consumed first so that the text box does not move the focus
                        let (enter, tab, stab, esc) = ui.input_mut(|i| {
                            (
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                                i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                            )
                        });
                        if let Some((_, _, buf)) = g.editing.as_mut() {
                            let te = ui.add(egui::TextEdit::singleline(buf).desired_width(f32::INFINITY).frame(egui::Frame::NONE).margin(egui::vec2(2.0, 2.0)));
                            if !te.has_focus() {
                                te.request_focus();
                                // F2 / double click select the old text; typing starts with the caret at the end
                                let n = buf.chars().count();
                                let range = if select_all_flag { egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n)) } else { egui::text::CCursorRange::one(egui::text::CCursor::new(n)) };
                                let mut st = egui::TextEdit::load_state(ui.ctx(), te.id).unwrap_or_default();
                                st.cursor.set_char_range(Some(range));
                                st.store(ui.ctx(), te.id);
                            }
                            if enter {
                                commit = Some((buf.clone(), 1, 0));
                            } else if tab {
                                commit = Some((buf.clone(), 0, 1));
                            } else if stab {
                                commit = Some((buf.clone(), 0, -1));
                            } else if esc {
                                cancel = true;
                            } else if te.lost_focus() {
                                commit = Some((buf.clone(), 0, 0));
                            }
                        }
                    } else {
                        let v = &g.qr.rows[r][c];
                        if is_bool[c] {
                            let b = parse_bool(v);
                            let sz = 15.0;
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                let (br, _) = ui.allocate_exact_size(egui::vec2(sz, sz), Sense::hover());
                                let pt = ui.painter();
                                pt.rect_filled(br, CornerRadius::same(3), if b == Some(true) { pl.accent } else { pl.editor });
                                pt.rect_stroke(br, CornerRadius::same(3), Stroke::new(1.2, if b.is_some() { pl.accent } else { pl.weak }), StrokeKind::Inside);
                                if b == Some(true) {
                                    icons::paint(pt, br.shrink(1.5), Icon::Check, egui::Color32::WHITE);
                                }
                                if b.is_none() {
                                    ui.label(RichText::new(null_text).small().italics().color(pl.weak));
                                } else {
                                    ui.label(RichText::new(if b == Some(true) { "true" } else { "false" }).small().color(pl.weak));
                                }
                            });
                            return;
                        }
                        let text = match v {
                            Some(s) => {
                                let t: String = cell_display_text(s);
                                let mut rt = RichText::new(t);
                                if g.numeric[c] {
                                    rt = rt.color(pl.number);
                                }
                                if st == RowState::Deleted {
                                    rt = rt.strikethrough().color(pl.err);
                                }
                                rt
                            }
                            None => RichText::new(null_text).small().italics().color(pl.weak),
                        };
                        if right_align[c] {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(egui::Label::new(text).truncate().selectable(false));
                            });
                        } else {
                            ui.add(egui::Label::new(text).truncate().selectable(false));
                        }
                    }
                });
                #[cfg(test)]
                g.cell_rects.push(((r, c), rect));
                frame_cells.push((vi, c, rect));
                if scroll_col == Some(c) && g.sel.map(|s| s.0) == Some(r) {
                    resp.ctx.request_repaint();
                    row_scroll_rect = Some(rect);
                }
                if resp.drag_started() && !editing_here {
                    drag_start = Some((vi, c));
                }
                if let Some((txt, dr, dc)) = commit {
                    commit_edit = Some((r, c, txt, dr, dc));
                }
                if cancel {
                    cancel_edit = true;
                }
                if resp.clicked() && !editing_here {
                    clicked = Some((r, c, resp.ctx.input(|i| i.modifiers)));
                    if is_bool[c] && g.editable && g.row_state[r] != RowState::Deleted {
                        bool_toggle = Some((r, c));
                    }
                }
                if resp.double_clicked() && g.editable {
                    dbl = Some((r, c));
                }
                resp.context_menu(|ui| {
                    let val = g.qr.rows[r][c].clone();
                    if ui.button("Copiar valor").clicked() {
                        ui.ctx().copy_text(val.clone().unwrap_or_default());
                        ui.close();
                    }
                    if ui.button("Copiar linha").clicked() {
                        ui.ctx().copy_text(g.tsv(&[r], false));
                        ui.close();
                    }
                    if ui.button("Cópia avançada (com cabeçalho)").clicked() {
                        let rows: Vec<usize> = g.view_idx.clone();
                        ui.ctx().copy_text(g.tsv(&rows, true));
                        ui.close();
                    }
                    if ui.button("Ver valor").clicked() {
                        ctx_actions.push(GridAction::ViewValue(val.clone().unwrap_or_else(|| "NULL".into())));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Filtrar por este valor").clicked() {
                        ctx_actions.push(GridAction::FilterBy(c, val.clone()));
                        ui.close();
                    }
                    if g.editable {
                        ui.separator();
                        if ui.button("Editar célula   F2").clicked() {
                            dbl = Some((r, c));
                            ui.close();
                        }
                        if ui.button("Definir como NULL").clicked() {
                            g.set_cell(r, c, None);
                            ctx_actions.push(GridAction::Changed);
                            ui.close();
                        }
                        if ui.button("Definir como vazio").clicked() {
                            g.set_cell(r, c, Some(String::new()));
                            ctx_actions.push(GridAction::Changed);
                            ui.close();
                        }
                        if ui.button("Colar   Ctrl+V").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Adicionar linha   Ins").clicked() {
                            let n = g.qr.columns.len();
                            g.add_row(vec![None; n]);
                            ui.close();
                        }
                        if ui.button("Duplicar linha").clicked() {
                            let row = g.qr.rows[r].clone();
                            g.add_row(row);
                            ui.close();
                        }
                        if ui.button("Excluir linha(s)   Del").clicked() {
                            let rows = if g.multi_sel.contains(&r) { g.target_rows() } else { vec![r] };
                            g.toggle_delete(&rows);
                            ui.close();
                        }
                    }
                });
            }
        });
    });

    // ---- apply interactions
    if let Some((r, c)) = bool_toggle {
        let rows = if g.multi_sel.contains(&r) { g.target_rows() } else { vec![r] };
        g.toggle_bool(c, nullable_col[c], &rows);
        actions.push(GridAction::Changed);
    }
    // the next page is fetched when the last row comes into view (RESULT_SET_AUTO_FETCH_NEXT_SEGMENT = true)
    if crate::constants::AUTO_FETCH_NEXT_SEGMENT && last_seen && g.qr.has_more && g.editing.is_none() {
        actions.push(GridAction::FetchMore);
    }
    if commit_edit.is_some() || cancel_edit {
        // give the keyboard back to the grid right away
        ui.ctx().memory_mut(|m| {
            if let Some(f) = m.focused() {
                m.surrender_focus(f);
            }
        });
    }
    if let Some((r, c, txt, dr, dc)) = commit_edit {
        g.editing = None;
        g.set_cell(r, c, Some(txt));
        actions.push(GridAction::Changed);
        g.sel = Some((r, c));
        g.move_sel(dr, dc);
    }
    if cancel_edit {
        g.editing = None;
    }
    if let Some(c) = sort_click {
        match g.sort {
            Some((sc, true)) if sc == c => g.sort = Some((c, false)),
            Some((sc, false)) if sc == c => g.sort = None,
            _ => g.sort = Some((c, true)),
        }
        g.dirty = true;
        actions.push(GridAction::SortBy);
    }
    if let Some((c, m)) = header_click {
        select_column(g, c, m);
    }
    // click and drag over cells selects a block
    if let Some((vi, c)) = drag_start {
        let r = g.view_idx[vi];
        select(g, r, Some(c), egui::Modifiers::NONE);
        g.drag_from = Some((vi, c));
    }
    if let Some(from) = g.drag_from {
        if ui.input(|i| i.pointer.primary_down()) {
            if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                if let Some(&(vi, c, _)) = frame_cells.iter().find(|(_, _, rc)| rc.contains(p)) {
                    if (vi, c) != from {
                        g.rect_sel = Some((from, (vi, c)));
                        g.multi_sel.clear();
                    }
                }
            }
        } else {
            g.drag_from = None;
        }
    }
    if let Some(rc) = row_scroll_rect {
        ui.scroll_to_rect(rc, None);
    }
    if let Some((r, c, m)) = clicked {
        select(g, r, Some(c), m);
    }
    if let Some((r, m)) = row_clicked {
        select(g, r, None, m);
    }
    if let Some((r, c)) = dbl {
        g.sel = Some((r, c));
        g.select_all = true;
        g.editing = Some((r, c, g.qr.rows[r][c].clone().unwrap_or_default()));
    }
    keyboard(ui, g, actions);
    actions.extend(ctx_actions);
}

fn select(g: &mut GridState, r: usize, c: Option<usize>, m: egui::Modifiers) {
    let col = c.or(g.sel.map(|s| s.1)).unwrap_or(0);
    if m.shift && g.anchor.is_some() {
        let a = g.view_idx.iter().position(|&x| Some(x) == g.anchor).unwrap_or(0);
        let b = g.view_idx.iter().position(|&x| x == r).unwrap_or(0);
        g.sel_cols.clear();
        if c.is_some() {
            // cells: a block from the anchor cell to the clicked one
            g.multi_sel.clear();
            g.rect_sel = Some(((a, g.anchor_col), (b, col)));
        } else {
            g.rect_sel = None;
            g.multi_sel = g.view_idx[a.min(b)..=a.max(b)].iter().copied().collect();
        }
        g.sel = Some((r, col));
    } else if m.command {
        g.rect_sel = None;
        g.sel_cols.clear();
        if !g.multi_sel.remove(&r) {
            if let Some((sr, _)) = g.sel {
                g.multi_sel.insert(sr);
            }
            g.multi_sel.insert(r);
        }
        g.sel = Some((r, col));
        g.anchor = Some(r);
        g.anchor_col = col;
    } else {
        g.clear_multi();
        g.sel = Some((r, col));
        g.anchor = Some(r);
        g.anchor_col = col;
    }
}

/// Click on a column title: selects the whole column (Ctrl adds/removes, Shift extends from the last one).
fn select_column(g: &mut GridState, c: usize, m: egui::Modifiers) {
    g.multi_sel.clear();
    g.rect_sel = None;
    if m.shift && g.col_anchor.is_some() {
        let a = g.col_anchor.unwrap_or(c);
        g.sel_cols = (a.min(c)..=a.max(c)).collect();
    } else if m.command {
        if !g.sel_cols.remove(&c) {
            g.sel_cols.insert(c);
        }
        g.col_anchor = Some(c);
    } else {
        g.sel_cols.clear();
        g.sel_cols.insert(c);
        g.col_anchor = Some(c);
    }
    if g.sel.is_none() && !g.view_idx.is_empty() {
        g.sel = Some((g.view_idx[0], c));
    } else if let Some((r, _)) = g.sel {
        g.sel = Some((r, c));
    }
}

fn keyboard(ui: &mut Ui, g: &mut GridState, actions: &mut Vec<GridAction>) {
    if g.editing.is_some() || ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (copy, paste, undo, up, down, left, right, del, ins, f2, esc, pgup, pgdn, text) = ui.input(|i| {
        (
            i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
            i.events.iter().find_map(|e| if let egui::Event::Paste(t) = e { Some(t.clone()) } else { None }),
            i.modifiers.command && i.key_pressed(egui::Key::Z),
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowLeft),
            i.key_pressed(egui::Key::ArrowRight),
            i.key_pressed(egui::Key::Delete),
            i.key_pressed(egui::Key::Insert),
            i.key_pressed(egui::Key::F2),
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::PageUp),
            i.key_pressed(egui::Key::PageDown),
            i.events.iter().find_map(|e| if let egui::Event::Text(t) = e { Some(t.clone()) } else { None }),
        )
    });
    if copy {
        if let Some(t) = g.selection_text() {
            ui.ctx().copy_text(t);
        }
    }
    if g.editable {
        if let Some(t) = paste {
            g.paste(&t);
            actions.push(GridAction::Changed);
        }
        if undo {
            g.undo_last();
            actions.push(GridAction::Changed);
        }
        if del && g.sel.is_some() {
            let rows = g.target_rows();
            g.toggle_delete(&rows);
            actions.push(GridAction::Changed);
        }
        if ins {
            let n = g.qr.columns.len();
            g.add_row(vec![None; n]);
        }
        if let Some((r, c)) = g.sel {
            if f2 && r < g.qr.rows.len() {
                g.select_all = true;
                g.editing = Some((r, c, g.qr.rows[r][c].clone().unwrap_or_default()));
            } else if let Some(t) = text {
                // typing on a selected cell starts editing with the typed text, like a spreadsheet
                if !t.chars().all(|ch| ch.is_control()) && r < g.qr.rows.len() && g.row_state[r] != RowState::Deleted {
                    g.select_all = false;
                    g.editing = Some((r, c, t));
                }
            }
        }
    }
    if up {
        g.move_sel(-1, 0);
    }
    if down {
        g.move_sel(1, 0);
    }
    if left {
        g.move_sel(0, -1);
    }
    if right {
        g.move_sel(0, 1);
    }
    if pgup {
        g.move_sel(-15, 0);
    }
    if pgdn {
        g.move_sel(15, 0);
    }
    if esc {
        g.clear_multi();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};

    fn grid() -> GridState {
        let qr = QueryResult {
            columns: vec!["id".into(), "name".into(), "city".into()],
            rows: (1..=6).map(|i| vec![Some(i.to_string()), Some(format!("n{}", i)), Some(format!("c{}", i))]).collect(),
            ..Default::default()
        };
        let mut g = GridState::new(qr, "t");
        g.editable = true;
        g
    }

    fn frame(ctx: &egui::Context, g: &mut GridState, events: Vec<Event>, t: f64) {
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 600.0)));
        input.time = Some(t);
        let mut evs = Vec::new();
        for e in events {
            if let Event::Key { modifiers, .. } | Event::PointerButton { modifiers, .. } = &e {
                evs.push(Event::ModifiersChanged(*modifiers));
            }
            evs.push(e);
        }
        input.events = evs;
        g.cell_rects.clear();
        let mut out = ctx.run_ui(input, |ui| {
            show(ui, g, "[NULL]", false);
        });
        out.textures_delta.clear();
    }

    fn key(k: Key, m: Modifiers) -> Event {
        Event::Key { key: k, physical_key: Some(k), pressed: true, repeat: false, modifiers: m }
    }

    fn click(ctx: &egui::Context, g: &mut GridState, rc: (usize, usize), t: &mut f64) {
        let pos = g.cell_rects.iter().find(|(k, _)| *k == rc).map(|(_, r)| r.center()).expect("cell visible");
        for pressed in [true, false] {
            *t += 0.05;
            frame(ctx, g, vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }], *t);
        }
    }

    #[test]
    fn click_type_enter_commits_and_moves_down() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        for _ in 0..3 {
            t += 0.05;
            frame(&ctx, &mut g, vec![], t);
        }
        click(&ctx, &mut g, (1, 1), &mut t);
        assert_eq!(g.sel, Some((1, 1)), "click selects the cell");
        // typing starts editing with the typed text
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Text("Z".into())], t);
        assert_eq!(g.editing.as_ref().map(|e| e.2.clone()), Some("Z".to_string()));
        t += 0.05;
        frame(&ctx, &mut g, vec![], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Text("ed".into())], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Enter, Modifiers::NONE)], t);
        assert!(g.editing.is_none(), "Enter ends editing");
        assert_eq!(g.qr.rows[1][1].as_deref(), Some("Zed"));
        assert!(matches!(g.row_state[1], RowState::Modified(_)));
        assert_eq!(g.sel, Some((2, 1)), "selection moves down");
        assert_eq!(g.pending_changes(), 1);
    }

    #[test]
    fn f2_tab_escape_and_undo() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        for _ in 0..3 {
            t += 0.05;
            frame(&ctx, &mut g, vec![], t);
        }
        click(&ctx, &mut g, (0, 0), &mut t);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::F2, Modifiers::NONE)], t);
        assert!(g.editing.is_some(), "F2 edits");
        t += 0.05;
        frame(&ctx, &mut g, vec![], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Escape, Modifiers::NONE)], t);
        assert!(g.editing.is_none(), "Esc cancels");
        assert_eq!(g.pending_changes(), 0);
        // edit and Tab -> moves right
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::F2, Modifiers::NONE)], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Text("9".into())], t);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Tab, Modifiers::NONE)], t);
        assert_eq!(g.sel, Some((0, 1)), "Tab moves right");
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("9"), "F2 selects the old text, typing replaces it");
        // undo
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Z, Modifiers::COMMAND)], t);
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("1"));
        assert_eq!(g.pending_changes(), 0);
    }

    fn rss_kb() -> usize {
        std::fs::read_to_string("/proc/self/statm").ok().and_then(|t| t.split_whitespace().nth(1).and_then(|v| v.parse::<usize>().ok())).unwrap_or(0) * 4
    }

    fn wide_rows(from: usize, n: usize) -> Vec<Vec<Option<String>>> {
        (from..from + n).map(|i| (0..12).map(|c| Some(if c == 5 { format!("{{\"id\":{},\"text\":\"{}\"}}", i, "lorem ipsum ".repeat(2000)) } else { format!("valor-{}-{}", i, c) })).collect()).collect()
    }

    /// Benchmark, run by hand: `cargo test measure_memory -- --ignored --nocapture`.
    /// Pages 200 -> 400 rows with ~24 kB values and scrolls the grid; RSS must stay flat once the page is loaded.
    #[test]
    #[ignore]
    fn measure_memory_while_paging() {
        let ctx = egui::Context::default();
        let qr = QueryResult { columns: (0..12).map(|c| format!("col{}", c)).collect(), rows: wide_rows(0, 200), has_more: true, ..Default::default() };
        let mut g = GridState::new(qr, "t");
        let mut t = 0.0;
        let mut last = 0;
        for step in 0..6 {
            let before = rss_kb();
            for r in (0..g.qr.rows.len()).step_by(5) {
                t += 0.02;
                let dy = if (r / 100) % 2 == 0 { -150.0 } else { 150.0 };
                let ev = vec![Event::PointerMoved(Pos2::new(300.0, 300.0)), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, dy), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE }];
                frame(&ctx, &mut g, ev, t);
            }
            last = rss_kb();
            eprintln!("rows={} rss {} -> {} kB", g.qr.rows.len(), before, last);
            if step == 0 {
                let more = wide_rows(g.qr.rows.len(), 200);
                let added = more.len();
                g.qr.rows.extend(more);
                g.row_state.extend(std::iter::repeat(RowState::Clean).take(added));
                g.dirty = true;
            }
        }
        let _ = last;
    }

    #[test]
    fn replacing_results_does_not_accumulate_memory() {
        let mk = |n: usize| GridState::new(QueryResult { columns: (0..12).map(|c| format!("c{}", c)).collect(), rows: wide_rows(0, n), ..Default::default() }, "t");
        let before = crate::memtrack::live_bytes();
        let mut g = mk(200);
        let one = crate::memtrack::live_bytes() - before;
        assert!(one > 1_000_000, "the result has real size ({} bytes)", one);
        for _ in 0..20 {
            g = mk(200); // a new execution replaces the previous result
            g.set_cell(0, 0, Some("edit".into())); // undo history and row state exist too
        }
        let held = crate::memtrack::live_bytes() - before;
        assert!(held < one * 3 / 2, "20 replacements hold {} bytes, one result is {}", held, one);
        drop(g);
        let left = crate::memtrack::live_bytes().saturating_sub(before);
        assert!(left < one / 20, "dropping the grid frees its data ({} bytes left of {})", left, one);
    }

    #[test]
    fn sorting_uses_keys_computed_once() {
        let qr = QueryResult { columns: vec!["a".into(), "b".into()], rows: vec![vec![Some("10".into()), Some("b".into())], vec![Some("9".into()), None], vec![None, Some("A".into())], vec![Some("100".into()), Some("c".into())]], ..Default::default() };
        let mut g = GridState::new(qr, "t");
        g.server_sort = false;
        g.sort = Some((0, true));
        g.reindex();
        assert_eq!(g.view_idx, vec![2, 1, 0, 3], "NULL first, then numeric order");
        g.sort = Some((1, false));
        g.reindex();
        assert_eq!(g.view_idx, vec![3, 0, 2, 1], "text descending, case-insensitive, NULL last");
    }

    #[test]
    fn cell_text_is_cut_before_layout() {
        assert_eq!(cell_display_text("a\nb\tc"), "a¶b c");
        let long = "é".repeat(5000);
        let t = cell_display_text(&long);
        assert_eq!(t.chars().count(), CELL_DISPLAY_CHARS + 1);
        assert!(t.ends_with('…'));
        assert_eq!(cell_display_text(&"x".repeat(CELL_DISPLAY_CHARS)).chars().count(), CELL_DISPLAY_CHARS);
    }

    #[test]
    fn value_panel_split_never_has_negative_sizes() {
        for (w, h) in [(0.0, 0.0), (-5.0, 10.0), (100.0, 400.0), (300.0, 400.0), (305.0, 400.0), (306.0, 400.0), (400.0, 30.0), (900.0, 500.0), (f32::NAN, 100.0)] {
            let full = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(w, h));
            if let Some((g, hd, p)) = split_value_panel(full, 520.0) {
                for r in [g, hd, p] {
                    assert!(r.width() >= 0.0 && r.height() >= 0.0, "{:?} for {}x{}", r, w, h);
                }
                assert!(g.width() >= VP_MIN_GRID - 0.01 && p.width() >= VP_MIN_PANEL - 0.01);
            } else {
                assert!(!(w >= 306.0 && h >= 60.0), "{}x{} must fit", w, h);
            }
        }
    }

    #[test]
    fn grid_with_value_panel_survives_tiny_areas() {
        let ctx = egui::Context::default();
        let mut g = grid();
        g.sel = Some((0, 0));
        let sizes = [(0.0f32, 0.0f32), (1.0, 1.0), (50.0, 30.0), (200.0, 100.0), (320.0, 200.0), (900.0, 500.0)];
        for (i, ((w, h), (panel, mode))) in sizes.into_iter().flat_map(|sz| [(sz, (true, ViewMode::Grid)), (sz, (false, ViewMode::Grid)), (sz, (false, ViewMode::Text)), (sz, (false, ViewMode::Record))]).enumerate() {
            g.mode = mode;
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))), time: Some(i as f64 * 0.05), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                show(ui, &mut g, "NULL", panel);
            });
            out.textures_delta.clear();
        }
    }

    #[test]
    fn delete_insert_and_multiselect() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        for _ in 0..3 {
            t += 0.05;
            frame(&ctx, &mut g, vec![], t);
        }
        click(&ctx, &mut g, (2, 1), &mut t);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Delete, Modifiers::NONE)], t);
        assert_eq!(g.row_state[2], RowState::Deleted);
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Delete, Modifiers::NONE)], t);
        assert_eq!(g.row_state[2], RowState::Clean, "Delete again restores");
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Insert, Modifiers::NONE)], t);
        assert_eq!(g.qr.rows.len(), 7);
        assert_eq!(g.row_state[3], RowState::New, "inserted after the selected row");
        assert_eq!(g.sel.map(|s| s.0), Some(3));
        t += 0.05;
        frame(&ctx, &mut g, vec![key(Key::Delete, Modifiers::NONE)], t);
        assert_eq!(g.qr.rows.len(), 6, "an unsaved new row is simply dropped");
        // paste a 2x2 block
        g.sel = Some((0, 1));
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Paste("a\tb\nc\td".into())], t);
        assert_eq!(g.qr.rows[0][1].as_deref(), Some("a"));
        assert_eq!(g.qr.rows[0][2].as_deref(), Some("b"));
        assert_eq!(g.qr.rows[1][1].as_deref(), Some("c"));
        assert_eq!(g.pending_changes(), 2);
    }

    #[test]
    fn new_rows_go_after_the_current_row() {
        let mut g = grid();
        g.sel = Some((2, 1));
        g.add_row(vec![None, Some("novo".into()), None]);
        assert_eq!(g.qr.rows.len(), 7);
        assert_eq!(g.qr.rows[3][1].as_deref(), Some("novo"));
        assert_eq!(g.row_state[3], RowState::New);
        assert_eq!(g.sel, Some((3, 1)));
        // sorted grids append at the end
        g.sort = Some((0, true));
        g.sel = Some((0, 0));
        g.add_row(vec![None; 3]);
        assert_eq!(g.row_state[7], RowState::New);
    }

    #[test]
    fn boolean_cycle_true_false_null() {
        let qr = QueryResult { columns: vec!["ok".into()], rows: vec![vec![Some("t".into())], vec![None], vec![Some("f".into())]], ..Default::default() };
        let mut g = GridState::new(qr, "b");
        g.editable = true;
        g.toggle_bool(0, true, &[0]);
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("f"), "true -> false keeps the t/f style");
        g.toggle_bool(0, true, &[0]);
        assert_eq!(g.qr.rows[0][0], None, "false -> NULL when nullable");
        g.toggle_bool(0, true, &[0]);
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("t"));
        g.set_cell(0, 0, Some("f".into()));
        g.toggle_bool(0, false, &[0]);
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("t"), "false -> true when NOT NULL");
        g.toggle_bool(0, true, &[1]);
        assert_eq!(g.qr.rows[1][0].as_deref(), Some("t"), "NULL -> true");
    }

    #[test]
    fn read_only_grid_ignores_typing() {
        let ctx = egui::Context::default();
        let mut g = grid();
        g.editable = false;
        let mut t = 0.0;
        for _ in 0..3 {
            t += 0.05;
            frame(&ctx, &mut g, vec![], t);
        }
        click(&ctx, &mut g, (1, 1), &mut t);
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Text("Z".into()), key(Key::Delete, Modifiers::NONE)], t);
        assert!(g.editing.is_none());
        assert_eq!(g.pending_changes(), 0);
    }

    fn click_mod(ctx: &egui::Context, g: &mut GridState, rc: (usize, usize), m: Modifiers, t: &mut f64) {
        let pos = g.cell_rects.iter().find(|(k, _)| *k == rc).map(|(_, r)| r.center()).expect("cell visible");
        for pressed in [true, false] {
            *t += 0.05;
            frame(ctx, g, vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: m }], *t);
        }
    }

    fn warm(ctx: &egui::Context, g: &mut GridState, t: &mut f64) {
        for _ in 0..3 {
            *t += 0.05;
            frame(ctx, g, vec![], *t);
        }
    }

    #[test]
    fn shift_click_selects_a_block_of_cells() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        warm(&ctx, &mut g, &mut t);
        click(&ctx, &mut g, (1, 0), &mut t);
        click_mod(&ctx, &mut g, (3, 1), Modifiers::SHIFT, &mut t);
        assert_eq!(g.rect_bounds(), Some((1, 3, 0, 1)));
        assert_eq!(g.selection_text().unwrap(), "2\tn2\n3\tn3\n4\tn4");
    }

    #[test]
    fn ctrl_click_toggles_rows() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        warm(&ctx, &mut g, &mut t);
        click(&ctx, &mut g, (0, 1), &mut t);
        click_mod(&ctx, &mut g, (2, 1), Modifiers::COMMAND, &mut t);
        click_mod(&ctx, &mut g, (4, 1), Modifiers::COMMAND, &mut t);
        assert_eq!(g.target_rows(), vec![0, 2, 4]);
        click_mod(&ctx, &mut g, (2, 1), Modifiers::COMMAND, &mut t);
        assert_eq!(g.target_rows(), vec![0, 4]);
        assert!(g.selection_text().unwrap().starts_with("1\tn1\tc1\n"));
    }

    #[test]
    fn drag_selects_a_block() {
        let ctx = egui::Context::default();
        let mut g = grid();
        let mut t = 0.0;
        warm(&ctx, &mut g, &mut t);
        let a = g.cell_rects.iter().find(|(k, _)| *k == (1, 0)).unwrap().1.center();
        let b = g.cell_rects.iter().find(|(k, _)| *k == (3, 2)).unwrap().1.center();
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::PointerMoved(a), Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], t);
        for p in [a + egui::vec2(12.0, 6.0), b - egui::vec2(3.0, 3.0), b] {
            t += 0.05;
            frame(&ctx, &mut g, vec![Event::PointerMoved(p)], t);
        }
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::PointerButton { pos: b, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], t);
        assert_eq!(g.rect_bounds(), Some((1, 3, 0, 2)), "{:?}", g.rect_sel);
    }

    #[test]
    fn column_selection_copies_the_whole_column_and_does_not_sort() {
        let mut g = grid();
        select_column(&mut g, 1, Modifiers::NONE);
        assert_eq!(g.sort, None);
        assert_eq!(g.selection_text().unwrap(), "n1\nn2\nn3\nn4\nn5\nn6");
        select_column(&mut g, 2, Modifiers::COMMAND);
        assert_eq!(g.selection_text().unwrap().lines().next().unwrap(), "n1\tc1");
        select_column(&mut g, 0, Modifiers::SHIFT);
        assert_eq!(g.sel_cols.len(), 3, "shift extends from the last clicked column");
    }

    #[test]
    fn block_paste_is_one_undo_step_and_single_value_fills_the_selection() {
        let mut g = grid();
        g.sel = Some((0, 1));
        g.paste("a\tb\nc\td\ne\tf");
        assert_eq!(g.qr.rows[2][2].as_deref(), Some("f"));
        g.undo_last();
        assert_eq!(g.qr.rows[0][1].as_deref(), Some("n1"), "one undo reverts the whole block");
        assert_eq!(g.qr.rows[2][2].as_deref(), Some("c3"));
        // 1x1 value over a selected block
        g.sel = Some((0, 1));
        g.rect_sel = Some(((0, 1), (2, 2)));
        g.paste("X");
        assert!((0..3).all(|r| g.qr.rows[r][1].as_deref() == Some("X") && g.qr.rows[r][2].as_deref() == Some("X")));
        assert_eq!(g.qr.rows[3][1].as_deref(), Some("n4"));
        g.undo_last();
        assert_eq!(g.qr.rows[0][1].as_deref(), Some("n1"));
    }

    #[test]
    fn block_paste_starts_at_the_top_left_of_the_selection() {
        let mut g = grid();
        g.sel = Some((3, 2));
        g.rect_sel = Some(((3, 2), (1, 1)));
        g.paste("p\tq");
        assert_eq!(g.qr.rows[1][1].as_deref(), Some("p"));
        assert_eq!(g.qr.rows[1][2].as_deref(), Some("q"));
    }

    #[test]
    fn read_only_grid_ignores_block_paste() {
        let ctx = egui::Context::default();
        let mut g = grid();
        g.editable = false;
        let mut t = 0.0;
        warm(&ctx, &mut g, &mut t);
        click(&ctx, &mut g, (0, 0), &mut t);
        t += 0.05;
        frame(&ctx, &mut g, vec![Event::Paste("z\tz".into())], t);
        assert_eq!(g.qr.rows[0][0].as_deref(), Some("1"));
    }

    #[test]
    fn value_panel_follows_external_changes_but_keeps_user_edits() {
        let mut g = grid();
        g.sync_value_buf(0, 1);
        assert_eq!(g.vp_buf, "n1");
        g.set_cell(0, 1, Some("novo".into()));
        g.sync_value_buf(0, 1);
        assert_eq!(g.vp_buf, "novo", "edit made in the grid shows up");
        g.undo_last();
        g.sync_value_buf(0, 1);
        assert_eq!(g.vp_buf, "n1", "undo shows up");
        g.vp_buf = "digitando".into();
        g.set_cell(0, 1, Some("outro".into()));
        g.sync_value_buf(0, 1);
        assert_eq!(g.vp_buf, "digitando", "text typed in the panel is not overwritten");
        g.sync_value_buf(1, 1);
        assert_eq!(g.vp_buf, "n2");
    }

    #[test]
    fn aggregates_over_selection_and_column() {
        let mut g = grid();
        g.qr.rows[1][1] = None;
        g.qr.rows[2][1] = Some("n1".into());
        // id column: 1..6
        select_column(&mut g, 0, Modifiers::NONE);
        let a = g.aggregates(None);
        assert_eq!((a.cells, a.count, a.distinct, a.nulls), (6, 6, 6, 0));
        assert_eq!(a.sum, Some(21.0));
        assert_eq!(a.avg, Some(3.5));
        assert_eq!((a.min.as_deref(), a.max.as_deref()), (Some("1"), Some("6")));
        // text column with a NULL and a duplicate
        let a = g.aggregates(Some(1));
        assert_eq!((a.cells, a.count, a.distinct, a.nulls), (6, 5, 4, 1));
        assert_eq!(a.sum, None);
        assert_eq!((a.min.as_deref(), a.max.as_deref()), (Some("n1"), Some("n6")));
        // a block
        g.clear_multi();
        g.rect_sel = Some(((0, 0), (1, 0)));
        assert_eq!(g.aggregates(None).sum, Some(3.0));
        assert_eq!(fmt_num(2.50), "2.5");
    }
}
