//! The open tabs, kept between sessions in `session.json`, in the configuration folder (never in a project:
//! it may hold unsaved script text).

use crate::grid::ViewMode;
use crate::shared::Shared;
use crate::tabs::{ErTab, SqlTab, TableSub, TableTab, Tab, TabKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SESSION_FORMAT: u32 = 1;

/// Minimum time between two automatic writes of the session.
pub const SAVE_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SavedTab {
    Welcome,
    Sql {
        #[serde(default)]
        conn: Option<String>,
        #[serde(default)]
        file: Option<PathBuf>,
        #[serde(default)]
        title: String,
        /// kept only for unsaved text and tabs without a file; a saved script is read again from its file
        #[serde(default)]
        text: String,
        #[serde(default)]
        cursor: (usize, usize),
        #[serde(default)]
        dirty: bool,
        /// vertical scroll of the editor
        #[serde(default)]
        scroll: f32,
    },
    Table {
        conn: String,
        schema: String,
        table: String,
        #[serde(default)]
        view: bool,
        #[serde(default)]
        sub: String,
        #[serde(default)]
        sort: Option<(usize, bool)>,
        #[serde(default)]
        mode: String,
    },
    Er {
        conn: String,
        schema: String,
        #[serde(default)]
        focus: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Session {
    pub format_version: u32,
    pub tabs: Vec<SavedTab>,
    pub active: usize,
}

pub fn path() -> PathBuf {
    crate::store::config_dir().join("session.json")
}

fn sub_name(s: TableSub) -> &'static str {
    match s {
        TableSub::Properties => "properties",
        TableSub::Data => "data",
        TableSub::Ddl => "ddl",
        TableSub::Diagram => "diagram",
    }
}

fn sub_of(s: &str) -> TableSub {
    match s {
        "properties" => TableSub::Properties,
        "ddl" => TableSub::Ddl,
        "diagram" => TableSub::Diagram,
        _ => TableSub::Data,
    }
}

fn mode_name(m: ViewMode) -> &'static str {
    match m {
        ViewMode::Grid => "grid",
        ViewMode::Text => "text",
        ViewMode::Record => "record",
    }
}

fn mode_of(s: &str) -> ViewMode {
    match s {
        "text" => ViewMode::Text,
        "record" => ViewMode::Record,
        _ => ViewMode::Grid,
    }
}

/// The state of the open tabs, as it would be restored.
pub fn capture(tabs: &[Tab], active: usize) -> Session {
    let tabs = tabs
        .iter()
        .map(|t| match &t.kind {
            TabKind::Welcome => SavedTab::Welcome,
            TabKind::Sql(s) => {
                let keep_text = s.dirty || s.file.is_none();
                SavedTab::Sql { conn: s.conn.clone(), file: s.file.clone(), title: s.title.clone(), text: if keep_text { s.text.clone() } else { String::new() }, cursor: s.cursor, dirty: s.dirty, scroll: s.scroll_y }
            }
            TabKind::Table(tt) => {
                // a restored tab that was never shown still has the view of the previous session
                let (sort, mode) = match (&tt.grid, tt.restore_view) {
                    (Some(g), _) => (g.sort, g.mode),
                    (None, Some(v)) => v,
                    (None, None) => (None, ViewMode::Grid),
                };
                SavedTab::Table { conn: tt.conn.clone(), schema: tt.schema.clone(), table: tt.table.clone(), view: tt.view, sub: sub_name(tt.sub).into(), sort, mode: mode_name(mode).into() }
            }
            TabKind::Er(e) => SavedTab::Er { conn: e.conn.clone(), schema: e.schema.clone(), focus: e.focus.clone() },
        })
        .collect();
    Session { format_version: SESSION_FORMAT, tabs, active }
}

/// Rebuilds the tabs without opening any connection. Table and diagram tabs of connections that no longer exist
/// are left out; a script tab whose connection is gone comes back without connection (its text is the user's).
/// A saved script whose file was deleted is left out. Returns the tabs and the index of the active one.
pub fn restore(s: &Session, sh: &Shared) -> (Vec<TabKind>, usize) {
    let mut out = Vec::new();
    let mut active = 0;
    for (i, t) in s.tabs.iter().enumerate() {
        if i == s.active {
            active = out.len();
        }
        let kind = match t {
            SavedTab::Welcome => Some(TabKind::Welcome),
            SavedTab::Sql { conn, file, title, text, cursor, dirty, scroll } => {
                let text = if *dirty || file.is_none() { Some(text.clone()) } else { file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()) };
                text.map(|text| {
                    let conn = conn.clone().filter(|c| sh.cfg(c).is_some());
                    let n = text.chars().count();
                    let mut st = SqlTab::new(conn, text, title.clone());
                    st.file = file.clone();
                    st.dirty = *dirty;
                    st.cursor = (cursor.0.min(n), cursor.1.min(n));
                    // the editor widget starts empty: the cursor, the scroll and the keyboard go back to where they were
                    st.set_cursor = Some(st.cursor.0);
                    st.scroll_y = *scroll;
                    st.set_scroll = Some(*scroll);
                    st.focus_editor = true;
                    st.new_script = st.text.trim().is_empty();
                    TabKind::Sql(Box::new(st))
                })
            }
            SavedTab::Table { conn, schema, table, view, sub, sort, mode } => sh.cfg(conn).map(|_| {
                let mut tt = TableTab::restored(sh, conn, schema, table, *view);
                tt.sub = sub_of(sub);
                tt.restore_view = Some((*sort, mode_of(mode)));
                TabKind::Table(Box::new(tt))
            }),
            SavedTab::Er { conn, schema, focus } => sh.cfg(conn).map(|_| TabKind::Er(Box::new(ErTab::restored(conn, schema, focus.as_deref())))),
        };
        if let Some(k) = kind {
            out.push(k);
        }
    }
    let active = active.min(out.len().saturating_sub(1));
    (out, active)
}

