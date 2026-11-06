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
                if std::env::var("DBR_DEMO_PROJECT").is_ok() {
                    // preview helper: a project with one organization folder and one connection
                    let dir = std::env::temp_dir().join("sqlion-demo-project");
                    let _ = std::fs::remove_dir_all(&dir);
                    if let Ok(p) = sh.store.create_project("Cliente Acme", &dir) {
                        let _ = sh.store.add_folder(&p.id, "Produção");
                        let mut c2 = ConnConfig::new(Kind::Postgres);
                        c2.name = "Financeiro".into();
                        c2.project = p.id.clone();
                        c2.folder = "Produção".into();
                        sh.store.connections.push(c2);
                        let _ = sh.store.add_folder(&p.id, "Homologação");
                    }
                }
                let sql = "SELECT c.customer_id, c.first_name, c.last_name, c.country,\n       COUNT(o.order_id) AS orders\nFROM customers c\nLEFT JOIN orders o ON o.customer_id = c.customer_id\nWHERE c.country <> 'Chile'\nGROUP BY c.customer_id\nORDER BY orders DESC;";
                // preview helper: DBR_SLOW=1 runs a query that never ends, to show the Cancel button
                let sql = if std::env::var("DBR_SLOW").is_ok() { "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 2000000000) SELECT count(*) FROM c" } else { sql };
                let mut t = SqlTab::new(Some(id.clone()), sql.into(), "<Amostra (SQLite)> Script-1".into());
                t.cursor = (10, 10);
                if std::env::var("DBR_SLOW").is_err() {
                    t.execute(&mut sh, 1, false, false, false);
                }
                tabs.push(Tab { id: 1, kind: TabKind::Sql(Box::new(t)) });
                if std::env::var("DBR_SLOW").is_err() {
                    // (loading a table needs the session, which the endless query holds)
                    let tt = TableTab::new(&mut sh, &id, "main", "customers", false);
                    tabs.push(Tab { id: 2, kind: TabKind::Table(Box::new(tt)) });
                }
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
        let mut active = if demo && std::env::var("DBR_SLOW").is_err() { 1 } else { 0 };
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
        self.open_sql_in(conn, text, title, None);
    }

    /// Opens an SQL tab; a new script file is created in `dir` (the loose scripts folder when `None`).
    pub fn open_sql_in(&mut self, conn: Option<String>, text: String, title: Option<String>, dir: Option<std::path::PathBuf>) {
        let n = self.tabs.iter().filter(|t| matches!(t.kind, TabKind::Sql(_))).count() + 1;
        let cname = conn.as_ref().map(|c| self.sh.conn_name(c)).unwrap_or_else(|| "Sem conexão".into());
        let title_given = title.is_some();
        let title = title.unwrap_or_else(|| format!("<{}> Script-{}", cname, n));
        let path = if self.sh.persist { Some(self.next_script_path(dir.as_deref())) } else { None };
        // new scripts are named like DBeaver does: Script.sql, Script-1.sql, Script-2.sql ... (SQLEditorUtils.java:258)
        let project_tag = path.as_ref().and_then(|p| self.sh.store.project_of_script(p)).map(|p| format!("[{}] ", p.name)).unwrap_or_default();
        let title = match (&path, title_given) {
            (Some(p), false) => format!("{}<{}> {}", project_tag, cname, p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()),
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
    fn next_script_path(&self, dir: Option<&std::path::Path>) -> std::path::PathBuf {
        let dir = dir.unwrap_or(&self.scripts_dir);
        let used: Vec<std::path::PathBuf> = self.tabs.iter().filter_map(|t| if let TabKind::Sql(s) = &t.kind { s.file.clone() } else { None }).collect();
        for n in 0.. {
            let name = if n == 0 { "Script.sql".to_string() } else { format!("Script-{}.sql", n) };
            let p = dir.join(name);
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
            let linked = self.sh.store.script_conn_of(p).filter(|c| self.sh.cfg(c).is_some());
            let c = linked.or_else(|| self.current_conn());
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
        let (old_key, new_key) = (self.sh.store.script_key(path), self.sh.store.script_key(&new_path));
        if let Some(c) = self.sh.store.script_conns.remove(&old_key) {
            self.sh.store.script_conns.insert(new_key, c);
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

    /// Moves a script to a project (`Some(id)`) or to the loose scripts (`None`), following open tabs.
    pub fn move_script_to(&mut self, path: &std::path::Path, project: Option<&str>) {
        let to = match project.and_then(|id| self.sh.store.projects.iter().find(|p| p.id == id)) {
            Some(p) => p.scripts_dir(),
            None => self.scripts_dir.clone(),
        };
        for t in self.tabs.iter_mut() {
            if let TabKind::Sql(s) = &mut t.kind {
                if s.file.as_deref() == Some(path) && s.dirty && !s.text.trim().is_empty() {
                    s.save_file();
                }
            }
        }
        match self.sh.store.move_script(path, &to) {
            Ok(dest) => {
                for t in self.tabs.iter_mut() {
                    if let TabKind::Sql(s) = &mut t.kind {
                        if s.file.as_deref() == Some(path) {
                            s.file = Some(dest.clone());
                        }
                    }
                }
                self.git_cache = None;
                self.sh.save();
                self.toast("Script movido");
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn delete_script(&mut self, path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let key = self.sh.store.script_key(path);
        self.sh.store.script_conns.remove(&key);
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

    /// Saves the open scripts that live in the loose scripts folder, so that a copy of the folder has their latest text.
    pub fn save_loose_scripts(&mut self) {
        let dir = self.scripts_dir.clone();
        for t in self.tabs.iter_mut() {
            if let TabKind::Sql(s) = &mut t.kind {
                if s.dirty && !s.text.trim().is_empty() && s.file.as_ref().map(|f| f.starts_with(&dir)).unwrap_or(false) {
                    s.save_file();
                }
            }
        }
    }

    /// After the default project was converted: tabs whose script moved now point at the project folder.
    pub fn relink_tabs_after_convert(&mut self, old_dir: &std::path::Path, new_dir: &std::path::Path) {
        for t in self.tabs.iter_mut() {
            if let TabKind::Sql(s) = &mut t.kind {
                if let Some(f) = &s.file {
                    if f.parent() == Some(old_dir) {
                        if let Some(name) = f.file_name() {
                            let moved = new_dir.join(name);
                            if moved.is_file() {
                                s.file = Some(moved);
                            }
                        }
                    }
                }
            }
        }
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
        crate::shared::release_memory();
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
        // the results of the closed tab are gone: give their memory back
        crate::shared::release_memory();
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
                ui.menu_button("Novo script em…", |ui| {
                    if ui.button("Scripts soltos").clicked() {
                        self.new_script_in(None);
                        ui.close();
                    }
                    for (id, name) in self.sh.store.projects.iter().map(|p| (p.id.clone(), p.name.clone())).collect::<Vec<_>>() {
                        if ui.button(name).clicked() {
                            self.new_script_in(Some(id));
                            ui.close();
                        }
                    }
                });
                if ui.button("Novo projeto…").clicked() {
                    self.open_new_project_dialog();
                    ui.close();
                }
                if ui.button("Abrir projeto…").clicked() {
                    self.dialog = Some(Dialog::OpenProject { dir: String::new(), msg: None });
                    ui.close();
                }
                if ui.add(egui::Button::new("Novo script SQL").shortcut_text("Ctrl+]")).clicked() {
                    self.new_script_here();
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
    /// Entry point of every mode switch from the UI: going to Auto asks first (always when there are pending changes).
    fn request_tx_mode(&mut self, mode: TxMode) {
        let Some(c) = self.current_conn() else { return };
        let Some((cur, _open, stmts, rows)) = self.sh.tx_info(&c) else { return };
        if needs_auto_commit_confirmation(cur, mode, stmts, rows, self.sh.store.prefs.confirm_auto_commit) {
            self.dialog = Some(Dialog::ConfirmAutoCommit { conn: c, stmts, rows, dont_ask: false });
        } else {
            self.set_tx_mode(mode);
        }
    }

    fn set_tx_mode(&mut self, mode: TxMode) {
        if let Some(c) = self.current_conn() {
            self.set_tx_mode_of(&c, mode);
        }
    }

    pub fn set_tx_mode_of(&mut self, c: &str, mode: TxMode) {
        let c = c.to_string();
        {
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
                self.request_tx_mode(next);
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
                                self.request_tx_mode(m);
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
                self.new_script_here();
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
        let mut close_others: Option<usize> = None;
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
                            // applied after the loop: the loop indexes `self.tabs` and must not see it change
                            close_others = Some(i);
                            ui.close();
                        }
                    });
                }
            });
        });
        if let Some(keep) = close_others {
            self.close_other_tabs(keep);
        } else if let Some(i) = close {
            if i < self.tabs.len() {
                self.close_tab(i);
            }
        }
    }

    /// Closes every tab but `keep`, saving scripts the same way `close_tab` does.
    fn close_other_tabs(&mut self, keep: usize) {
        if keep >= self.tabs.len() {
            return;
        }
        let mut i = self.tabs.len();
        while i > 0 {
            i -= 1;
            if i != keep {
                self.close_tab(i);
            }
        }
        self.active = 0;
    }

    /// Blank state shown when the last tab is closed: the app stays open and offers the usual ways to start.
    fn empty_workspace(&mut self, ui: &mut Ui) {
        let pl = pal(self.sh.store.prefs.dark);
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.3).max(24.0));
            ui.label(RichText::new("Nenhuma aba aberta").size(20.0).strong());
            ui.add_space(4.0);
            ui.label(RichText::new("Abra um script, conecte a um banco ou volte à tela inicial.").color(pl.weak));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let w = 640.0_f32.min(ui.available_width());
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                if icons::primary(ui, Icon::Script, "Novo script").on_hover_text("Ctrl+]").clicked() {
                    self.new_script_here();
                }
                if icons::button(ui, Icon::Plug, "Nova conexão").clicked() {
                    self.sh.req(Req::NewConn);
                }
                if icons::button(ui, Icon::Folder, "Novo projeto").clicked() {
                    self.open_new_project_dialog();
                }
                if icons::button(ui, Icon::Folder, "Abrir projeto").clicked() {
                    self.dialog = Some(Dialog::OpenProject { dir: String::new(), msg: None });
                }
                if icons::button(ui, Icon::Home, "Tela inicial").clicked() {
                    let id = self.sh.fresh_id();
                    self.tabs.push(Tab { id, kind: TabKind::Welcome });
                    self.active = self.tabs.len() - 1;
                }
            });
        });
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
                            self.new_script_here();
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
        let projects = self.sh.store.projects.clone();
        let nothing = self.sh.store.connections.is_empty() && projects.is_empty();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            if nothing {
                ui.add_space(10.0);
                ui.label(RichText::new("Nenhuma conexão ainda.").color(pl.weak));
                ui.horizontal_wrapped(|ui| {
                    if icons::primary(ui, Icon::Plug, "Criar conexão").clicked() {
                        self.sh.req(Req::NewConn);
                    }
                    if icons::button(ui, Icon::Folder, "Novo projeto").clicked() {
                        self.open_new_project_dialog();
                    }
                });
                ui.add_space(6.0);
            }
            self.project_node(ui, None);
            for p in projects {
                self.project_node(ui, Some(&p));
            }
            ui.add_space(8.0);
            if self.sh.persist {
                self.scripts_node(ui, None);
            }
        });
    }

    /// A project in the tree: `None` is the default project (the workspace as it was before projects existed).
    fn project_node(&mut self, ui: &mut Ui, p: Option<&store::Project>) {
        let pl = pal(self.sh.store.prefs.dark);
        let pid = p.map(|p| p.id.clone()).unwrap_or_default();
        let name = p.map(|p| p.name.clone()).unwrap_or_else(|| "Projeto padrão".to_string());
        let conns: Vec<ConnConfig> = self.sh.store.connections.iter().filter(|c| c.project == pid).cloned().collect();
        let folders = self.sh.store.folders_of(&pid);
        let st = CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new(("project", &pid)), true);
        let hdr = st.show_header(ui, |ui| {
            let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.accent), None, RichText::new(&name).strong(), &conns.len().to_string(), false, None);
            let r = match p {
                Some(p) => r.on_hover_text(format!("Projeto: {}", p.path)),
                None => r.on_hover_text(format!("Projeto padrão: dados em {}", store::config_dir().display())),
            };
            r.context_menu(|ui| self.project_menu(ui, p));
        });
        hdr.body(|ui| {
            for f in &folders {
                let fid = egui::Id::new(("folder", &pid, f));
                let n = conns.iter().filter(|c| &c.folder == f).count();
                let st = CollapsingState::load_with_default_open(ui.ctx(), fid, true);
                let h = st.show_header(ui, |ui| {
                    let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.warn), None, RichText::new(f), &n.to_string(), false, None);
                    r.context_menu(|ui| {
                        if ui.button("Renomear pasta…").clicked() {
                            self.dialog = Some(Dialog::Folder { project: pid.clone(), old: Some(f.clone()), name: f.clone(), msg: None });
                            ui.close();
                        }
                        if ui.button("Excluir pasta").on_hover_text("As conexões voltam para a raiz do projeto").clicked() {
                            self.delete_folder(&pid, f);
                            ui.close();
                        }
                    });
                });
                h.body(|ui| {
                    for c in conns.iter().filter(|c| &c.folder == f) {
                        self.conn_node(ui, c);
                    }
                });
            }
            for c in conns.iter().filter(|c| c.folder.is_empty()) {
                self.conn_node(ui, c);
            }
            if conns.is_empty() && folders.is_empty() && p.is_none() {
                ui.label(RichText::new("(vazio)").small().color(pl.weak));
            }
            if let Some(p) = p {
                self.scripts_node(ui, Some(p));
            }
        });
    }

    fn delete_folder(&mut self, project: &str, folder: &str) {
        match self.sh.store.delete_folder(project, folder) {
            Ok(_) => {
                self.sh.save();
                self.toast(format!("Pasta “{}” excluída; as conexões foram para a raiz", folder));
            }
            Err(e) => self.toast(e),
        }
    }

    pub fn open_new_project_dialog(&mut self) {
        self.dialog = Some(Dialog::NewProject { name: String::new(), dir: String::new(), dir_edited: false, msg: None });
    }

    fn project_menu(&mut self, ui: &mut Ui, p: Option<&store::Project>) {
        let pid = p.map(|p| p.id.clone()).unwrap_or_default();
        let is_default = p.is_none();
        let why_default = "O projeto padrão não pode ser renomeado, removido nem excluído; converta-o em um projeto de pasta";
        let rename = ui.add_enabled(!is_default, egui::Button::new("Renomear…"));
        let rename = if is_default { rename.on_disabled_hover_text(why_default) } else { rename };
        if rename.clicked() {
            if let Some(p) = p {
                self.dialog = Some(Dialog::RenameProject { id: p.id.clone(), name: p.name.clone(), msg: None });
            }
            ui.close();
        }
        if is_default && ui.button("Converter em projeto de pasta…").clicked() {
            self.dialog = Some(Dialog::ConvertDefault { name: "Meu projeto".into(), dir: String::new(), dir_edited: false, msg: None });
            ui.close();
        }
        ui.separator();
        if ui.button("Novo script").clicked() {
            self.new_script_in(Some(pid.clone()));
            ui.close();
        }
        if ui.button("Nova conexão neste projeto…").clicked() {
            self.sh.req(if is_default { Req::NewConn } else { Req::NewConnIn(pid.clone()) });
            ui.close();
        }
        if ui.button("Nova pasta…").clicked() {
            self.dialog = Some(Dialog::Folder { project: pid.clone(), old: None, name: String::new(), msg: None });
            ui.close();
        }
        if let Some(p) = p {
            if ui.button("Versionamento (git)…").clicked() {
                self.open_git_dialog_for(p.dir());
                ui.close();
            }
        }
        ui.separator();
        let path = p.map(|p| p.dir()).unwrap_or_else(store::config_dir);
        if ui.button("Abrir a pasta").clicked() {
            crate::shared::open_path(&path);
            ui.close();
        }
        if ui.button("Copiar o caminho").clicked() {
            ui.ctx().copy_text(path.display().to_string());
            ui.close();
        }
        ui.separator();
        let remove = ui.add_enabled(!is_default, egui::Button::new("Remover da lista"));
        let remove = if is_default { remove.on_disabled_hover_text(why_default) } else { remove.on_hover_text("Fecha o projeto; a pasta continua no disco") };
        if remove.clicked() {
            for i in 0..self.tabs.len() {
                if self.tab_in_project(&self.tabs[i], &pid) {
                    if let TabKind::Sql(sq) = &mut self.tabs[i].kind {
                        if sq.dirty && !sq.text.trim().is_empty() {
                            sq.save_file();
                        }
                    }
                }
            }
            self.close_tabs_of_project(&pid);
            self.sh.store.close_project(&pid);
            self.sh.save();
            ui.close();
        }
        let del = ui.add_enabled(!is_default, egui::Button::new("Excluir a pasta…"));
        let del = if is_default { del.on_disabled_hover_text(why_default) } else { del };
        if del.clicked() {
            self.dialog = Some(Dialog::DeleteProject { id: pid.clone(), msg: None });
            ui.close();
        }
    }

    /// Creates a script in the project (`None`/empty id = loose scripts) and opens it.
    pub fn new_script_in(&mut self, project: Option<String>) {
        let dir = match project.as_deref().filter(|p| !p.is_empty()).and_then(|id| self.sh.store.projects.iter().find(|p| p.id == id)) {
            Some(p) => p.scripts_dir(),
            None => self.scripts_dir.clone(),
        };
        let _ = std::fs::create_dir_all(&dir);
        let c = self.current_conn();
        self.open_sql_in(c, String::new(), None, Some(dir));
    }

    /// "Novo script": goes to the project of the current connection, loose otherwise.
    pub fn new_script_here(&mut self) {
        let c = self.current_conn();
        let dir = self.sh.store.new_script_dir(c.as_deref(), &self.scripts_dir);
        self.open_sql_in(c, String::new(), None, Some(dir));
    }

    /// Scripts of a project (`Some`) or the loose scripts (`None`).
    fn scripts_node(&mut self, ui: &mut Ui, project: Option<&store::Project>) {
        let pl = pal(self.sh.store.prefs.dark);
        let dir = project.map(|p| p.scripts_dir()).unwrap_or_else(|| self.scripts_dir.clone());
        let pid = project.map(|p| p.id.clone());
        let label = if project.is_some() { "Scripts" } else { "Scripts soltos" };
        let mut scripts: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map(|e| e == "sql").unwrap_or(false)).collect())
            .unwrap_or_default();
        scripts.sort();
        let st = CollapsingState::load_with_default_open(ui.ctx(), egui::Id::new(("scripts_node", pid.clone().unwrap_or_default())), false);
        let open = st.is_open();
        let changes = if open && project.is_none() { self.git_changes() } else { vec![] };
        let hdr = st.show_header(ui, |ui| {
            let r = icons::tree_row(ui, icons::Lead::Icon(Icon::Folder, pl.warn), None, RichText::new(label).strong(), &scripts.len().to_string(), false, None);
            let r = r.on_hover_text(format!("Pasta: {}", dir.display()));
            r.context_menu(|ui| {
                if ui.button("Novo script").clicked() {
                    self.new_script_in(pid.clone());
                    ui.close();
                }
                if project.is_none() {
                    if ui.button("Versionamento (git)…").clicked() {
                        self.open_git_dialog();
                        ui.close();
                    }
                    if ui.button("Mudar a pasta dos scripts…").clicked() {
                        self.dialog = Some(Dialog::Prefs);
                        ui.close();
                    }
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
                let conn_hint = self.sh.store.script_conn_of(&p).filter(|c| self.sh.cfg(c).is_some()).map(|c| self.sh.conn_name(&c)).unwrap_or_default();
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
                    let targets: Vec<(Option<String>, String)> = std::iter::once((None, "Scripts soltos".to_string())).chain(self.sh.store.projects.iter().map(|x| (Some(x.id.clone()), x.name.clone()))).filter(|(id, _)| *id != pid).collect();
                    ui.menu_button("Mover para…", |ui| {
                        for (id, name) in targets {
                            if ui.button(name).clicked() {
                                self.move_script_to(&p, id.as_deref());
                                ui.close();
                            }
                        }
                    });
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
                let projects = self.sh.store.projects.clone();
                let cur_project = c.project.clone();
                ui.menu_button("Mover para o projeto", |ui| {
                    let mut targets: Vec<(String, String)> = vec![(String::new(), "Projeto padrão".to_string())];
                    targets.extend(projects.iter().map(|p| (p.id.clone(), p.name.clone())));
                    for (pid, pname) in targets.into_iter().filter(|(pid, _)| *pid != cur_project) {
                        if ui.button(pname).clicked() {
                            match self.sh.store.move_connection(&id, &pid) {
                                Ok(_) => {
                                    self.sh.save();
                                    self.toast("Conexão movida");
                                }
                                Err(e) => self.toast(e),
                            }
                            ui.close();
                        }
                    }
                });
                let folders = self.sh.store.folders_of(&cur_project);
                ui.menu_button("Mover para a pasta", |ui| {
                    if !c.folder.is_empty() && ui.button("(raiz do projeto)").clicked() {
                        let _ = self.sh.store.move_to_folder(&id, "");
                        self.sh.save();
                        ui.close();
                    }
                    for f in folders.iter().filter(|f| **f != c.folder) {
                        if ui.button(f).clicked() {
                            let _ = self.sh.store.move_to_folder(&id, f);
                            self.sh.save();
                            ui.close();
                        }
                    }
                    if folders.is_empty() {
                        ui.label(RichText::new("Crie pastas no menu do projeto").small().weak());
                    }
                });
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
            self.new_script_here();
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
                self.empty_workspace(ui);
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
        if self.demo && self.frames == 30 && std::env::var("DBR_SLOW").is_ok() {
            self.sql_cmd(SqlCmd::Execute);
        }
        if self.demo && self.frames == 60 && std::env::var("DBR_CLOSE_ALL").is_ok() {
            // preview helper: closes every tab the way the tab bar does
            while !self.tabs.is_empty() {
                self.close_tab(0);
            }
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
            let busy = busy && std::env::var("DBR_SLOW").is_err();
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

/// Switching to Auto asks first when the preference is on, and always when changes are pending.
pub fn needs_auto_commit_confirmation(from: TxMode, to: TxMode, pending_stmts: usize, pending_rows: u64, ask: bool) -> bool {
    to == TxMode::Auto && from != TxMode::Auto && (pending_stmts > 0 || pending_rows > 0 || ask)
}

#[cfg(test)]
mod auto_commit_confirmation_tests {
    use super::needs_auto_commit_confirmation as need;
    use crate::db::TxMode::*;

    #[test]
    fn confirmation_rules() {
        assert!(need(Manual, Auto, 0, 0, true), "no pending changes, preference on");
        assert!(need(Smart, Auto, 2, 5, true), "pending changes");
        assert!(!need(Manual, Auto, 0, 0, false), "dismissed, nothing pending");
        assert!(need(Manual, Auto, 2, 5, false), "pending changes ask even when dismissed");
        assert!(!need(Auto, Auto, 0, 0, true), "already Auto");
        assert!(!need(Auto, Manual, 0, 0, true), "only Auto asks");
        assert!(!need(Manual, Smart, 3, 3, true));
    }
}
