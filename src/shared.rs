//! Shared application state: connections, metadata cache, background query jobs.
use crate::db::*;
use crate::store::{HistoryEntry, Store};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct JobStmt {
    pub sql: String,
    pub limit: usize,
    pub offset: usize,
    pub tag: usize,
}

pub struct JobItem {
    pub sql: String,
    pub res: Result<QueryResult, String>,
    pub tag: usize,
}

pub enum JobMsg {
    Item(JobItem),
    Finished,
}

pub struct Job {
    pub rx: Receiver<JobMsg>,
    pub total: usize,
    pub done: usize,
    pub started: Instant,
}

pub fn spawn_job(sess: Arc<Mutex<Session>>, stmts: Vec<JobStmt>, ctx: egui::Context) -> Job {
    let (tx, rx) = channel();
    let total = stmts.len();
    std::thread::spawn(move || {
        {
        let mut s = sess.lock().unwrap_or_else(|e| e.into_inner());
        for st in stmts {
            let mut r = s.run(&st.sql, st.limit, st.offset).map_err(|e| e.to_string());
            let failed = r.is_err();
            if failed && crate::constants::SCRIPT_ERROR_STOP_ROLLBACK && !s.auto_commit() && s.tx_open {
                // SCRIPT_ERROR_HANDLING = STOP_ROLLBACK
                if s.rollback().is_ok() {
                    r = r.map_err(|e| format!("{}\n(a transação foi revertida)", e));
                }
            }
            let _ = tx.send(JobMsg::Item(JobItem { sql: st.sql, res: r, tag: st.tag }));
            ctx.request_repaint();
            if failed {
                break;
            }
        }
        }
        let _ = tx.send(JobMsg::Finished);
        ctx.request_repaint();
    });
    Job { rx, total, done: 0, started: Instant::now() }
}

pub enum Req {
    OpenSql { conn: Option<String>, text: String, title: Option<String> },
    OpenTable { conn: String, schema: String, table: String, view: bool },
    OpenEr { conn: String, schema: String, focus: Option<String> },
    Export { conn: String, sql: String, name: String, rows: Option<QueryResult> },
    Import { conn: String, schema: String, table: String },
    GenSql { title: String, text: String },
    ViewValue(String),
    EditConn(String),
    NewConn,
    DeleteConn(String),
    Params { tab: u64, names: Vec<String>, stmts: Vec<String>, new_tab: bool },
    ConfirmRun { tab: u64, stmts: Vec<String>, new_tab: bool, env: String },
    OpenSource { conn: String, schema: String, key: String, name: String, detail: String },
    ExportWith { conn: String, sql: String, opts: Box<crate::export::ExportOptions>, dir: String, file: String },
}

pub struct Shared {
    pub store: Store,
    pub persist: bool,
    pub sessions: HashMap<String, Arc<Mutex<Session>>>,
    pub snaps: HashMap<String, Arc<crate::db::TxSnap>>,
    /// project connection that needs a password the user has not given on this machine yet
    pub need_password: Option<String>,
    /// connections whose password prompt was dismissed: they are tried without a password
    pub password_declined: std::collections::HashSet<String>,
    pub schemas: HashMap<String, Vec<String>>,
    pub tables: HashMap<(String, String), Vec<(String, bool)>>,
    pub tmeta: HashMap<(String, String, String), TableMeta>,
    pub objects: HashMap<(String, String, String), Vec<ObjInfo>>,
    pub versions: HashMap<String, String>,
    pub status: String,
    pub requests: Vec<Req>,
    pub vars: Vec<(String, String)>,
    pub next_id: u64,
    pub ctx: egui::Context,
}