/// Reads the session. A missing file is no session; an unreadable one (or of an unknown format) is deleted.
pub fn read(path: &Path) -> Option<Session> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Session>(&text) {
        Ok(s) if s.format_version == SESSION_FORMAT => Some(s),
        _ => {
            let _ = std::fs::remove_file(path);
            None
        }
    }
}

pub fn to_json(s: &Session) -> String {
    serde_json::to_string_pretty(s).unwrap_or_default()
}

/// Writes through a temporary file and a rename, so that a crash never leaves half a session; readable only by
/// the user, since it may hold unsaved script text.
pub fn write(path: &Path, json: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ConnConfig, Kind};
    use crate::store::Store;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dbr-test-session-{}-{}", tag, crate::store::new_project_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn shared_with_conn() -> (Shared, String) {
        let mut sh = Shared::new(Store::memory(), false, egui::Context::default());
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = "/caminho/que/nao/existe.sqlite".into();
        let id = c.id.clone();
        sh.store.connections.push(c);
        (sh, id)
    }

    fn sample_tabs(sh: &Shared, id: &str, dir: &Path) -> Vec<Tab> {
        let saved = dir.join("salvo.sql");
        std::fs::write(&saved, "select 1").unwrap();
        let mut clean = SqlTab::new(Some(id.into()), "select 1".into(), "salvo".into());
        clean.file = Some(saved);
        clean.cursor = (3, 5);
        let mut dirty = SqlTab::new(Some(id.into()), "select 2 -- não gravado".into(), "sujo".into());
        dirty.file = Some(dir.join("sujo.sql"));
        dirty.dirty = true;
        dirty.cursor = (7, 7);
        dirty.scroll_y = 120.0;
        let mut tt = TableTab::restored(sh, id, "main", "customers", false);
        tt.sub = TableSub::Ddl;
        tt.restore_view = Some((Some((2, false)), ViewMode::Record));
        vec![
            Tab { id: 1, kind: TabKind::Welcome },
            Tab { id: 2, kind: TabKind::Sql(Box::new(clean)) },
            Tab { id: 3, kind: TabKind::Sql(Box::new(dirty)) },
            Tab { id: 4, kind: TabKind::Table(Box::new(tt)) },
            Tab { id: 5, kind: TabKind::Er(Box::new(ErTab::restored(id, "main", Some("orders")))) },
        ]
    }

    #[test]
    fn every_kind_of_tab_round_trips_without_connecting() {
        let d = temp("roundtrip");
        let (sh, id) = shared_with_conn();
        let tabs = sample_tabs(&sh, &id, &d);
        let s = capture(&tabs, 2);
        let f = d.join("session.json");
        write(&f, &to_json(&s)).unwrap();
        let back = read(&f).expect("the session reads back");
        assert_eq!(back, s);
        let (kinds, active) = restore(&back, &sh);
        assert_eq!(active, 2, "the same tab is active");
        assert!(sh.session(&id).is_none(), "restoring opens no connection");
        // restoring and capturing again gives the same session: nothing is lost on the way
        let again: Vec<Tab> = kinds.into_iter().enumerate().map(|(i, kind)| Tab { id: i as u64, kind }).collect();
        assert_eq!(capture(&again, active), s);
        match &again[2].kind {
            TabKind::Sql(t) => {
                assert_eq!((t.text.as_str(), t.dirty, t.cursor), ("select 2 -- não gravado", true, (7, 7)), "unsaved text comes back marked as changed");
                assert_eq!((t.set_cursor, t.set_scroll, t.focus_editor), (Some(7), Some(120.0), true), "the editor goes back to the same place");
            }
            _ => panic!("script tab expected"),
        }
        match &again[3].kind {
            TabKind::Table(t) => assert!(t.meta_pending && t.grid.is_none() && t.sub == TableSub::Ddl, "the table loads only when shown"),
            _ => panic!("table tab expected"),
        }
        match &again[4].kind {
            TabKind::Er(e) => assert!(e.pending && e.boxes.is_empty()),
            _ => panic!("diagram tab expected"),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn saved_scripts_are_read_again_from_their_file() {
        let d = temp("reread");
        let (sh, id) = shared_with_conn();
        let s = capture(&sample_tabs(&sh, &id, &d), 0);
        match &s.tabs[1] {
            SavedTab::Sql { text, .. } => assert!(text.is_empty(), "a saved script's text is not duplicated in the session"),
            _ => panic!(),
        }
        std::fs::write(d.join("salvo.sql"), "select 'mudou'").unwrap();
        let (kinds, _) = restore(&s, &sh);
        match &kinds[1] {
            TabKind::Sql(t) => assert_eq!(t.text, "select 'mudou'"),
            _ => panic!(),
        }
        // the file was deleted: the tab is left out
        std::fs::remove_file(d.join("salvo.sql")).unwrap();
        let (kinds, _) = restore(&s, &sh);
        assert_eq!(kinds.len(), 4);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn tabs_of_removed_connections_are_left_out() {
        let d = temp("removed");
        let (mut sh, id) = shared_with_conn();
        let s = capture(&sample_tabs(&sh, &id, &d), 4);
        sh.store.connections.clear();
        let (kinds, active) = restore(&s, &sh);
        // welcome and the two scripts stay (the scripts lose their connection); table and diagram go
        assert_eq!(kinds.len(), 3);
        assert!(kinds.iter().all(|k| !matches!(k, TabKind::Table(_) | TabKind::Er(_))));
        assert!(kinds.iter().all(|k| !matches!(k, TabKind::Sql(t) if t.conn.is_some())));
        assert_eq!(active, 2, "the active tab was left out: the last one before it is active");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_or_broken_session_files() {
        let d = temp("broken");
        let f = d.join("session.json");
        assert!(read(&f).is_none(), "no file, no session");
        std::fs::write(&f, "{ isto não é json").unwrap();
        assert!(read(&f).is_none());
        assert!(!f.exists(), "an unreadable session is discarded");
        std::fs::write(&f, r#"{"format_version": 99, "tabs": [], "active": 0}"#).unwrap();
        assert!(read(&f).is_none());
        assert!(!f.exists(), "an unknown format is discarded");
        let _ = std::fs::remove_dir_all(&d);
    }
}
