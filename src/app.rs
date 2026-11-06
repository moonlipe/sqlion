//! Main window: menus, toolbar, database navigator, tabs.
use crate::shared::*;
use crate::db::*;
use crate::icons::{self, Icon};
use crate::theme::pal;
use crate::dialogs::Dialog;
use crate::store::{self, Store};
use crate::tabs::*;
use egui::collapsing_header::CollapsingState;
use egui::{Color32, Key, Modifiers, RichText, Ui};

#[derive(Clone, PartialEq)]
pub enum NavSel {
    Conn(String),
    Schema(String, String),
    Table(String, String, String, bool),
}

pub struct App {
    pub sh: Shared,
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub dialog: Option<Dialog>,
    pub nav_filter: String,
    pub nav_sel: Option<NavSel>,
    pub screenshot: Option<std::path::PathBuf>,
    pub frames: u32,
    pub shot_requested: bool,
    pub toast: Option<(String, std::time::Instant)>,
    pub applied_theme: Option<(bool, f32)>,
    pub scripts_dir: std::path::PathBuf,
    pub nav_w: f32,
    pub demo: bool,
    pub git_cache: Option<(std::time::Instant, Vec<(String, String)>)>,
    pub last_scripts_dir: std::path::PathBuf,
}

pub fn setup_style(ctx: &egui::Context, dark: bool, size: f32) {
    crate::theme::apply(ctx, dark, size);
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<std::path::PathBuf>) -> App {
        let demo = screenshot.is_some();
        let store = if demo { Store::memory() } else { Store::load() };
        let scripts_dir = store::config_dir().join("scripts");
        let _ = std::fs::create_dir_all(&scripts_dir);
        let mut sh = Shared::new(store, !demo, cc.egui_ctx.clone());
        if std::env::var("DBR_DARK").is_ok() {
            sh.store.prefs.dark = true;
        }
        if std::env::var("DBR_VP").is_ok() {
            sh.store.prefs.show_value_panel = true;
        }
        if std::env::var("DBR_LIGHT").is_ok() {
            sh.store.prefs.dark = false;
        }
        let mut tabs = Vec::new();
        let mut nav_sel = None;
        if demo {
            // Demonstration state for the automatic screenshot: a sample database and one query run.
            let path = std::env::temp_dir().join("sqlion-sample.sqlite");
            if crate::sample::create(&path).is_ok() {
                let mut cfg = ConnConfig::new(Kind::Sqlite);
                cfg.name = "Amostra (SQLite)".into();
                cfg.database = path.to_string_lossy().to_string();
                let id = cfg.id.clone();
                sh.store.connections.push(cfg);
                let _ = sh.connect(&id);
                nav_sel = Some(NavSel::Conn(id.clone()));
                let sql = "SELECT c.customer_id, c.first_name, c.last_name, c.country,\n       COUNT(o.order_id) AS orders\nFROM customers c\nLEFT JOIN orders o ON o.customer_id = c.customer_id\nWHERE c.country <> 'Chile'\nGROUP BY c.customer_id\nORDER BY orders DESC;";
                let mut t = SqlTab::new(Some(id.clone()), sql.into(), "<Amostra (SQLite)> Script-1".into());
                t.cursor = (10, 10);
                t.execute(&mut sh, 1, false, false, false);
                tabs.push(Tab { id: 1, kind: TabKind::Sql(Box::new(t)) });
                let tt = TableTab::new(&mut sh, &id, "main", "customers", false);
                tabs.push(Tab { id: 2, kind: TabKind::Table(Box::new(tt)) });
                let open_ids: Vec<egui::Id> = vec![egui::Id::new(("conn", &id)), egui::Id::new(("grp", &id, "main", "T")), egui::Id::new(("tbl", &id, "main", "customers")), egui::Id::new(("objfolder", &id, "main", "index")), egui::Id::new(("objfolder", &id, "main", "trigger"))];
                for oid in open_ids {
                    let mut s = CollapsingState::load_with_default_open(&cc.egui_ctx, oid, true);
                    s.set_open(true);
                    s.store(&cc.egui_ctx);
                }
            }
            sh.next_id = 3;
        } else {
            tabs.push(Tab { id: 1, kind: TabKind::Welcome });
            sh.next_id = 2;
        }
        let dark = sh.store.prefs.dark;
        let size = sh.store.prefs.font_size;
        crate::theme::install_fonts(&cc.egui_ctx);
        setup_style(&cc.egui_ctx, dark, size);
        let mut dialog = None;
        if let Ok(which) = std::env::var("DBR_DEMO_DIALOG") {
            // preview helper: DBR_DEMO_DIALOG=mongo|export
            let mut c = ConnConfig::new(Kind::Mongo);
            c.flavor = "MongoDB".into();
            if which == "prefs" {
                dialog = Some(Dialog::Prefs);
            }
            if which == "about" {
                dialog = Some(Dialog::About);
            }
            if which == "conn" {
                let mut c = ConnConfig::new(Kind::Postgres);
                c.name = "Financeiro (prod)".into();
                c.color = Some([190, 60, 60]);
                c.icon = Some("lock".into());
                dialog = Some(Dialog::Conn { cfg: Box::new(c), is_new: true, msg: None, pass: String::new() });
            }
            if which == "mongo" {
                dialog = Some(Dialog::Conn { cfg: Box::new(c), is_new: true, msg: None, pass: String::new() });
            }
        }
        let mut active = if demo { 1 } else { 0 };
        if demo && std::env::var("DBR_TAB").map(|v| v == "welcome").unwrap_or(false) {
            tabs.push(Tab { id: 9999, kind: TabKind::Welcome });
            active = tabs.len() - 1;
        }
        App {
            dialog,
            sh,
            active,
            tabs,
            nav_filter: String::new(),
            nav_sel,
            screenshot,
            frames: 0,
            shot_requested: false,
            toast: None,
            applied_theme: Some((dark, size)),
            scripts_dir,
            nav_w: 280.0,
            demo,
            git_cache: None,
            last_scripts_dir: std::path::PathBuf::new(),
        }
    }

    // ------------------------------------------------------------ helpers
    pub fn open_sql(&mut self, conn: Option<String>, text: String, title: Option<String>) {
        let n = self.tabs.iter().filter(|t| matches!(t.kind, TabKind::Sql(_))).count() + 1;
        let cname = conn.as_ref().map(|c| self.sh.conn_name(c)).unwrap_or_else(|| "Sem conexão".into());
        let title_given = title.is_some();
        let title = title.unwrap_or_else(|| format!("<{}> Script-{}", cname, n));
        let path = if self.sh.persist { Some(self.next_script_path()) } else { None };
        // new scripts are named like DBeaver does: Script.sql, Script-1.sql, Script-2.sql ... (SQLEditorUtils.java:258)
        let title = match (&path, title_given) {
            (Some(p), false) => format!("<{}> {}", cname, p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
            _ => title,
        };
        let mut t = SqlTab::new(conn, text, title);
        t.new_script = text_is_empty_marker(&t.text);
        t.file = path;
        let id = self.sh.fresh_id();
        self.tabs.push(Tab { id, kind: TabKind::Sql(Box::new(t)) });
        self.active = self.tabs.len() - 1;
    }

    /// First free `Script.sql`, `Script-1.sql`... in the scripts folder, also skipping names of open tabs.
    fn next_script_path(&self) -> std::path::PathBuf {
        let used: Vec<std::path::PathBuf> = self.tabs.iter().filter_map(|t| if let TabKind::Sql(s) = &t.kind { s.file.clone() } else { None }).collect();
        for n in 0.. {
            let name = if n == 0 { "Script.sql".to_string() } else { format!("Script-{}.sql", n) };
            let p = self.scripts_dir.join(name);
            if !p.exists() && !used.contains(&p) {
                return p;
            }
        }
        unreachable!()
    }

    /// Opens a saved script (or focuses the tab where it is already open), on the connection it last ran on.
    pub fn open_script_file(&mut self, p: &std::path::Path) {
        if let Some(i) = self.tabs.iter().position(|t| matches!(&t.kind, TabKind::Sql(s) if s.file.as_deref() == Some(p))) {
            self.active = i;
            return;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if let Ok(t) = std::fs::read_to_string(p) {
            let c = self.sh.script_conn(&name).or_else(|| self.current_conn());
            self.open_sql(c, t, Some(name));
            if let Some(Tab { kind: TabKind::Sql(s), .. }) = self.tabs.last_mut() {
                s.file = Some(p.to_path_buf());
                s.new_script = false;
            }
        } else {
            self.toast("Não foi possível ler o script");
        }
    }

    pub fn rename_script(&mut self, path: &std::path::Path, new_stem: &str) -> Result<(), String> {
        let stem = new_stem.trim().trim_end_matches(".sql");
        if stem.is_empty() || stem.contains('/') || stem.contains('\\') || stem.starts_with('.') {
            return Err("Nome inválido".into());
        }
        let new_path = path.with_file_name(format!("{}.sql", stem));
        if new_path != path && new_path.exists() {
            return Err("Já existe um script com esse nome".into());
        }
        std::fs::rename(path, &new_path).map_err(|e| e.to_string())?;
        let old = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let new = new_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if let Some(c) = self.sh.store.script_conns.remove(&old) {
            self.sh.store.script_conns.insert(new.clone(), c);
            self.sh.save();
        }
        for t in self.tabs.iter_mut() {
            if let TabKind::Sql(s) = &mut t.kind {
                if s.file.as_deref() == Some(path) {
                    s.file = Some(new_path.clone());
                    if s.title == old {
                        s.title = new.clone();
                    }
                }
            }
        }
        self.git_cache = None;
        Ok(())
    }

    pub fn delete_script(&mut self, path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        self.sh.store.script_conns.remove(&name);
        for t in self.tabs.iter_mut() {
            if let TabKind::Sql(s) = &mut t.kind {
                if s.file.as_deref() == Some(path) {
                    s.file = None;
                }
            }
        }
        self.git_cache = None;
        self.sh.save();
    }

    /// `git status` of the scripts folder, refreshed at most every few seconds.
    fn git_changes(&mut self) -> Vec<(String, String)> {
        if let Some((at, v)) = &self.git_cache {
            if at.elapsed().as_secs() < 4 {
                return v.clone();
            }
        }
        let v = if crate::git::available() && crate::git::is_repo(&self.scripts_dir) { crate::git::changed_files(&self.scripts_dir) } else { vec![] };
        self.git_cache = Some((std::time::Instant::now(), v.clone()));
        v
    }

    pub fn open_git_dialog(&mut self) {
        let dir = self.scripts_dir.clone();
        let _ = std::fs::create_dir_all(&dir);
        let avail = crate::git::available();
        let repo = avail && crate::git::is_repo(&dir);
        let (branch, changed) = if repo { (crate::git::branch(&dir).unwrap_or_default(), crate::git::changed_files(&dir).len()) } else { (String::new(), 0) };
        let msg = format!("Atualiza scripts ({})", chrono::Local::now().format("%Y-%m-%d %H:%M"));
        self.dialog = Some(Dialog::Git { msg, output: String::new(), avail, repo, branch, changed, dir });
    }

    pub fn open_git_dialog_for(&mut self, dir: std::path::PathBuf) {
        let avail = crate::git::available();
        let repo = avail && crate::git::is_repo(&dir);
        let (branch, changed) = if repo { (crate::git::branch(&dir).unwrap_or_default(), crate::git::changed_files(&dir).len()) } else { (String::new(), 0) };
        let msg = format!("Atualiza o projeto ({})", chrono::Local::now().format("%Y-%m-%d %H:%M"));
        self.dialog = Some(Dialog::Git { msg, output: String::new(), avail, repo, branch, changed, dir });
    }

    /// True when the tab depends on the project: a script file inside it or a connection that belongs to it.
    pub fn tab_in_project(&self, t: &Tab, project: &str) -> bool {
        let dir = self.sh.store.projects.iter().find(|p| p.id == project).map(|p| p.dir());
        let conn_in = |c: &str| self.sh.cfg(c).map(|x| x.project == project).unwrap_or(false);
        match &t.kind {
            TabKind::Sql(s) => s.file.as_ref().zip(dir.as_ref()).map(|(f, d)| f.starts_with(d)).unwrap_or(false) || s.conn.as_deref().map(conn_in).unwrap_or(false),
            TabKind::Table(tt) => conn_in(&tt.conn),
            TabKind::Er(e) => conn_in(&e.conn),
            TabKind::Welcome => false,
        }
    }

    /// Closes (without saving) the tabs that depend on a project that is about to be deleted.
    pub fn close_tabs_of_project(&mut self, project: &str) {
        let keep: Vec<bool> = self.tabs.iter().map(|t| !self.tab_in_project(t, project)).collect();
        let mut it = keep.into_iter();
        self.tabs.retain(|_| it.next().unwrap_or(true));
        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
    }

    pub fn current_conn(&self) -> Option<String> {
        if let Some(t) = self.tabs.get(self.active) {
            match &t.kind {
                TabKind::Sql(s) => {
                    if s.conn.is_some() {
                        return s.conn.clone();
                    }
                }
                TabKind::Table(t) => return Some(t.conn.clone()),
                TabKind::Er(e) => return Some(e.conn.clone()),
                _ => {}
            }
        }
        match &self.nav_sel {
            Some(NavSel::Conn(c)) | Some(NavSel::Schema(c, _)) | Some(NavSel::Table(c, _, _, _)) => Some(c.clone()),
            None => self
                .sh
                .store
                .last_exec_conn
                .clone()
                .filter(|id| self.sh.cfg(id).is_some())
                .or_else(|| self.sh.store.connections.first().map(|c| c.id.clone())),
        }
    }

    fn close_tab(&mut self, i: usize) {
        if let TabKind::Sql(s) = &mut self.tabs[i].kind {
            if s.dirty && !s.text.trim().is_empty() {
                s.save_file();
            } else if s.new_script && s.text.trim().is_empty() {
                // SCRIPT_DELETE_EMPTY = DELETE_NEW: a new script that stayed empty leaves no file behind
                if let Some(p) = &s.file {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        self.tabs.remove(i);
        if self.active >= self.tabs.len() && !self.tabs.is_empty() {
            self.active = self.tabs.len() - 1;
        }
    }

    fn sql_cmd(&mut self, cmd: SqlCmd) {
        if let Some(Tab { kind: TabKind::Sql(s), .. }) = self.tabs.get_mut(self.active) {
            s.cmd = Some(cmd);
        } else if matches!(cmd, SqlCmd::Execute | SqlCmd::Script) {
            self.sh.status = "Abra um editor SQL (Ctrl+]) para executar consultas".into();
        }
    }

    fn open_table(&mut self, conn: String, schema: String, table: String, view: bool) {
        if let Some(i) = self.tabs.iter().position(|t| matches!(&t.kind, TabKind::Table(x) if x.conn == conn && x.schema == schema && x.table == table)) {
            self.active = i;
            return;
        }
        let t = TableTab::new(&mut self.sh, &conn, &schema, &table, view);
        let id = self.sh.fresh_id();
        self.tabs.push(Tab { id, kind: TabKind::Table(Box::new(t)) });
        self.active = self.tabs.len() - 1;
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), std::time::Instant::now()));
    }

    fn process_requests(&mut self) {
        if let Some(id) = self.sh.need_password.take() {
            if self.dialog.is_none() {
                self.dialog = Some(Dialog::Password { conn: id, pass: String::new(), remember: true });
            }
        }
        let reqs = std::mem::take(&mut self.sh.requests);
        for r in reqs {
            match r {
                Req::OpenSql { conn, text, title } => self.open_sql(conn, text, title),
                Req::OpenTable { conn, schema, table, view } => self.open_table(conn, schema, table, view),
                Req::OpenEr { conn, schema, focus } => {
                    let er = ErTab::build(&mut self.sh, &conn, &schema, focus.as_deref());
                    let id = self.sh.fresh_id();
                    self.tabs.push(Tab { id, kind: TabKind::Er(Box::new(er)) });
                    self.active = self.tabs.len() - 1;
                }
                Req::OpenSource { conn, schema, key, name, detail } => {
                    let text = match self.sh.session(&conn) {
                        Some(s) => s.lock().unwrap_or_else(|e| e.into_inner()).object_source(&key, &schema, &name, &detail).unwrap_or_else(|e| format!("-- não foi possível ler a definição: {}", e)),
                        None => "-- conexão fechada".to_string(),
                    };
                    let kind = self.sh.kind(&conn);
                    let text = editable_source(kind, &key, &schema, &name, &detail, &text);
                    self.open_sql(Some(conn), text, Some(format!("{} (código)", name)));
                }
                other => self.dialog_request(other),
            }
        }
    }

    // ------------------------------------------------------------ UI parts
    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Arquivo", |ui| {
                if ui.button("Nova conexão…").clicked() {
                    self.sh.req(Req::NewConn);
                    ui.close();
                }
                if ui.add(egui::Button::new("Novo script SQL").shortcut_text("Ctrl+]")).clicked() {
                    let c = self.current_conn();
                    self.open_sql(c, String::new(), None);
                    ui.close();
                }
                if ui.button("Importar dados CSV…").clicked() {
                    if let Some(NavSel::Table(c, s, t, _)) = self.nav_sel.clone() {
                        self.sh.req(Req::Import { conn: c, schema: s, table: t });
                    } else {
                        self.toast("Selecione uma tabela no navegador para importar");
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Versionar scripts (git)…").clicked() {
                    self.open_git_dialog();
                    ui.close();
                }
                if ui.add(egui::Button::new("Salvar script").shortcut_text("Ctrl+S")).clicked() {
                    self.sql_cmd(SqlCmd::Save);
                    ui.close();
                }
                ui.separator();
                if ui.button("Sair").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Editar", |ui| {
                for (label, sc, cmd) in [
                    ("Formatar SQL", "Ctrl+Shift+F", SqlCmd::Format),
                    ("Comentar/descomentar linha", "Ctrl+/", SqlCmd::Comment),
                    ("Seleção em MAIÚSCULAS", "Ctrl+Shift+X", SqlCmd::Upper),
                    ("Seleção em minúsculas", "Ctrl+Shift+Y", SqlCmd::Lower),
                    ("Autocompletar", "Ctrl+Espaço", SqlCmd::Complete),
                ] {
                    if ui.add(egui::Button::new(label).shortcut_text(sc)).clicked() {
                        self.sql_cmd(cmd);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Editor SQL", |ui| {
                for (label, sc, cmd) in [
                    ("Executar instrução", "Ctrl+Enter", SqlCmd::Execute),
                    ("Executar em nova aba", "Ctrl+\\", SqlCmd::ExecuteNewTab),
                    ("Executar script", "Alt+X", SqlCmd::Script),
                    ("Plano de execução", "Ctrl+Shift+E", SqlCmd::Explain),
                ] {
                    if ui.add(egui::Button::new(label).shortcut_text(sc)).clicked() {
                        self.sql_cmd(cmd);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Histórico de consultas…").clicked() {
                    self.dialog = Some(Dialog::History { filter: String::new() });
                    ui.close();
                }
            });
            ui.menu_button("Banco de dados", |ui| {
                let cur = self.current_conn();
                if ui.button("Nova conexão…").clicked() {
                    self.sh.req(Req::NewConn);
                    ui.close();
                }
                if ui.button("Criar banco de amostra (SQLite)").clicked() {
                    self.create_sample();
                    ui.close();
                }
                ui.separator();
                if ui.add_enabled(cur.is_some(), egui::Button::new("Conectar")).clicked() {
                    if let Some(c) = &cur {
                        if let Err(e) = self.sh.connect(c) {
                            self.toast(e);
                        }
                    }
                    ui.close();
                }
                if ui.add_enabled(cur.is_some(), egui::Button::new("Reconectar")).clicked() {
                    if let Some(c) = &cur {
                        self.request_disconnect(c, true);
                    }
                    ui.close();
                }
                if ui.add_enabled(cur.is_some(), egui::Button::new("Desconectar")).clicked() {
                    if let Some(c) = &cur {
                        self.request_disconnect(c, false);
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Confirmar (commit)").clicked() {
                    self.tx_action(true);
                    ui.close();
                }
                if ui.button("Reverter (rollback)").clicked() {
                    self.tx_action(false);
                    ui.close();
                }
                if ui.button("Alternar modo de transação").clicked() {
                    self.cycle_tx_mode();
                    ui.close();
                }
            });
            ui.menu_button("Janela", |ui| {
                if ui.button("Preferências…").clicked() {
                    self.dialog = Some(Dialog::Prefs);
                    ui.close();
                }
                let mut dark = self.sh.store.prefs.dark;
                if ui.checkbox(&mut dark, "Tema escuro").changed() {
                    self.sh.store.prefs.dark = dark;
                    self.sh.save();
                }
            });
            ui.menu_button("Ajuda", |ui| {
                if ui.button("Atalhos de teclado").clicked() {
                    self.dialog = Some(Dialog::Shortcuts);
                    ui.close();
                }
                if ui.button("Sobre").clicked() {
                    self.dialog = Some(Dialog::About);
                    ui.close();
                }
            });
        });
    }

    fn create_sample(&mut self) {
        let path = store::config_dir().join("sample.sqlite");
        match crate::sample::create(&path) {
            Ok(_) => {
                let mut cfg = ConnConfig::new(Kind::Sqlite);
                cfg.name = "Banco de amostra".into();
                cfg.database = path.to_string_lossy().to_string();
                cfg.set_mode(TxMode::from_auto_commit(self.sh.store.prefs.default_auto_commit));
                let id = cfg.id.clone();
                self.sh.store.connections.push(cfg);
                self.sh.save();
                let _ = self.sh.connect(&id);
                self.nav_sel = Some(NavSel::Conn(id));
                self.toast("Banco de amostra criado");
            }
            Err(e) => self.toast(format!("Erro: {}", e)),
        }
    }

    /// Disconnect or reconnect; with uncommitted changes it first asks what to do with them.
    pub fn request_disconnect(&mut self, id: &str, reconnect: bool) {
        if let Some(rows) = self.sh.has_pending(id) {
            self.dialog = Some(Dialog::ConfirmDisconnect { conn: id.to_string(), reconnect, rows });
        } else {
            self.finish_disconnect(id, reconnect, None);
        }
    }

    /// `pending`: Some(true) commits first, Some(false) rolls back first.
    pub fn finish_disconnect(&mut self, id: &str, reconnect: bool, pending: Option<bool>) {
        if let (Some(commit), Some(s)) = (pending, self.sh.session(id)) {
            let mut g = s.lock().unwrap_or_else(|e| e.into_inner());
            let _ = if commit { g.commit() } else { g.rollback() };
        }
        if reconnect {
            match self.sh.reconnect(id) {
                Ok(_) => self.toast(format!("Reconectado a {}", self.sh.conn_name(id))),
                Err(e) => self.toast(format!("Falha ao reconectar: {}", e)),
            }
        } else {
            self.sh.disconnect(id);
        }
    }

    fn tx_action(&mut self, commit: bool) {
        if let Some(c) = self.current_conn() {
            if let Some(s) = self.sh.session(&c) {
                let r = {
                    let mut g = s.lock().unwrap_or_else(|e| e.into_inner());
                    if commit {
                        g.commit()
                    } else {
                        g.rollback()
                    }
                };
                match r {
                    Ok(_) => self.toast(if commit { "Transação confirmada" } else { "Transação revertida" }),
                    Err(e) => self.toast(format!("Erro: {}", e)),
                }
            }
        }
    }

    /// Switch the transaction mode of the current connection: the open session and the saved connection both change.
    fn set_tx_mode(&mut self, mode: TxMode) {
        if let Some(c) = self.current_conn() {
            if let Some(s) = self.sh.session(&c) {
                let res = s.lock().unwrap_or_else(|e| e.into_inner()).set_mode(mode);
                match res {
                    Ok(_) => {
                        if let Some(cfg) = self.sh.store.connections.iter_mut().find(|x| x.id == c) {
                            cfg.set_mode(mode);
                        }
                        self.sh.save();
                        self.toast(format!("Modo de transação: {}", mode.label()));
                    }
                    Err(e) => self.toast(format!("Erro: {}", e)),
                }
            }
        }
    }

    fn cycle_tx_mode(&mut self) {
        if let Some(c) = self.current_conn() {
            if let Some((m, _, _, _)) = self.sh.tx_info(&c) {
                let next = match m {
                    TxMode::Auto => TxMode::Manual,
                    TxMode::Manual => TxMode::Smart,
                    TxMode::Smart => TxMode::Auto,
                };
                self.set_tx_mode(next);
            }
        }
    }

    fn toolbar_left(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        ui.horizontal_wrapped(|ui| {
            if icons::button(ui, Icon::Plug, "Nova conexão").clicked() {
                self.sh.req(Req::NewConn);
            }
            {
                let cur = self.current_conn();
                let connected = cur.as_ref().map(|c| self.sh.is_connected(c)).unwrap_or(false);
                if ui.add_enabled(connected, |ui: &mut Ui| icons::button(ui, Icon::Cross, "Desconectar")).on_hover_text("Fecha a sessão da conexão atual").clicked() {
                    if let Some(c) = &cur {
                        self.request_disconnect(c, false);
                    }
                }
                if ui.add_enabled(cur.is_some(), |ui: &mut Ui| icons::button(ui, Icon::Refresh, "Reconectar")).on_hover_text("Descarta a sessão e as caches e abre a conexão de novo").clicked() {
                    if let Some(c) = &cur {
                        self.request_disconnect(c, true);
                    }
                }
            }
            ui.separator();
            let cur = self.current_conn();
            let label = cur.as_ref().map(|c| self.sh.conn_name(c)).unwrap_or_else(|| "Sem conexão".into());
            let mut chosen: Option<String> = None;
            egui::ComboBox::from_id_salt("active_conn").selected_text(label).width(170.0).show_ui(ui, |ui| {
                for c in &self.sh.store.connections {
                    ui.horizontal(|ui| {
                        let (rc, _) = ui.allocate_exact_size(egui::vec2(28.0, 18.0), egui::Sense::hover());
                        icons::conn_badge(ui.painter(), rc, &c.badge_style());
                        if ui.selectable_label(cur.as_deref() == Some(c.id.as_str()), c.display_name()).clicked() {
                            chosen = Some(c.id.clone());
                        }
                    });
                }
            });
            if let Some(id) = chosen {
                let name = self.sh.conn_name(&id);
                if let Some(Tab { kind: TabKind::Sql(s), .. }) = self.tabs.get_mut(self.active) {
                    s.conn = Some(id.clone());
                    s.title = format!("<{}> {}", name, s.title.split("> ").last().unwrap_or("Script"));
                }
                self.nav_sel = Some(NavSel::Conn(id));
            }
            if let Some(c) = &cur {
                if let Some((mode, open, n, rows)) = self.sh.tx_info(c) {
                    ui.separator();
                    let no_tx = self.sh.cfg(c).map(|x| x.kind == Kind::Mongo).unwrap_or(false);
                    ui.add_enabled_ui(!no_tx, |ui| {
                        for m in TxMode::ALL {
                            if icons::segment(ui, Icon::Lock, m.label(), mode == m).on_hover_text("Modo de transação: Auto confirma tudo; Manual abre transação em qualquer instrução; Smart só em instruções que alteram dados").clicked() && mode != m {
                                self.set_tx_mode(m);
                            }
                        }
                    });
                    if mode != TxMode::Auto {
                        ui.add_enabled_ui(open, |ui| {
                            if icons::button(ui, Icon::Check, &commit_label(n, rows)).on_hover_text(format!("{} registro(s) afetado(s) em {} instrução(ões) pendente(s)", rows, n)).clicked() {
                                self.tx_action(true);
                            }
                            if icons::button(ui, Icon::Undo, "Rollback").clicked() {
                                self.tx_action(false);
                            }
                        });
                    }
                }
            }
        });
    }

    /// Second toolbar row: scripts and execution.
    fn toolbar_scripts(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if icons::button(ui, Icon::Script, "Novo script").on_hover_text("Ctrl+]").clicked() {
                let c = self.current_conn();
                self.open_sql(c, String::new(), None);
            }
            ui.separator();
            let is_sql = matches!(self.tabs.get(self.active), Some(Tab { kind: TabKind::Sql(_), .. }));
            ui.add_enabled_ui(is_sql, |ui| {
                if icons::primary(ui, Icon::Play, "Executar").on_hover_text("Executa a instrução sob o cursor — Ctrl+Enter").clicked() {
                    self.sql_cmd(SqlCmd::Execute);
                }
                if icons::button(ui, Icon::PlayAll, "Script").on_hover_text("Executa tudo — Alt+X").clicked() {
                    self.sql_cmd(SqlCmd::Script);
                }
                if icons::button(ui, Icon::Plan, "Plano").on_hover_text("Plano de execução — Ctrl+Shift+E").clicked() {
                    self.sql_cmd(SqlCmd::Explain);
                }
                if icons::button(ui, Icon::Format, "Formatar").on_hover_text("Ctrl+Shift+F").clicked() {
                    self.sql_cmd(SqlCmd::Format);
                }
            });
        });
    }

    /// Preferences and theme: always together, at the right edge when the bar fits on one line.
    fn toolbar_right(&mut self, ui: &mut Ui) {
        // right-to-left: the first widget added ends up at the far right
        let dark = self.sh.store.prefs.dark;
        // the icon shows the theme you will switch to: a sun on the dark theme, a moon on the light one
        if icons::button(ui, if dark { Icon::Sun } else { Icon::Moon }, if dark { "Claro" } else { "Escuro" }).on_hover_text("Alternar tema claro/escuro").clicked() {
            self.sh.store.prefs.dark = !dark;
            self.sh.save();
        }
        if icons::button(ui, Icon::Settings, "").on_hover_text("Preferências").clicked() {
            self.dialog = Some(Dialog::Prefs);
        }
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        let full = ui.available_rect_before_wrap();
        let rw = 128.0;
        if full.width() > 900.0 {
            let rr = egui::Rect::from_min_size(egui::pos2(full.right() - rw, full.top()), egui::vec2(rw, 30.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(rr).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| self.toolbar_right(ui));
            let lr = egui::Rect::from_min_max(full.min, egui::pos2(full.right() - rw - 2.0, full.bottom()));
            ui.scope_builder(egui::UiBuilder::new().max_rect(lr), |ui| self.toolbar_left(ui));
        } else {
            self.toolbar_left(ui);
            ui.horizontal(|ui| {
                self.toolbar_right(ui);
            });
        }
        self.toolbar_scripts(ui);
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.sh.status).small().color(pl.weak));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(c) = self.current_conn() {
                    if let Some(v) = self.sh.versions.get(&c) {
                        ui.label(RichText::new(v).small().color(pl.weak));
                        ui.separator();
                    }
                    if let Some((mode, _, n, rows)) = self.sh.tx_info(&c) {
                        ui.label(RichText::new(if mode == TxMode::Auto { "Auto-commit".to_string() } else { format!("{} • {} registro(s) pendente(s) ({} instr.)", mode.label(), rows, n) }).small().color(pl.weak));
                        ui.separator();
                    }
                }
                if let Some(Tab { kind: TabKind::Sql(s), .. }) = self.tabs.get(self.active) {
                    let before: String = s.text.chars().take(s.cursor.0).collect();
                    let ln = before.matches('\n').count() + 1;
                    let col = before.rsplit('\n').next().map(|x| x.chars().count()).unwrap_or(0) + 1;
                    ui.label(RichText::new(format!("Ln {}, Col {}", ln, col)).small().color(pl.weak));
                    ui.separator();
                }
                if let Some((t, at)) = &self.toast {
                    if at.elapsed().as_secs() < 5 {
                        ui.label(RichText::new(t).small().color(pl.warn));
                    }
                }
            });
        });
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        let mut close: Option<usize> = None;
        egui::ScrollArea::horizontal().id_salt("tab_scroll").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for i in 0..self.tabs.len() {
                    let (title, icon, dirty) = match &self.tabs[i].kind {
                        TabKind::Welcome => ("Bem-vindo".to_string(), Icon::Home, false),
                        TabKind::Sql(s) => (s.title.clone(), Icon::Script, s.dirty),
                        TabKind::Table(t) => (t.table.clone(), if t.view { Icon::View } else { Icon::Table }, t.grid.as_ref().map(|g| g.pending_changes() > 0).unwrap_or(false)),
                        TabKind::Er(e) => (format!("Diagrama {}", e.title(&self.sh)), Icon::Diagram, false),
                    };
                    let (r, x) = icons::tab_item(ui, icon, &title, self.active == i, dirty);
                    if x || r.middle_clicked() {
                        close = Some(i);
                    } else if r.clicked() {
                        self.active = i;
                    }
                    r.context_menu(|ui| {
                        if ui.button("Fechar").clicked() {
                            close = Some(i);
                            ui.close();
                        }
                        if ui.button("Fechar as outras").clicked() {
                            let keep = self.tabs.remove(i);
                            self.tabs.clear();
                            self.tabs.push(keep);
                            self.active = 0;
                            ui.close();
                        }
                    });
                }
            });
        });
        if let Some(i) = close {
            if i < self.tabs.len() {
                self.close_tab(i);
            }
        }
    }

    fn welcome(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        egui::ScrollArea::vertical().id_salt("welcome_scroll").auto_shrink([false, false]).show(ui, |ui| {
            // one centered column that never gets wider than the page; everything inside wraps
            let w = ui.available_width();
            let cw = (w - 24.0).clamp(240.0, 760.0);
            let full = ui.available_rect_before_wrap();
            let x0 = full.left() + ((w - cw) / 2.0).max(0.0);
            let col = egui::Rect::from_min_max(egui::pos2(x0, full.top()), egui::pos2(x0 + cw, full.top() + 4000.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(col), |ui| {
                {
                    ui.set_width(cw);
                                        ui.add_space(30.0);
                    ui.label(RichText::new("Sqlion").size(34.0).strong());
                    ui.add(egui::Label::new(RichText::new("Gerenciador universal de bancos de dados — leve, sem Java e sem Eclipse").color(pl.weak)).wrap());
                    ui.add_space(18.0);
                    ui.horizontal_wrapped(|ui| {
                        if icons::primary(ui, Icon::Plug, "Nova conexão").clicked() {
                            self.sh.req(Req::NewConn);
                        }
                        if icons::button(ui, Icon::Database, "Criar banco de amostra").clicked() {
                            self.create_sample();
                        }
                        if icons::button(ui, Icon::Script, "Abrir editor SQL").clicked() {
                            let c = self.current_conn();
                            self.open_sql(c, String::new(), None);
                        }
                    });
                    ui.add_space(22.0);
                    icons::section(ui, "Bancos suportados");
                    ui.add_space(6.0);
                    // flat items (no nested `horizontal`) so the row wraps between databases
                    ui.horizontal_wrapped(|ui| {
                        for p in PRESETS {
                            let (t, (r, g, b)) = p.kind.badge();
                            let sz = icons::icon_size(ui);
                            let (rc, _) = ui.allocate_exact_size(egui::vec2(sz + 8.0, sz), egui::Sense::hover());
                            icons::badge(ui.painter(), rc, t, Color32::from_rgb(r, g, b));
                            ui.label(p.name);
                            ui.add_space(8.0);
                        }
                    });
                    ui.add_space(20.0);
                    ui.add(egui::Label::new(RichText::new("Ctrl+Enter executa a instrução • Alt+X executa o script • Ctrl+Espaço autocompleta • duplo clique em uma tabela abre os dados para editar").small().color(pl.weak)).wrap_mode(egui::TextWrapMode::Wrap));
                    ui.add_space(20.0);
                }
            });
        });
    }

    // ------------------------------------------------------------ navigator
    fn navigator(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        ui.horizontal(|ui| {
            icons::section(ui, "Navegador de bancos");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Refresh, "").on_hover_text("Atualizar (F5)").clicked() {
                    self.refresh_nav();
                }
                if icons::button(ui, Icon::Plus, "").on_hover_text("Nova conexão").clicked() {
                    self.sh.req(Req::NewConn);
                }
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.nav_filter).hint_text("Filtrar tabelas…").desired_width(f32::INFINITY));
        ui.add_space(4.0);
        let conns: Vec<ConnConfig> = self.sh.store.connections.iter().filter(|c| c.project.is_empty()).cloned().collect();
        let projects = self.sh.store.projects.clone();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            if conns.is_empty() && projects.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("Nenhuma conexão ainda.").color(pl.weak));
                if icons::primary(ui, Icon::Plug, "Criar conexão").clicked() {
                    self.sh.req(Req::NewConn);
                }
            }
            let mut folders: Vec<String> = conns.iter().map(|c| c.folder.clone()).filter(|f| !f.is_empty()).collect();
            folders.sort();
            folders.dedup();
            for c in conns.iter().filter(|c| c.folder.is_empty()) {
                self.conn_node(ui, c);
            }
            for f in folders {
                egui::CollapsingHeader::new(RichText::new(&f).strong()).id_salt(("folder", &f)).default_open(true).show(ui, |ui| {
                    for c in conns.iter().filter(|c| c.folder == f) {
                        self.conn_node(ui, c);
                    }
                });
            }
            for p in projects {
                self.project_node(ui, &p);
            }
            ui.add_space(8.0);
            if self.sh.persist {
                self.scripts_node(ui);
            }
        });
    }

    fn project_node(&mut self, ui: &mut Ui, p: &store::Project) {
        let pl = pal(self.sh.store.prefs.dark);
        let conns: Vec<ConnConfig> = self.sh.store.connections.iter().filter(|c| c.project == p.id).cloned().collect();
        let st = CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new(("project", &p.id)), true);
        let hdr = st.show_header(ui, |ui| {
            let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.accent), None, RichText::new(&p.name).strong(), &conns.len().to_string(), false, None);
            let r = r.on_hover_text(format!("Projeto: {}", p.path));
            r.context_menu(|ui| self.project_menu(ui, p));
        });
        hdr.body(|ui| {
            if conns.is_empty() {
                ui.label(RichText::new("(nenhuma conexão)").small().color(pl.weak));
            }
            for c in &conns {
                self.conn_node(ui, c);
            }
        });
    }

    fn project_menu(&mut self, ui: &mut Ui, p: &store::Project) {
        if ui.button("Renomear…").clicked() {
            self.dialog = Some(Dialog::RenameProject { id: p.id.clone(), name: p.name.clone(), msg: None });
            ui.close();
        }
        ui.separator();
        if ui.button("Novo script").clicked() {
            let dir = p.scripts_dir();
            let _ = std::fs::create_dir_all(&dir);
            let mut n = 1;
            let path = loop {
                let f = dir.join(format!("novo-{}.sql", n));
                if !f.exists() {
                    break f;
                }
                n += 1;
            };
            if std::fs::write(&path, "").is_ok() {
                self.open_script_file(&path);
            }
            ui.close();
        }
        if ui.button("Nova conexão neste projeto…").clicked() {
            self.sh.req(Req::NewConnIn(p.id.clone()));
            ui.close();
        }
        if ui.button("Versionamento (git)…").clicked() {
            self.open_git_dialog_for(p.dir());
            ui.close();
        }
        ui.separator();
        if ui.button("Abrir a pasta").clicked() {
            crate::shared::open_path(&p.dir());
            ui.close();
        }
        if ui.button("Copiar o caminho").clicked() {
            ui.ctx().copy_text(p.path.clone());
            ui.close();
        }
        ui.separator();
        if ui.button("Remover da lista").on_hover_text("Fecha o projeto; a pasta continua no disco").clicked() {
            for i in 0..self.tabs.len() {
                if self.tab_in_project(&self.tabs[i], &p.id) {
                    if let TabKind::Sql(sq) = &mut self.tabs[i].kind {
                        if sq.dirty && !sq.text.trim().is_empty() {
                            sq.save_file();
                        }
                    }
                }
            }
            self.close_tabs_of_project(&p.id);
            self.sh.store.close_project(&p.id);
            self.sh.save();
            ui.close();
        }
        if ui.button("Excluir a pasta…").clicked() {
            self.dialog = Some(Dialog::DeleteProject { id: p.id.clone(), msg: None });
            ui.close();
        }
    }

    fn scripts_node(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        let dir = self.scripts_dir.clone();
        let mut scripts: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map(|e| e == "sql").unwrap_or(false)).collect())
            .unwrap_or_default();
        scripts.sort();
        let st = CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new("scripts_node"), false);
        let open = st.is_open();
        let changes = if open { self.git_changes() } else { vec![] };
        let hdr = st.show_header(ui, |ui| {
            let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.warn), None, RichText::new("Scripts").strong(), &scripts.len().to_string(), false, None);
            let r = r.on_hover_text(format!("Pasta: {}", dir.display()));
            r.context_menu(|ui| {
                if ui.button("Novo script").clicked() {
                    let c = self.current_conn();
                    self.open_sql(c, String::new(), None);
                    ui.close();
                }
                if ui.button("Versionamento (git)…").clicked() {
                    self.open_git_dialog();
                    ui.close();
                }
                if ui.button("Mudar a pasta dos scripts…").clicked() {
                    self.dialog = Some(Dialog::Prefs);
                    ui.close();
                }
                if ui.button("Abrir a pasta").clicked() {
                    let _ = crate::shared::open_path(&dir);
                    ui.close();
                }
                if ui.button("Copiar o caminho").clicked() {
                    ui.ctx().copy_text(dir.display().to_string());
                    ui.close();
                }
            });
        });
        hdr.body(|ui| {
            ui.label(RichText::new(dir.display().to_string()).small().color(pl.weak));
            if scripts.is_empty() {
                ui.label(RichText::new("(nenhum script salvo)").small().color(pl.weak));
            }
            for p in scripts {
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let change = changes.iter().find(|(_, f)| f.rsplit('/').next() == Some(name.as_str())).map(|(c, _)| c.clone());
                let dot = change.as_ref().map(|_| pl.warn);
                let suffix = match &change {
                    Some(c) if c.contains('?') => "novo",
                    Some(_) => "alterado",
                    None => "",
                };
                let conn_hint = self.sh.script_conn(&name).map(|c| self.sh.conn_name(&c)).unwrap_or_default();
                let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Script, pl.weak), dot, RichText::new(&name), suffix, false, None);
                let r = if conn_hint.is_empty() { r } else { r.on_hover_text(format!("Última conexão: {}", conn_hint)) };
                if r.double_clicked() {
                    self.open_script_file(&p);
                }
                r.context_menu(|ui| {
                    if ui.button("Abrir").clicked() {
                        self.open_script_file(&p);
                        ui.close();
                    }
                    if ui.button("Renomear…").clicked() {
                        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                        self.dialog = Some(Dialog::RenameScript { path: p.clone(), name: stem, msg: None });
                        ui.close();
                    }
                    if ui.button("Excluir…").clicked() {
                        self.dialog = Some(Dialog::DeleteScript(p.clone()));
                        ui.close();
                    }
                    if ui.button("Copiar o caminho").clicked() {
                        ui.ctx().copy_text(p.display().to_string());
                        ui.close();
                    }
                });
            }
        });
    }

    fn refresh_nav(&mut self) {
        if let Some(c) = self.current_conn() {
            self.sh.refresh(&c);
            self.toast("Navegador atualizado");
        }
    }

    fn conn_node(&mut self, ui: &mut Ui, c: &ConnConfig) {
        let pl = pal(self.sh.store.prefs.dark);
        let id = c.id.clone();
        let connected = self.sh.is_connected(&id);
        let cid = egui::Id::new(("conn", &id));
        let mut state = CollapsingState::load_with_default_open(ui.ctx(), cid, false);
        if state.is_open() && !connected && self.sh.connect(&id).is_err() {
            state.set_open(false);
            state.store(ui.ctx());
        }
        let sel = self.nav_sel == Some(NavSel::Conn(id.clone()));
        let style = c.badge_style();
        let tint = if c.conn_type != crate::constants::ConnType::Dev {
            let (cr, cg, cb) = c.conn_type.color();
            Some(Color32::from_rgb(cr, cg, cb))
        } else {
            None
        };
        let hdr = state.show_header(ui, |ui| {
            let r = icons::tree_row(
                ui,
                icons::Lead::Conn(&style),
                Some(if connected { pl.ok } else { pl.weak.gamma_multiply(0.5) }),
                RichText::new(c.display_name()).strong(),
                if c.flavor.is_empty() { "" } else { &c.flavor },
                sel,
                tint,
            );
            if r.clicked() {
                self.nav_sel = Some(NavSel::Conn(id.clone()));
            }
            if r.double_clicked() && !connected {
                if let Err(e) = self.sh.connect(&id) {
                    self.toast(e);
                }
            }
            r.context_menu(|ui| {
                if !connected {
                    if ui.button("Conectar").clicked() {
                        if let Err(e) = self.sh.connect(&id) {
                            self.toast(e);
                        }
                        ui.close();
                    }
                } else {
                    if ui.button("Desconectar").clicked() {
                        self.request_disconnect(&id, false);
                        ui.close();
                    }
                    if ui.button("Reconectar").clicked() {
                        self.request_disconnect(&id, true);
                        ui.close();
                    }
                    if ui.button("Atualizar").clicked() {
                        self.sh.refresh(&id);
                        ui.close();
                    }
                    if ui.button("Novo script SQL").clicked() {
                        self.open_sql(Some(id.clone()), String::new(), None);
                        ui.close();
                    }
                    if c.kind != Kind::Mongo && ui.button("Diagrama ER do esquema").clicked() {
                        let s = self.sh.default_schema(&id);
                        self.sh.req(Req::OpenEr { conn: id.clone(), schema: s, focus: None });
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Editar conexão…").clicked() {
                    self.sh.req(Req::EditConn(id.clone()));
                    ui.close();
                }
                if ui.button("Excluir conexão").clicked() {
                    self.sh.req(Req::DeleteConn(id.clone()));
                    ui.close();
                }
            });
        });
        hdr.body(|ui| {
            if !self.sh.is_connected(&id) {
                return;
            }
            let kind = c.kind;
            for fd in db_folders(kind) {
                self.object_folder(ui, &id, "", &fd);
            }
            if kind == Kind::Sqlite {
                self.schema_body(ui, &id, "main");
            } else {
                for s in self.sh.ensure_schemas(&id) {
                    let sid = egui::Id::new(("schema", &id, &s));
                    let selected = self.nav_sel == Some(NavSel::Schema(id.clone(), s.clone()));
                    let open_default = (kind == Kind::Postgres && s == "public") || (kind == Kind::SqlServer && s == "dbo") || ((kind == Kind::Mysql || kind == Kind::Mongo) && s == c.database);
                    let st = CollapsingState::load_with_default_open(ui.ctx(), sid, open_default);
                    st.show_header(ui, |ui| {
                        let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Database, pl.accent), None, RichText::new(&s), "", selected, None);
                        if r.clicked() {
                            self.nav_sel = Some(NavSel::Schema(id.clone(), s.clone()));
                        }
                        r.context_menu(|ui| {
                            if kind != Kind::Mongo && ui.button("Diagrama ER").clicked() {
                                self.sh.req(Req::OpenEr { conn: id.clone(), schema: s.clone(), focus: None });
                                ui.close();
                            }
                            if ui.button("Novo script").clicked() {
                                self.open_sql(Some(id.clone()), String::new(), None);
                                ui.close();
                            }
                        });
                    })
                    .body(|ui| self.schema_body(ui, &id, &s));
                }
            }
        });
    }

    fn schema_body(&mut self, ui: &mut Ui, id: &str, schema: &str) {
        let pl = pal(self.sh.store.prefs.dark);
        let tables = self.sh.ensure_tables(id, schema);
        let mongo = self.sh.kind(id) == Kind::Mongo;
        let f = self.nav_filter.to_lowercase();
        for (grp, want_view) in [(if mongo { "Coleções" } else { "Tabelas" }, false), ("Visões", true)] {
            let list: Vec<&(String, bool)> = tables.iter().filter(|(n, v)| *v == want_view && (f.is_empty() || n.to_lowercase().contains(&f))).collect();
            if want_view && list.is_empty() {
                continue;
            }
            let gid = egui::Id::new(("grp", id, schema, if want_view { "V" } else { "T" }));
            let mut st = CollapsingState::load_with_default_open(ui.ctx(), gid, false);
            if !f.is_empty() && !list.is_empty() {
                st.set_open(true);
            }
            st.show_header(ui, |ui| {
                icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.warn), None, RichText::new(grp), &format!("{}", list.len()), false, None);
            })
            .body(|ui| {
                for (name, view) in list {
                    self.table_node(ui, id, schema, name, *view);
                }
            });
        }
        let kind = self.sh.kind(id);
        for fd in schema_folders(kind) {
            self.object_folder(ui, id, schema, &fd);
        }
    }

    /// A folder of schema objects (procedures, functions, sequences, triggers ...) loaded when opened.
    fn object_folder(&mut self, ui: &mut Ui, id: &str, schema: &str, fd: &FolderDef) {
        let pl = pal(self.sh.store.prefs.dark);
        let gid = egui::Id::new(("objfolder", id, schema, fd.key));
        let st = CollapsingState::load_with_default_open(ui.ctx(), gid, false);
        let open = st.is_open();
        let f = self.nav_filter.to_lowercase();
        let count = self.sh.objects.get(&(id.to_string(), schema.to_string(), fd.key.to_string())).map(|v| v.len());
        let kind = self.sh.kind(id);
        st.show_header(ui, |ui| {
            icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.warn), None, RichText::new(fd.label), &count.map(|n| n.to_string()).unwrap_or_default(), false, None);
        })
        .body(|ui| {
            if !open {
                return;
            }
            let items = self.sh.ensure_objects(id, schema, fd.key);
            if items.is_empty() {
                ui.label(RichText::new("(vazio)").small().color(pl.weak));
            }
            for o in items.iter().filter(|o| f.is_empty() || o.name.to_lowercase().contains(&f)) {
                let with_args = matches!(fd.key, "function" | "procedure" | "aggregate");
                let shown = if with_args && !o.detail.is_empty() { format!("{}({})", o.name, o.detail) } else { o.name.clone() };
                let suffix = if with_args { "" } else { o.detail.as_str() };
                let r = icons::tree_row(ui, icons::Lead::Icon(object_icon(fd.key), pl.accent), None, RichText::new(shown), suffix, false, None);
                if r.double_clicked() {
                    self.sh.req(Req::OpenSource { conn: id.into(), schema: schema.into(), key: fd.key.into(), name: o.name.clone(), detail: o.detail.clone() });
                }
                r.context_menu(|ui| {
                    if ui.button("Ver código-fonte").clicked() {
                        self.sh.req(Req::OpenSource { conn: id.into(), schema: schema.into(), key: fd.key.into(), name: o.name.clone(), detail: o.detail.clone() });
                        ui.close();
                    }
                    if matches!(fd.key, "function" | "procedure") && ui.button("Gerar chamada (CALL/EXEC)").clicked() {
                        let text = call_template(kind, fd.key, schema, &o.name);
                        self.sh.req(Req::OpenSql { conn: Some(id.into()), text, title: None });
                        ui.close();
                    }
                    if ui.button("Copiar nome").clicked() {
                        ui.ctx().copy_text(o.name.clone());
                        ui.close();
                    }
                });
            }
        });
    }

    fn table_node(&mut self, ui: &mut Ui, id: &str, schema: &str, name: &str, view: bool) {
        let pl = pal(self.sh.store.prefs.dark);
        let tid = egui::Id::new(("tbl", id, schema, name));
        let st = CollapsingState::load_with_default_open(ui.ctx(), tid, false);
        let sel = self.nav_sel == Some(NavSel::Table(id.into(), schema.into(), name.into(), view));
        let open = st.is_open();
        let hdr = st.show_header(ui, |ui| {
            let r = icons::tree_row(ui, icons::Lead::Icon(if view { Icon::View } else { Icon::Table }, pl.accent), None, RichText::new(name), "", sel, None);
            if r.clicked() {
                self.nav_sel = Some(NavSel::Table(id.into(), schema.into(), name.into(), view));
            }
            if r.double_clicked() {
                self.sh.req(Req::OpenTable { conn: id.into(), schema: schema.into(), table: name.into(), view });
            }
            r.context_menu(|ui| self.table_menu(ui, id, schema, name, view));
        });
        hdr.body(|ui| {
            if !open {
                return;
            }
            let m = self.sh.ensure_tmeta(id, schema, name);
            for c in &m.columns {
                let fk = m.fks.iter().any(|f| f.columns.contains(&c.name));
                let (ic, col) = if c.pk {
                    (Icon::Key, pl.warn)
                } else if fk {
                    (Icon::Link, pl.accent)
                } else {
                    (Icon::Column, pl.weak)
                };
                icons::tree_row(ui, icons::Lead::Icon(ic, col), None, RichText::new(&c.name), &format!("{}{}", c.typ, if c.nullable { "" } else { " • obrigatória" }), false, None);
            }
            if !m.indexes.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Índices ({})", m.indexes.len())).color(pl.weak)).id_salt(("idx", id, schema, name)).show(ui, |ui| {
                    for i in &m.indexes {
                        icons::tree_row(ui, icons::Lead::Icon(if i.unique { Icon::Key } else { Icon::Column }, pl.weak), None, RichText::new(&i.name), &i.columns.join(", "), false, None);
                    }
                });
            }
            if !m.fks.is_empty() {
                egui::CollapsingHeader::new(RichText::new(format!("Chaves estrangeiras ({})", m.fks.len())).color(pl.weak)).id_salt(("fks", id, schema, name)).show(ui, |ui| {
                    for f in &m.fks {
                        icons::tree_row(ui, icons::Lead::Icon(Icon::Link, pl.accent), None, RichText::new(f.columns.join(", ")), &format!("→ {}", f.ref_table), false, None);
                    }
                });
            }
        });
    }

    fn table_menu(&mut self, ui: &mut Ui, id: &str, schema: &str, name: &str, view: bool) {
        if ui.button("Abrir tabela (dados)").clicked() {
            self.sh.req(Req::OpenTable { conn: id.into(), schema: schema.into(), table: name.into(), view });
            ui.close();
        }
        if ui.button("Novo script SQL (SELECT)").clicked() {
            let q = self.sh.qualified(id, schema, name);
            self.open_sql(Some(id.into()), format!("SELECT * FROM {};\n", q), None);
            ui.close();
        }
        if ui.button("Diagrama ER").clicked() {
            self.sh.req(Req::OpenEr { conn: id.into(), schema: schema.into(), focus: Some(name.into()) });
            ui.close();
        }
        ui.menu_button("Gerar SQL", |ui| {
            for (label, kind) in [("SELECT", 0u8), ("INSERT", 1), ("UPDATE", 2), ("DELETE", 3), ("DDL", 4)] {
                if ui.button(label).clicked() {
                    let text = self.generate_sql(id, schema, name, kind);
                    self.sh.req(Req::GenSql { title: format!("Gerar SQL — {}", label), text });
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.button("Exportar dados…").clicked() {
            let q = self.sh.qualified(id, schema, name);
            self.sh.req(Req::Export { conn: id.into(), sql: format!("SELECT * FROM {}", q), name: name.into(), rows: None });
            ui.close();
        }
        if !view && ui.button("Importar dados CSV…").clicked() {
            self.sh.req(Req::Import { conn: id.into(), schema: schema.into(), table: name.into() });
            ui.close();
        }
        ui.separator();
        if ui.button("Copiar nome").clicked() {
            ui.ctx().copy_text(name.to_string());
            ui.close();
        }
        if ui.button("Atualizar").clicked() {
            self.sh.refresh(id);
            ui.close();
        }
        if !view && ui.button("Excluir tabela (abre DROP no editor)").clicked() {
            let q = self.sh.qualified(id, schema, name);
            self.open_sql(Some(id.into()), format!("DROP TABLE {};\n", q), None);
            ui.close();
        }
    }

    fn generate_sql(&mut self, id: &str, schema: &str, name: &str, kind: u8) -> String {
        let m = self.sh.ensure_tmeta(id, schema, name);
        let k = self.sh.kind(id);
        let q = self.sh.qualified(id, schema, name);
        let cols: Vec<String> = m.columns.iter().map(|c| k.quote(&c.name)).collect();
        let pk: Vec<String> = m.columns.iter().filter(|c| c.pk).map(|c| format!("{} = ?", k.quote(&c.name))).collect();
        let wh = if pk.is_empty() { "<condição>".to_string() } else { pk.join(" AND ") };
        match kind {
            0 => format!("SELECT {}\nFROM {};", cols.join(",\n       "), q),
            1 => format!("INSERT INTO {} ({})\nVALUES ({});", q, cols.join(", "), vec!["?"; cols.len()].join(", ")),
            2 => format!("UPDATE {}\nSET {}\nWHERE {};", q, cols.iter().map(|c| format!("{} = ?", c)).collect::<Vec<_>>().join(",\n    "), wh),
            3 => format!("DELETE FROM {}\nWHERE {};", q, wh),
            _ => {
                if m.ddl.is_empty() {
                    build_ddl(k, schema, name, &m)
                } else {
                    m.ddl
                }
            }
        }
    }

    fn frame_logic(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.frames += 1;
        let want = (self.sh.store.prefs.dark, self.sh.store.prefs.font_size);
        if self.applied_theme != Some(want) {
            setup_style(ctx, want.0, want.1);
            self.applied_theme = Some(want);
        }
        // shortcuts (consumed before the editors see them)
        let cmds: [(Modifiers, Key, SqlCmd); 10] = [
            (Modifiers::COMMAND, Key::Enter, SqlCmd::Execute),
            (Modifiers::ALT, Key::X, SqlCmd::Script),
            (Modifiers::COMMAND, Key::Backslash, SqlCmd::ExecuteNewTab),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::E, SqlCmd::Explain),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::F, SqlCmd::Format),
            (Modifiers::COMMAND, Key::Slash, SqlCmd::Comment),
            (Modifiers::COMMAND, Key::Space, SqlCmd::Complete),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::X, SqlCmd::Upper),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::Y, SqlCmd::Lower),
            (Modifiers::COMMAND, Key::S, SqlCmd::Save),
        ];
        for (m, k, c) in cmds {
            if ctx.input_mut(|i| i.consume_key(m, k)) {
                self.sql_cmd(c);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::CloseBracket)) {
            let c = self.current_conn();
            self.open_sql(c, String::new(), None);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::W)) && !self.tabs.is_empty() {
            let a = self.active;
            self.close_tab(a);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            self.refresh_nav();
        }

        if self.sh.persist {
            let d = self.sh.store.scripts_dir();
            if d != self.last_scripts_dir {
                let _ = std::fs::create_dir_all(&d);
                self.last_scripts_dir = d.clone();
                self.git_cache = None;
            }
            self.scripts_dir = d;
        }
        // background tabs keep receiving their results
        for t in self.tabs.iter_mut() {
            match &mut t.kind {
                TabKind::Sql(s) => s.poll(&mut self.sh),
                TabKind::Table(t) => t.poll(&mut self.sh),
                _ => {}
            }
        }
        let pl = pal(self.sh.store.prefs.dark);
        let fr = |fill: egui::Color32| egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(10, 5));
        egui::Panel::top("menu").frame(fr(pl.panel)).show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar").frame(fr(pl.toolbar).stroke(egui::Stroke::new(1.0, pl.border))).show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").frame(fr(pl.toolbar).stroke(egui::Stroke::new(1.0, pl.border))).show(ui, |ui| self.status_bar(ui));
        let r = egui::Panel::left("navigator")
            .frame(egui::Frame::new().fill(pl.panel).inner_margin(egui::Margin::same(10)).stroke(egui::Stroke::new(1.0, pl.border)))
            .resizable(true)
            .default_size(self.nav_w)
            .size_range(200.0..=560.0)
            .show(ui, |ui| self.navigator(ui));
        self.nav_w = r.response.rect.width();

        egui::CentralPanel::default().frame(egui::Frame::new().fill(pl.bg)).show(ui, |ui| {
            if self.tabs.is_empty() {
                self.welcome(ui);
                return;
            }
            egui::Frame::new().fill(pl.toolbar).inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 0 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.tab_bar(ui);
            });
            let a = self.active.min(self.tabs.len() - 1);
            let dark = self.sh.store.prefs.dark;
            let id = self.tabs[a].id;
            egui::Frame::new().fill(pl.editor).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                match &mut self.tabs[a].kind {
                    TabKind::Welcome => {}
                    TabKind::Sql(s) => s.show(ui, &mut self.sh, id, dark),
                    TabKind::Table(t) => t.show(ui, &mut self.sh, id),
                    TabKind::Er(e) => e.show(ui, &mut self.sh, id),
                }
                if matches!(self.tabs[a].kind, TabKind::Welcome) {
                    self.welcome(ui);
                }
            });
        });
        self.process_requests();
        self.dialogs(ctx);
        self.process_requests();

        if self.demo && self.frames == 3 && std::env::var("DBR_DEMO_DIALOG").map(|v| v == "git").unwrap_or(false) {
            self.open_git_dialog();
        }
        if self.demo && self.frames == 40 && std::env::var("DBR_VP").is_ok() {
            for t in self.tabs.iter_mut() {
                if let TabKind::Sql(s) = &mut t.kind {
                    if let Some(g) = s.results.first_mut() {
                        g.sel = Some((0, 1));
                    }
                }
            }
            if std::env::var("DBR_TAB").map(|v| v == "sql").unwrap_or(false) {
                self.active = 0;
            }
        }
        if self.demo && self.frames == 40 {
            // show what editing looks like: one edited cell, one new row, one deleted row
            if let Some(Tab { kind: TabKind::Table(t), .. }) = self.tabs.get_mut(1) {
                if let Some(g) = t.grid.as_mut() {
                    if g.qr.rows.len() > 5 {
                        g.set_cell(1, 1, Some("Beatriz".into()));
                        g.set_cell(1, 4, Some("Portugal".into()));
                        g.set_cell(3, 2, Some("Oliveira".into()));
                        g.toggle_delete(&[5]);
                        let n = g.qr.columns.len();
                        let mut row = vec![None; n];
                        row[1] = Some("Camila".into());
                        row[2] = Some("Duarte".into());
                        row[4] = Some("Brasil".into());
                        g.add_row(row);
                        g.sel = Some((1, 4));
                        g.scroll_to = None;
                    }
                }
            }
        }
        // automatic screenshot: draw ~120 frames, save PNG, quit
        if let Some(path) = self.screenshot.clone() {
            let busy = self.tabs.iter().any(|t| match &t.kind {
                TabKind::Sql(s) => s.job.is_some(),
                TabKind::Table(t) => t.job.is_some(),
                _ => false,
            });
            ctx.request_repaint();
            if self.frames >= 120 && !self.shot_requested && !busy {
                self.shot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let img = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = img {
                let (w, h) = (img.width() as u32, img.height() as u32);
                let mut buf = Vec::with_capacity((w * h * 4) as usize);
                for p in img.pixels.iter() {
                    buf.extend_from_slice(&[p.r(), p.g(), p.b(), p.a()]);
                }
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(im) = image::RgbaImage::from_raw(w, h, buf) {
                    let _ = im.save(&path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame_logic(&ctx, ui);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        for t in &mut self.tabs {
            if let TabKind::Sql(s) = &mut t.kind {
                if s.dirty && !s.text.trim().is_empty() {
                    s.save_file();
                }
            }
        }
        self.sh.save();
    }
}

fn text_is_empty_marker(t: &str) -> bool {
    t.trim().is_empty()
}

fn object_icon(key: &str) -> Icon {
    match key {
        "function" | "aggregate" => Icon::Func,
        "procedure" => Icon::Proc,
        "trigger" | "dbtrigger" => Icon::Bolt,
        "sequence" => Icon::Seq,
        "event" => Icon::Clock,
        "synonym" => Icon::Link,
        "type" => Icon::Type,
        "extension" => Icon::Puzzle,
        "tablespace" => Icon::Database,
        "role" | "user" | "login" => Icon::User,
        "engine" => Icon::Settings,
        "charset" => Icon::Text,
        "index" => Icon::Index,
        "matview" => Icon::View,
        _ => Icon::Table,
    }
}

fn call_template(kind: Kind, key: &str, schema: &str, name: &str) -> String {
    match (kind, key) {
        (Kind::SqlServer, "procedure") => format!("EXEC [{}].[{}];\n", schema, name),
        (Kind::SqlServer, _) => format!("SELECT [{}].[{}]();\n", schema, name),
        (Kind::Mysql, "procedure") => format!("CALL {}.{}();\n", kind.quote(schema), kind.quote(name)),
        (Kind::Mysql, _) => format!("SELECT {}.{}();\n", kind.quote(schema), kind.quote(name)),
        (_, "procedure") => format!("CALL {}.{}();\n", kind.quote(schema), kind.quote(name)),
        _ => format!("SELECT {}.{}();\n", kind.quote(schema), kind.quote(name)),
    }
}

/// The definition as a script that can be run to change the object in the database.
/// Editing the text never touches the database by itself: running it (Ctrl+Enter / Alt+X) does.
pub fn editable_source(kind: Kind, key: &str, schema: &str, name: &str, detail: &str, src: &str) -> String {
    let header = format!(
        "-- Código de {}.\n-- Editar este texto NÃO altera o banco. Para aplicar a mudança, execute: Alt+X (script inteiro) ou Ctrl+Enter (instrução sob o cursor).\n",
        name
    );
    let q = |x: &str| kind.quote(x);
    let drop_line = match (kind, key) {
        (Kind::Postgres, "trigger") => Some(format!("DROP TRIGGER IF EXISTS {} ON {}.{};", q(name), q(schema), q(detail))),
        (Kind::Postgres, "index") => Some(format!("DROP INDEX IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Postgres, "matview") => Some(format!("DROP MATERIALIZED VIEW IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Mysql, "procedure") => Some(format!("DROP PROCEDURE IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Mysql, "function") => Some(format!("DROP FUNCTION IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Mysql, "trigger") => Some(format!("DROP TRIGGER IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Mysql, "event") => Some(format!("DROP EVENT IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Mysql, "view") => Some(format!("DROP VIEW IF EXISTS {}.{};", q(schema), q(name))),
        (Kind::Sqlite, "view") => Some(format!("DROP VIEW IF EXISTS {};", q(name))),
        (Kind::Sqlite, "trigger") => Some(format!("DROP TRIGGER IF EXISTS {};", q(name))),
        (Kind::Sqlite, "index") => Some(format!("DROP INDEX IF EXISTS {};", q(name))),
        _ => None,
    };
    let mut body = src.to_string();
    if kind == Kind::SqlServer && matches!(key, "view" | "procedure" | "function" | "trigger" | "dbtrigger") {
        // T-SQL changes an existing object with ALTER
        if let Some(pos) = body.to_uppercase().find("CREATE") {
            body.replace_range(pos..pos + 6, "ALTER");
        }
    }
    match drop_line {
        Some(d) => format!("{}{}\n\n{}\n", header, d, body),
        None => format!("{}{}\n", header, body),
    }
}

/// Text of the Commit button: pending statements (transactions) and affected rows.
pub fn commit_label(statements: usize, rows: u64) -> String {
    format!("Commit · {} instr · {} reg", statements, rows)
}

#[cfg(test)]
mod commit_label_tests {
    use super::commit_label;

    #[test]
    fn label_shows_statements_and_rows() {
        assert_eq!(commit_label(2, 5), "Commit · 2 instr · 5 reg");
        assert_eq!(commit_label(0, 0), "Commit · 0 instr · 0 reg");
    }
}