impl Shared {
    pub fn new(store: Store, persist: bool, ctx: egui::Context) -> Shared {
        Shared {
            store,
            persist,
            sessions: HashMap::new(),
            snaps: HashMap::new(),
            need_password: None,
            password_declined: Default::default(),
            schemas: HashMap::new(),
            tables: HashMap::new(),
            tmeta: HashMap::new(),
            objects: HashMap::new(),
            versions: HashMap::new(),
            status: "Pronto".into(),
            requests: Vec::new(),
            vars: Vec::new(),
            next_id: 1,
            ctx,
        }
    }

    pub fn save(&self) {
        if self.persist {
            self.store.save();
        }
    }

    pub fn cfg(&self, id: &str) -> Option<&ConnConfig> {
        self.store.connections.iter().find(|c| c.id == id)
    }

    pub fn conn_name(&self, id: &str) -> String {
        self.cfg(id).map(|c| c.display_name()).unwrap_or_else(|| "?".into())
    }

    pub fn kind(&self, id: &str) -> Kind {
        self.cfg(id).map(|c| c.kind).unwrap_or(Kind::Sqlite)
    }

    pub fn conn_type(&self, id: &str) -> crate::constants::ConnType {
        self.cfg(id).map(|c| c.conn_type).unwrap_or_default()
    }

    /// The connection a saved script was last executed on, if it still exists.
    pub fn script_conn(&self, file_name: &str) -> Option<String> {
        let id = self.store.script_conns.get(file_name)?;
        self.cfg(id).map(|c| c.id.clone())
    }

    /// Remembers that the script (identified by its file name) ran on `conn`.
    pub fn remember_script_conn(&mut self, file: Option<&std::path::Path>, conn: &str) {
        self.store.last_exec_conn = Some(conn.to_string());
        if let Some(name) = file.and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()) {
            self.store.script_conns.insert(name, conn.to_string());
        }
        self.save();
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    pub fn session(&self, id: &str) -> Option<Arc<Mutex<Session>>> {
        self.sessions.get(id).cloned()
    }

    pub fn connect(&mut self, id: &str) -> Result<(), String> {
        if self.sessions.contains_key(id) {
            return Ok(());
        }
        let cfg = self.cfg(id).cloned().ok_or("Conexão não encontrada")?;
        // a connection that came with a project folder has no password on this machine yet: ask for it
        if !cfg.project.is_empty() && cfg.password.is_empty() && !matches!(cfg.kind, Kind::Sqlite) && !self.password_declined.contains(id) {
            self.need_password = Some(id.to_string());
            return Err("Senha necessária: informe-a na janela que se abriu".into());
        }
        match Session::open(&cfg) {
            Ok(mut s) => {
                let v = s.version();
                let v = if s.legacy_auth { format!("{} (senha em formato antigo)", v) } else { v };
                self.versions.insert(id.to_string(), v);
                self.snaps.insert(id.to_string(), s.snap.clone());
                self.sessions.insert(id.to_string(), Arc::new(Mutex::new(s)));
                self.status = format!("Conectado a {}", cfg.display_name());
                Ok(())
            }
            Err(e) => {
                let e = clean_err(e).to_string();
                self.status = format!("Falha ao conectar: {}", e);
                Err(e)
            }
        }
    }

    pub fn test(cfg: &ConnConfig) -> Result<String, String> {
        match Session::open(cfg) {
            Ok(mut s) => {
                let v = s.version();
                Ok(if s.legacy_auth { format!("{} — o servidor usa senha em formato antigo (pré-4.1); o app habilitou isso automaticamente", v) } else { v })
            }
            Err(e) => Err(clean_err(e).to_string()),
        }
    }

    pub fn disconnect(&mut self, id: &str) {
        self.sessions.remove(id);
        self.snaps.remove(id);
        self.schemas.remove(id);
        self.tables.retain(|k, _| k.0 != id);
        self.tmeta.retain(|k, _| k.0 != id);
        self.objects.retain(|k, _| k.0 != id);
        self.status = format!("Desconectado de {}", self.conn_name(id));
    }

    /// Drops the session and every cache of the connection, then opens a new session from the saved configuration.
    pub fn reconnect(&mut self, id: &str) -> Result<(), String> {
        self.disconnect(id);
        self.connect(id)
    }

    /// Writes made in a manual/smart transaction that are not committed yet.
    pub fn has_pending(&self, id: &str) -> Option<u64> {
        match self.tx_info(id) {
            Some((m, true, n, rows)) if m != crate::db::TxMode::Auto && n > 0 => Some(rows),
            _ => None,
        }
    }

    pub fn refresh(&mut self, id: &str) {
        self.schemas.remove(id);
        self.tables.retain(|k, _| k.0 != id);
        self.tmeta.retain(|k, _| k.0 != id);
        self.objects.retain(|k, _| k.0 != id);
    }

    pub fn ensure_schemas(&mut self, id: &str) -> Vec<String> {
        if let Some(v) = self.schemas.get(id) {
            return v.clone();
        }
        let v = self
            .session(id)
            .and_then(|s| s.lock().unwrap_or_else(|e| e.into_inner()).schemas().ok())
            .unwrap_or_default();
        self.schemas.insert(id.to_string(), v.clone());
        v
    }

    pub fn ensure_tables(&mut self, id: &str, schema: &str) -> Vec<(String, bool)> {
        let key = (id.to_string(), schema.to_string());
        if let Some(v) = self.tables.get(&key) {
            return v.clone();
        }
        let v = self
            .session(id)
            .and_then(|s| s.lock().unwrap_or_else(|e| e.into_inner()).tables(schema).ok())
            .unwrap_or_default();
        self.tables.insert(key, v.clone());
        v
    }

    pub fn ensure_objects(&mut self, id: &str, schema: &str, key: &str) -> Vec<ObjInfo> {
        let k = (id.to_string(), schema.to_string(), key.to_string());
        if let Some(v) = self.objects.get(&k) {
            return v.clone();
        }
        let v = self.session(id).and_then(|s| s.lock().unwrap_or_else(|e| e.into_inner()).objects(key, schema).ok()).unwrap_or_default();
        self.objects.insert(k, v.clone());
        v
    }

    pub fn ensure_tmeta(&mut self, id: &str, schema: &str, table: &str) -> TableMeta {
        let key = (id.to_string(), schema.to_string(), table.to_string());
        if let Some(v) = self.tmeta.get(&key) {
            return v.clone();
        }
        let v = self
            .session(id)
            .and_then(|s| s.lock().unwrap_or_else(|e| e.into_inner()).table_meta(schema, table).ok())
            .unwrap_or_default();
        self.tmeta.insert(key, v.clone());
        v
    }

    pub fn default_schema(&mut self, id: &str) -> String {
        let cfg_db = self.cfg(id).map(|c| c.database.clone()).unwrap_or_default();
        let kind = self.kind(id);
        let sch = self.ensure_schemas(id);
        match kind {
            Kind::Sqlite => "main".into(),
            Kind::Postgres => {
                if sch.iter().any(|s| s == "public") {
                    "public".into()
                } else {
                    sch.first().cloned().unwrap_or_default()
                }
            }
            Kind::SqlServer => {
                if sch.iter().any(|s| s == "dbo") {
                    "dbo".into()
                } else {
                    sch.first().cloned().unwrap_or_default()
                }
            }
            Kind::Mongo => {
                if !cfg_db.is_empty() {
                    cfg_db
                } else {
                    sch.first().cloned().unwrap_or_default()
                }
            }
            Kind::Mysql => {
                if !cfg_db.is_empty() {
                    cfg_db
                } else {
                    sch.first().cloned().unwrap_or_default()
                }
            }
        }
    }

    pub fn qualified(&self, id: &str, schema: &str, table: &str) -> String {
        let k = self.kind(id);
        if k == Kind::Mongo {
            format!("db.getSiblingDB({}).getCollection({})", serde_json::to_string(schema).unwrap_or_default(), serde_json::to_string(table).unwrap_or_default())
        } else if k == Kind::Sqlite {
            k.quote(table)
        } else {
            format!("{}.{}", k.quote(schema), k.quote(table))
        }
    }

    pub fn history(&mut self, conn: &str, sql: &str, ms: u128, rows: String, ok: bool) {
        self.store.history.push(HistoryEntry { when: chrono::Utc::now().timestamp(), conn: self.conn_name(conn), sql: sql.to_string(), ms, rows, ok });
        let n = self.store.history.len();
        if n > 2000 {
            self.store.history.drain(0..n - 2000);
        }
    }

    pub fn tx_info(&self, id: &str) -> Option<(crate::db::TxMode, bool, usize, u64)> {
        // read from the lock-free snapshot: a running job holds the session lock
        let s = self.sessions.get(id)?;
        let snap = self.snaps.get(id)?;
        let _ = s;
        Some(snap.get())
    }

    pub fn req(&mut self, r: Req) {
        self.requests.push(r);
    }

    pub fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_remembers_the_connection_it_last_ran_on() {
        let mk = |name: &str| {
            let p = std::env::temp_dir().join(format!("dbr-test-conn-{}.sqlite", name));
            crate::sample::create(&p).unwrap();
            let mut c = ConnConfig::new(Kind::Sqlite);
            c.name = name.into();
            c.database = p.to_string_lossy().to_string();
            c
        };
        let (ca, cb) = (mk("a"), mk("b"));
        let (a, b) = (ca.id.clone(), cb.id.clone());
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(ca);
        sh.store.connections.push(cb);
        sh.connect(&a).unwrap();
        sh.connect(&b).unwrap();
        let mut t = crate::tabs::SqlTab::new(Some(a.clone()), "select 1".into(), "t".into());
        t.file = Some(std::path::PathBuf::from("/tmp/scripts/Script-1.sql"));
        assert_eq!(sh.script_conn("Script-1.sql"), None, "never executed yet");
        t.start_confirmed(&mut sh, &a, vec!["select 1".into()], false);
        assert_eq!(sh.script_conn("Script-1.sql"), Some(a.clone()));
        t.start_confirmed(&mut sh, &b, vec!["select 1".into()], false);
        assert_eq!(sh.script_conn("Script-1.sql"), Some(b.clone()), "the last execution wins");
        assert_eq!(sh.store.last_exec_conn, Some(b.clone()));
        // another script keeps its own connection
        assert_eq!(sh.script_conn("Script-2.sql"), None);
        // a deleted connection is forgotten
        sh.store.connections.retain(|c| c.id != b);
        assert_eq!(sh.script_conn("Script-1.sql"), None);
    }

    #[test]
    fn failing_script_rolls_back_the_open_transaction() {
        let p = std::env::temp_dir().join("dbr-test-rollback.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        let mut s = Session::open(&c).unwrap();
        s.set_auto_commit(false).unwrap();
        let sess = Arc::new(Mutex::new(s));
        let stmts = vec![
            JobStmt { sql: "DELETE FROM order_items".into(), limit: 0, offset: 0, tag: 0 },
            JobStmt { sql: "SELECT * FROM tabela_que_nao_existe".into(), limit: 0, offset: 0, tag: 0 },
            JobStmt { sql: "DELETE FROM orders".into(), limit: 0, offset: 0, tag: 0 },
        ];
        let job = spawn_job(sess.clone(), stmts, egui::Context::default());
        let mut errors = Vec::new();
        let mut items = 0;
        loop {
            match job.rx.recv().unwrap() {
                JobMsg::Item(it) => {
                    items += 1;
                    if let Err(e) = it.res {
                        errors.push(e);
                    }
                }
                JobMsg::Finished => break,
            }
        }
        assert_eq!(items, 2, "the script stops at the first error");
        assert!(errors[0].contains("a transação foi revertida"), "{:?}", errors);
        let mut g = sess.lock().unwrap();
        assert!(!g.tx_open);
        let n = g.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].clone().unwrap();
        assert_ne!(n, "0", "the DELETE was rolled back");
    }

    #[test]
    fn pending_rows_show_up_in_tx_info_after_a_job() {
        let p = std::env::temp_dir().join("dbr-test-txinfo.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        c.set_mode(crate::db::TxMode::Smart);
        let id = c.id.clone();
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(c);
        sh.connect(&id).unwrap();
        let sess = sh.session(&id).unwrap();
        let stmts = vec![
            JobStmt { sql: "SELECT * FROM orders".into(), limit: 200, offset: 0, tag: 0 },
            JobStmt { sql: "UPDATE products SET in_stock = 1 WHERE product_id <= 3".into(), limit: 200, offset: 0, tag: 0 },
        ];
        let job = spawn_job(sess, stmts, egui::Context::default());
        while !matches!(job.rx.recv().unwrap(), JobMsg::Finished) {}
        let (mode, open, n, rows) = sh.tx_info(&id).unwrap();
        assert_eq!((mode, open, n, rows), (crate::db::TxMode::Smart, true, 1, 3));
    }

    #[test]
    fn reconnect_replaces_the_session_and_clears_caches() {
        let p = std::env::temp_dir().join("dbr-test-reconnect.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        let id = c.id.clone();
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(c);
        sh.connect(&id).unwrap();
        let before = sh.session(&id).unwrap();
        sh.ensure_tables(&id, "main");
        assert!(!sh.tables.is_empty());
        sh.reconnect(&id).unwrap();
        let after = sh.session(&id).unwrap();
        assert!(!Arc::ptr_eq(&before, &after), "a new session was opened");
        assert!(sh.tables.is_empty(), "caches were dropped");
        // a failing reconnect leaves the connection closed and reports the error
        sh.store.connections[0].database = "/nonexistent-dir/x/y.sqlite".into();
        assert!(sh.reconnect(&id).is_err());
        assert!(!sh.is_connected(&id));
    }

    #[test]
    fn pending_changes_are_reported_for_manual_and_smart_only() {
        let p = std::env::temp_dir().join("dbr-test-pending.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        c.set_mode(crate::db::TxMode::Smart);
        let id = c.id.clone();
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(c);
        sh.connect(&id).unwrap();
        assert_eq!(sh.has_pending(&id), None);
        sh.session(&id).unwrap().lock().unwrap().run("UPDATE products SET in_stock = 1 WHERE product_id <= 2", 0, 0).unwrap();
        assert_eq!(sh.has_pending(&id), Some(2));
    }

    #[test]
    fn running_after_a_disconnect_reopens_the_session() {
        let p = std::env::temp_dir().join("dbr-test-after-disc.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        let id = c.id.clone();
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(c);
        sh.connect(&id).unwrap();
        sh.disconnect(&id);
        assert!(!sh.is_connected(&id));
        // what `start_confirmed` and `reload` do before running
        sh.connect(&id).unwrap();
        let r = sh.session(&id).unwrap().lock().unwrap().run("SELECT COUNT(*) FROM orders", 0, 0).unwrap();
        assert_eq!(r.rows.len(), 1);
    }

    #[test]
    fn project_connection_without_a_local_password_asks_for_it() {
        let mut c = ConnConfig::new(Kind::Postgres);
        c.project = "proj-1".into();
        let id = c.id.clone();
        let mut sh = Shared::new(crate::store::Store::memory(), false, egui::Context::default());
        sh.store.connections.push(c);
        let e = sh.connect(&id).unwrap_err();
        assert!(e.contains("Senha"), "{}", e);
        assert_eq!(sh.need_password.as_deref(), Some(id.as_str()));
        // after declining the prompt it tries without a password (and fails on the network, not on the prompt)
        sh.need_password = None;
        sh.password_declined.insert(id.clone());
        let _ = sh.connect(&id);
        assert!(sh.need_password.is_none());
    }
}
