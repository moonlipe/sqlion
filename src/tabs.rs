//! Editor tabs: SQL editor, table editor (properties / data / DDL / diagram) and ER diagram.
use crate::shared::*;
use crate::db::*;
use crate::editing;
use crate::grid::{self, EditSource, GridAction, GridState, RowState, ViewMode};
use crate::icons::{self, Icon};
use crate::theme::pal;
use crate::sqltext::{self, Tok};
use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Color32, FontId, RichText, Sense, TextFormat, Ui};

pub enum TabKind {
    Welcome,
    Sql(Box<SqlTab>),
    Table(Box<TableTab>),
    Er(Box<ErTab>),
}

/// Suffix shown on tabs whose connection is closed; running or reloading the tab reopens it.
fn closed(sh: &Shared, conn: Option<&str>) -> &'static str {
    match conn {
        Some(c) if !sh.is_connected(c) => " (desc.)",
        _ => "",
    }
}

pub struct Tab {
    pub id: u64,
    pub kind: TabKind,
}

impl Tab {
    pub fn title(&self, sh: &Shared) -> String {
        match &self.kind {
            TabKind::Welcome => "Bem-vindo".into(),
            TabKind::Sql(s) => format!("{}{}{}", if s.dirty { "*" } else { "" }, s.title, closed(sh, s.conn.as_deref())),
            TabKind::Table(t) => format!("{}{} ", t.table, closed(sh, Some(&t.conn))),
            TabKind::Er(e) => format!("Diagrama {}", e.title(sh)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SqlCmd {
    Execute,
    ExecuteNewTab,
    Script,
    Explain,
    Format,
    Comment,
    Upper,
    Lower,
    Complete,
    Save,
}

// ---------------------------------------------------------------- highlighting
pub fn highlight(text: &str, dark: bool, size: f32, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let font = FontId::monospace(size);
    let (kw, st, nu, co, pa, id) = if dark {
        (
            Color32::from_rgb(86, 156, 214),
            Color32::from_rgb(206, 145, 120),
            Color32::from_rgb(181, 206, 168),
            Color32::from_rgb(106, 153, 85),
            Color32::from_rgb(197, 134, 192),
            Color32::from_rgb(212, 212, 212),
        )
    } else {
        (
            Color32::from_rgb(0, 0, 192),
            Color32::from_rgb(42, 120, 42),
            Color32::from_rgb(160, 80, 0),
            Color32::from_rgb(110, 130, 110),
            Color32::from_rgb(130, 0, 130),
            Color32::from_rgb(30, 30, 30),
        )
    };
    for (k, a, b) in sqltext::tokenize(text) {
        let color = match k {
            Tok::Keyword => kw,
            Tok::Str => st,
            Tok::Number => nu,
            Tok::Comment => co,
            Tok::Param => pa,
            _ => id,
        };
        let mut f = TextFormat::simple(font.clone(), color);
        if k == Tok::Comment {
            f.italics = true;
        }
        job.append(&text[a..b], 0.0, f);
    }
    job
}

fn byte_of_char(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|x| x.0).unwrap_or(s.len())
}
fn char_of_byte(s: &str, bi: usize) -> usize {
    s[..bi.min(s.len())].chars().count()
}

// ---------------------------------------------------------------- SQL tab
pub enum ResSel {
    Data(usize),
    Stats,
    Output,
}

pub struct Stat {
    pub sql: String,
    pub affected: u64,
    pub ms: u128,
    pub when: String,
}

pub struct Ac {
    pub items: Vec<String>,
    pub sel: usize,
    pub start_char: usize,
}

pub struct SqlTab {
    pub title: String,
    pub conn: Option<String>,
    pub text: String,
    pub cursor: (usize, usize), // char indexes (primary, secondary)
    pub results: Vec<GridState>,
    pub sel: ResSel,
    pub stats: Vec<Stat>,
    pub output: Vec<String>,
    pub error: Option<String>,
    pub job: Option<Job>,
    pub clear_on_first: bool,
    pub file: Option<std::path::PathBuf>,
    pub dirty: bool,
    pub cmd: Option<SqlCmd>,
    pub ac: Option<Ac>,
    pub set_cursor: Option<usize>,
    pub results_h: f32,
    pub pending_params: Option<Vec<String>>,
    pub confirm_save: Option<usize>,
    pub save_confirmed: bool,
    pub new_script: bool,
    pub typed_pair: Option<usize>,
}

impl SqlTab {
    pub fn new(conn: Option<String>, text: String, title: String) -> SqlTab {
        SqlTab {
            title,
            conn,
            text,
            cursor: (0, 0),
            results: Vec::new(),
            sel: ResSel::Stats,
            stats: Vec::new(),
            output: Vec::new(),
            error: None,
            job: None,
            clear_on_first: false,
            file: None,
            dirty: false,
            cmd: None,
            ac: None,
            set_cursor: None,
            results_h: 260.0,
            pending_params: None,
            confirm_save: None,
            save_confirmed: false,
            new_script: false,
            typed_pair: None,
        }
    }

    pub fn save_file(&mut self) {
        if let Some(p) = &self.file {
            if std::fs::write(p, &self.text).is_ok() {
                self.dirty = false;
            }
        }
    }

    fn log(&mut self, s: impl Into<String>) {
        self.output.push(format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), s.into()));
    }

    fn sel_range_bytes(&self) -> (usize, usize) {
        let a = byte_of_char(&self.text, self.cursor.0.min(self.cursor.1));
        let b = byte_of_char(&self.text, self.cursor.0.max(self.cursor.1));
        (a, b)
    }

    /// Statements to run for the current command.
    fn gather(&self, sh: &Shared, script: bool) -> Vec<String> {
        let delim = sh.store.prefs.statement_delimiter.clone();
        let (a, b) = self.sel_range_bytes();
        let (text, base): (&str, usize) = if a != b { (&self.text[a..b], a) } else { (&self.text, 0) };
        let _ = base;
        if script || a != b && script {
            sqltext::split_statements(text, &delim).into_iter().map(|(x, y)| text[x..y].trim().to_string()).collect()
        } else if a != b {
            vec![text.trim().trim_end_matches(';').to_string()]
        } else {
            match sqltext::statement_at(&self.text, a, &delim) {
                Some((x, y)) => vec![self.text[x..y].trim().to_string()],
                None => vec![],
            }
        }
    }

    pub fn execute(&mut self, sh: &mut Shared, tab_id: u64, script: bool, new_tab: bool, explain: bool) {
        let Some(conn) = self.conn.clone() else {
            sh.status = "Selecione uma conexão ativa para executar a consulta".into();
            self.error = Some("Nenhuma conexão selecionada. Escolha uma conexão na barra de ferramentas.".into());
            return;
        };
        if self.job.is_some() {
            return;
        }
        let stmts = self.gather(sh, script);
        if stmts.is_empty() {
            sh.status = "Nada para executar".into();
            return;
        }
        // control commands (@set, @unset, @echo, @export, @include; SQLConstants.java:695) run in order, so that a variable
        // defined by @set is already known by the statements that follow it
        let mut real = Vec::new();
        let mut pending_export: Option<serde_json::Value> = None;
        let mut stack: Vec<std::path::PathBuf> = self.file.iter().cloned().collect();
        let explain_pre = if explain { Some(if sh.kind(&conn) == Kind::Sqlite { "EXPLAIN QUERY PLAN " } else { "EXPLAIN " }) } else { None };
        if let Err(e) = self.collect(sh, &conn, stmts, &mut stack, &mut real, &mut pending_export, explain_pre) {
            self.log(format!("ERRO: {}", e));
            self.error = Some(e);
            self.sel = ResSel::Output;
            return;
        }
        if real.is_empty() {
            return;
        }
        if sh.store.prefs.params_enabled {
            let mut names = Vec::new();
            for s in &real {
                for n in sqltext::find_params(s) {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            if !names.is_empty() {
                self.pending_params = Some(names.clone());
                sh.req(Req::Params { tab: tab_id, names, stmts: real, new_tab });
                return;
            }
        }
        self.start(sh, &conn, real, new_tab, tab_id);
    }

    pub fn start(&mut self, sh: &mut Shared, conn: &str, stmts: Vec<String>, new_tab: bool, tab_id: u64) {
        self.pending_params = None;
        // DBPConnectionType.java:52,71,90 - production asks before running statements that change data
        let ct = sh.conn_type(conn);
        if ct.confirm_execute() && stmts.iter().any(|s| !is_read_only_sql(s)) {
            sh.req(Req::ConfirmRun { tab: tab_id, stmts, new_tab, env: ct.label().to_string() });
            return;
        }
        self.start_confirmed(sh, conn, stmts, new_tab);
    }

    pub fn start_confirmed(&mut self, sh: &mut Shared, conn: &str, stmts: Vec<String>, new_tab: bool) {
        if let Err(e) = sh.connect(conn) {
            self.error = Some(e);
            return;
        }
        let Some(sess) = sh.session(conn) else { return };
        sh.remember_script_conn(self.file.as_deref(), conn);
        self.error = None;
        self.clear_on_first = !new_tab;
        let max = sh.store.prefs.max_rows;
        let jobs = stmts.into_iter().map(|sql| JobStmt { sql, limit: max, offset: 0, tag: 0 }).collect();
        self.job = Some(spawn_job(sess, jobs, sh.ctx.clone()));
        sh.status = "Executando…".into();
    }

    #[allow(clippy::too_many_arguments)]
    fn collect(
        &mut self,
        sh: &mut Shared,
        conn: &str,
        stmts: Vec<String>,
        stack: &mut Vec<std::path::PathBuf>,
        real: &mut Vec<String>,
        pending_export: &mut Option<serde_json::Value>,
        explain_pre: Option<&str>,
    ) -> Result<(), String> {
        for raw in stmts {
            let clean = sqltext::strip_comments(&raw);
            let c = clean.trim();
            if let Some(cmd) = c.strip_prefix('@') {
                let (name, param) = match cmd.find(char::is_whitespace) {
                    Some(i) => (cmd[..i].to_lowercase(), cmd[i..].trim().to_string()),
                    None => (cmd.to_lowercase(), String::new()),
                };
                match name.as_str() {
                    "set" => self.cmd_set(sh, &param)?,
                    "unset" => self.cmd_unset(sh, &param)?,
                    "echo" => {
                        // variables are replaced, then the text goes to the Output log (SQLCommandEcho.java:36-41)
                        let m = sqltext::substitute_vars(&param, &sh.vars);
                        self.output.push(m);
                    }
                    "export" => {
                        let v: serde_json::Value = serde_json::from_str(&param).map_err(|_| "Sintaxe inválida. Use '@export {\"type\": <tipo>, \"consumer\": {...}, \"processor\": {...}}'".to_string())?;
                        if !v.is_object() {
                            return Err("Sintaxe inválida. Use '@export {\"type\": <tipo>, \"consumer\": {...}, \"processor\": {...}}'".into());
                        }
                        *pending_export = Some(v);
                    }
                    "include" => self.cmd_include(sh, conn, &param, stack, real, pending_export, explain_pre)?,
                    other => return Err(format!("Comando de controle desconhecido: @{}", other)),
                }
                continue;
            }
            let mut q = raw;
            if sh.store.prefs.vars_enabled {
                q = sqltext::substitute_vars(&q, &sh.vars);
            }
            if let Some(ex) = pending_export.take() {
                // the query after @export is not run: it opens the export with the requested settings (SQLPragmaExport.java:111)
                self.request_export(sh, conn, &ex, &q)?;
                continue;
            }
            if let Some(pre) = explain_pre {
                q = format!("{}{}", pre, q);
            }
            real.push(q);
        }
        Ok(())
    }

    /// @set varName = value (SQLCommandSet.java:35-58): the value may use ${other} variables; it is stored as written.
    fn cmd_set(&mut self, sh: &mut Shared, param: &str) -> Result<(), String> {
        const SYNTAX: &str = "Sintaxe esperada:\n@set nomeVariavel = valor ou expressão";
        let p = param.trim_start();
        let end = var_name_end(p).ok_or_else(|| format!("Falta o nome da variável. {}", SYNTAX))?;
        let name = unquote_name(&p[..end]);
        let eq = p[end..].find('=').ok_or_else(|| format!("Sintaxe de @set inválida. {}", SYNTAX))? + end;
        let between = p[end..eq].trim();
        if !between.is_empty() {
            return Err(format!("Caracteres inesperados “{}” depois do nome da variável {}. {}", between, name, SYNTAX));
        }
        let value = sqltext::substitute_vars(p[eq + 1..].trim(), &sh.vars);
        sh.vars.retain(|(n, _)| n != &name);
        sh.vars.push((name, value));
        Ok(())
    }

    /// @unset varName (SQLCommandUnset.java:33-45)
    fn cmd_unset(&mut self, sh: &mut Shared, param: &str) -> Result<(), String> {
        let p = param.trim();
        if var_name_end(p) != Some(p.len()) {
            return Err("Comando @unset inválido. Sintaxe esperada:\n@unset nomeVariavel".into());
        }
        let name = unquote_name(p);
        sh.vars.retain(|(n, _)| n != &name);
        Ok(())
    }

    /// @include file (SQLCommandInclude.java:55-103): relative to the folder of the current script, then as typed;
    /// runs the other script's statements here; a file that includes itself (even indirectly) is an error.
    #[allow(clippy::too_many_arguments)]
    fn cmd_include(
        &mut self,
        sh: &mut Shared,
        conn: &str,
        param: &str,
        stack: &mut Vec<std::path::PathBuf>,
        real: &mut Vec<String>,
        pending_export: &mut Option<serde_json::Value>,
        explain_pre: Option<&str>,
    ) -> Result<(), String> {
        if param.trim().is_empty() {
            return Err("Arquivo de entrada vazio".into());
        }
        let mut file = sqltext::substitute_vars(param, &sh.vars).trim().to_string();
        if file.len() > 1 && ((file.starts_with('"') && file.ends_with('"')) || (file.starts_with('`') && file.ends_with('`'))) {
            file = file[1..file.len() - 1].to_string();
        }
        let cand = match stack.last().and_then(|f| f.parent()) {
            Some(dir) => dir.join(&file),
            None => std::path::PathBuf::from(&file),
        };
        let path = if cand.exists() { cand } else { std::path::PathBuf::from(&file) };
        if !path.exists() {
            return Err(format!("Arquivo '{}' não encontrado", file));
        }
        let path = path.canonicalize().unwrap_or(path);
        if stack.iter().any(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == path) {
            return Err(format!("Arquivo '{}': inclusão recursiva", file));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("Não foi possível ler '{}': {}", file, e))?;
        let delim = sh.store.prefs.statement_delimiter.clone();
        let inc: Vec<String> = sqltext::split_statements(&text, &delim).into_iter().map(|(a, b)| text[a..b].trim().to_string()).collect();
        stack.push(path);
        let r = self.collect(sh, conn, inc, stack, real, pending_export, explain_pre);
        stack.pop();
        r
    }

    /// Turns the JSON of @export into the export window, filled in (type, processor settings, consumer folder and file pattern).
    fn request_export(&mut self, sh: &mut Shared, conn: &str, v: &serde_json::Value, sql: &str) -> Result<(), String> {
        use crate::export::{ExportOptions, Format};
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty.is_empty() {
            return Err("O atributo `type` é obrigatório".into());
        }
        let format = match ty {
            "csv" => Format::Csv,
            "json" => Format::Json,
            "xml" => Format::Xml,
            "html" => Format::Html,
            "sql" => Format::Sql,
            "markdown.table" | "markdown" => Format::Markdown,
            "txt" => Format::Txt,
            "dbunit" => Format::DbUnit,
            other => return Err(format!("Não existe processador do tipo '{}'", other)),
        };
        let mut o = ExportOptions { format, table_name: "result".into(), ..Default::default() };
        let text = |m: &serde_json::Map<String, serde_json::Value>, k: &str| m.get(k).map(|x| x.as_str().map(|s| s.to_string()).unwrap_or_else(|| x.to_string()));
        let flag = |m: &serde_json::Map<String, serde_json::Value>, k: &str| m.get(k).map(|x| x.as_bool().unwrap_or_else(|| x.as_str().map(|s| s == "true" || s == "top").unwrap_or(false)));
        if let Some(p) = v.get("processor").and_then(|x| x.as_object()) {
            if let Some(d) = text(p, "delimiter") {
                o.delimiter = d;
            }
            if let Some(h) = flag(p, "header").or_else(|| flag(p, "showHeader")) {
                o.header = h;
            }
            if let Some(q) = text(p, "quoteChar") {
                o.quote_char = q;
            }
            if let Some(n) = text(p, "nullString") {
                o.null_string = n;
            }
            if let Some(n) = text(p, "rowsInStatement").and_then(|s| s.parse::<usize>().ok()) {
                o.rows_in_statement = n;
            }
            if let Some(b) = flag(p, "omitSchema") {
                o.omit_schema = b;
            }
            if let Some(b) = flag(p, "includeDoctype") {
                o.include_doctype = b;
            }
            if let Some(b) = flag(p, "showNulls") {
                o.show_nulls = b;
            }
            if let Some(b) = flag(p, "showHeaderSeparator") {
                o.md_header_sep = b;
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into()); // default output folder = user home (StreamConsumerSettings.java:124)
        let (mut dir, mut pattern) = (home, crate::constants::EXPORT_FILE_PATTERN.to_string());
        if let Some(c) = v.get("consumer").and_then(|x| x.as_object()) {
            if let Some(f) = text(c, "outputFolder") {
                dir = f;
            }
            if let Some(p) = text(c, "outputFilePattern") {
                pattern = p;
            }
        }
        let file = pattern.replace("${table}", "result").replace("${timestamp}", &chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
        sh.req(Req::ExportWith { conn: conn.to_string(), sql: sql.trim().trim_end_matches(';').to_string(), opts: Box::new(o), dir, file });
        Ok(())
    }

    pub fn poll(&mut self, sh: &mut Shared) {
        let Some(mut job) = self.job.take() else { return };
        let mut finished = false;
        let conn = self.conn.clone().unwrap_or_default();
        while let Ok(msg) = job.rx.try_recv() {
            match msg {
                JobMsg::Item(it) => {
                    job.done += 1;
                    self.on_item(sh, &conn, it);
                }
                JobMsg::Finished => finished = true,
            }
        }
        if finished {
            self.detect_edit(sh);
            sh.status = format!("Concluído em {} ms", job.started.elapsed().as_millis());
            if let Some(i) = self.results.len().checked_sub(1) {
                if matches!(self.sel, ResSel::Data(_)) || self.error.is_none() && self.stats.is_empty() {
                    self.sel = ResSel::Data(i);
                }
            }
            if self.error.is_some() {
                self.sel = ResSel::Output;
            }
        } else {
            self.job = Some(job);
        }
    }

    /// Marks results that come from a single plain table as editable.
    fn detect_edit(&mut self, sh: &mut Shared) {
        let conn = match &self.conn {
            Some(c) => c.clone(),
            None => return,
        };
        for g in self.results.iter_mut() {
            if g.edit.is_some() || g.readonly_reason.is_some() {
                continue;
            }
            match editing::detect_source(sh, &conn, &g.qr.sql, &g.qr.columns) {
                Ok(src) => {
                    g.editable = true;
                    g.edit = Some(src);
                }
                Err(why) => g.readonly_reason = Some(why),
            }
        }
    }

    fn on_item(&mut self, sh: &mut Shared, conn: &str, it: JobItem) {
        match it.res {
            Ok(qr) => {
                let rows = if qr.columns.is_empty() { format!("{} afetadas", qr.affected.unwrap_or(0)) } else { format!("{} linhas", qr.rows.len()) };
                sh.history(conn, &it.sql, qr.elapsed_ms, rows.clone(), true);
                self.log(format!("OK ({} ms, {}): {}", qr.elapsed_ms, rows, one_line(&it.sql)));
                if it.tag > 0 {
                    // fetch next page: append to an existing result
                    let idx = it.tag - 1;
                    if let Some(g) = self.results.get_mut(idx) {
                        let more = qr.has_more;
                        let added = qr.rows.len();
                        g.qr.rows.extend(qr.rows);
                        g.row_state.extend(std::iter::repeat(RowState::Clean).take(added));
                        g.qr.has_more = more;
                        g.dirty = true;
                    }
                    return;
                }
                if qr.columns.is_empty() {
                    self.stats.push(Stat { sql: it.sql.clone(), affected: qr.affected.unwrap_or(0), ms: qr.elapsed_ms, when: chrono::Local::now().format("%H:%M:%S").to_string() });
                    if self.results.is_empty() {
                        self.sel = ResSel::Stats;
                    }
                } else {
                    if self.clear_on_first {
                        self.results.clear();
                        self.clear_on_first = false;
                    }
                    if self.results.len() >= crate::constants::MAX_RESULT_TABS {
                        self.log(format!("Limite de {} abas de resultado por execução atingido: o resultado de “{}” não foi aberto", crate::constants::MAX_RESULT_TABS, one_line(&it.sql)));
                        return;
                    }
                    let n = self.results.len() + 1;
                    let mut g = GridState::new(qr, &format!("Resultado {}", n));
                    g.server_sort = false;
                    self.results.push(g);
                    self.sel = ResSel::Data(self.results.len() - 1);
                }
            }
            Err(e) => {
                sh.history(conn, &it.sql, 0, "erro".into(), false);
                self.log(format!("ERRO: {}", e));
                self.error = Some(e);
            }
        }
    }

    fn apply_cmd(&mut self, sh: &mut Shared, tab_id: u64, cmd: SqlCmd) {
        let upper = sh.store.prefs.keyword_upper;
        let (a, b) = self.sel_range_bytes();
        match cmd {
            SqlCmd::Execute => self.execute(sh, tab_id, false, false, false),
            SqlCmd::ExecuteNewTab => self.execute(sh, tab_id, false, true, false),
            SqlCmd::Script => self.execute(sh, tab_id, true, false, false),
            SqlCmd::Explain => self.execute(sh, tab_id, false, true, true),
            SqlCmd::Format => {
                let delim = sh.store.prefs.statement_delimiter.clone();
                let (fa, fb) = if a != b {
                    (a, b)
                } else if crate::constants::FORMAT_ACTIVE_QUERY {
                    sqltext::statement_at(&self.text, a, &delim).unwrap_or((0, self.text.len()))
                } else {
                    (0, self.text.len())
                };
                let up = if crate::constants::FORMAT_KEYWORD_CASE_AUTO { sqltext::auto_upper(&self.text[fa..fb], upper) } else { upper };
                let f = sqltext::format_sql(&self.text[fa..fb], up);
                self.text.replace_range(fa..fb, &f);
                self.dirty = true;
            }
            SqlCmd::Comment => {
                // whole lines touched by the selection
                let ls = self.text[..a].rfind('\n').map(|i| i + 1).unwrap_or(0);
                let le = self.text[b..].find('\n').map(|i| i + b).unwrap_or(self.text.len());
                let t = sqltext::toggle_comment(&self.text[ls..le]);
                self.text.replace_range(ls..le, &t);
                self.dirty = true;
            }
            SqlCmd::Upper | SqlCmd::Lower => {
                if a != b {
                    let t = if cmd == SqlCmd::Upper { self.text[a..b].to_uppercase() } else { self.text[a..b].to_lowercase() };
                    self.text.replace_range(a..b, &t);
                    self.dirty = true;
                }
            }
            SqlCmd::Complete => {
                self.start_complete(sh);
                if crate::constants::AUTOCOMPLETE_INSERT_SINGLE {
                    if let Some(ac) = &self.ac {
                        if ac.items.len() == 1 {
                            let (item, start) = (ac.items[0].clone(), ac.start_char);
                            self.insert_proposal(&item, start);
                        }
                    }
                }
            }
            SqlCmd::Save => self.save_file(),
        }
    }

    /// Replaces the word before the cursor with the chosen proposal.
    fn insert_proposal(&mut self, item: &str, start_char: usize) {
        let a = byte_of_char(&self.text, start_char);
        let b = byte_of_char(&self.text, self.cursor.0);
        let mut ins = item.to_string();
        let next = self.text[b..].chars().next();
        if crate::constants::AUTOCOMPLETE_SPACE_AFTER && !matches!(next, Some(' ') | Some('.') | Some('(') | Some(',') | Some(';')) {
            ins.push(' ');
        }
        self.text.replace_range(a..b, &ins);
        self.set_cursor = Some(start_char + ins.chars().count());
        self.ac = None;
        self.dirty = true;
    }

    fn start_complete(&mut self, sh: &mut Shared) {
        let cur = byte_of_char(&self.text, self.cursor.0);
        self.ac = complete(sh, self.conn.as_deref(), &self.text, cur);
    }

    pub fn show(&mut self, ui: &mut Ui, sh: &mut Shared, tab_id: u64, dark: bool) {
        self.poll(sh);
        if self.job.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(cmd) = self.cmd.take() {
            self.apply_cmd(sh, tab_id, cmd);
        }
        let h = ui.available_height();
        self.results_h = self.results_h.clamp(80.0, (h - 80.0).max(80.0));
        egui::Panel::bottom(egui::Id::new(("sql_results", tab_id)))
            .resizable(true)
            .size_range(80.0..=(h - 80.0).max(81.0))
            .default_size(self.results_h)
            .show(ui, |ui| {
                self.results_h = ui.available_height();
                self.show_results(ui, sh);
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.show_editor(ui, sh, dark, tab_id));
    }

    fn show_editor(&mut self, ui: &mut Ui, sh: &mut Shared, dark: bool, tab_id: u64) {
        let size = sh.store.prefs.font_size;
        // autocomplete key handling (before the editor sees the keys)
        let mut accept: Option<String> = None;
        if let Some(ac) = &mut self.ac {
            let n = ac.items.len().max(1);
            ui.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                    ac.sel = (ac.sel + 1) % n;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                    ac.sel = (ac.sel + n - 1) % n;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                    accept = ac.items.get(ac.sel).cloned();
                }
            });
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.ac = None;
            }
        }
        if let Some(item) = accept {
            if let Some(ac) = self.ac.take() {
                self.insert_proposal(&item, ac.start_char);
            }
        }

        // auto-close quotes and brackets (SQLEDITOR_CLOSE_SINGLE_QUOTES / DOUBLE_QUOTES / BRACKETS = true)
        if crate::constants::AUTO_CLOSE_PAIRS && self.cursor.0 == self.cursor.1 && self.ac.is_none() {
            let cur_b = byte_of_char(&self.text, self.cursor.0);
            let next = self.text[cur_b..].chars().next();
            let prev = self.text[..cur_b].chars().next_back();
            let mut skip = false;
            let mut pair = false;
            ui.input_mut(|i| {
                for ev in i.events.iter_mut() {
                    if let egui::Event::Text(tx) = ev {
                        let mut it = tx.chars();
                        let (Some(ch), None) = (it.next(), it.next()) else { continue };
                        match ch {
                            ')' | ']' | '}' if next == Some(ch) => {
                                tx.clear();
                                skip = true;
                            }
                            '\'' | '"' if next == Some(ch) => {
                                tx.clear();
                                skip = true;
                            }
                            '(' => {
                                *tx = "()".into();
                                pair = true;
                            }
                            '[' => {
                                *tx = "[]".into();
                                pair = true;
                            }
                            '{' => {
                                *tx = "{}".into();
                                pair = true;
                            }
                            '\'' | '"' if !prev.map(|p| p.is_alphanumeric()).unwrap_or(false) => {
                                *tx = format!("{}{}", ch, ch);
                                pair = true;
                            }
                            _ => {}
                        }
                        break;
                    }
                }
            });
            if skip || pair {
                self.set_cursor = Some(self.cursor.0 + 1);
            }
        }

        let editor_h = ui.available_height();
        egui::ScrollArea::vertical().id_salt(("sql_scroll", tab_id)).auto_shrink([false, false]).show(ui, |ui| {
            let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                let job = highlight(text.as_str(), dark, size, wrap);
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let id = egui::Id::new(("sql_edit", tab_id));
            if let Some(pos) = self.set_cursor.take() {
                let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
                st.store(ui.ctx(), id);
            }
            let out = egui::TextEdit::multiline(&mut self.text)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .code_editor()
                .desired_width(f32::INFINITY)
                .desired_rows(((editor_h / (size + 4.0)) as usize).max(4))
                .hint_text("Digite SQL aqui… (Ctrl+Enter executa a instrução, Alt+X executa o script)")
                .layouter(&mut layouter)
                .show(ui);
            if out.response.changed() {
                self.dirty = true;
                // proposals pop up by themselves after a dot or two letters (ENABLE_AUTO_ACTIVATION = true, delay 0)
                let cur_b = byte_of_char(&self.text, out.cursor_range.map(|r| r.primary.index.into()).unwrap_or(self.cursor.0));
                let before = &self.text[..cur_b];
                let word_len = before.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').count();
                let after_dot = before.ends_with('.');
                if crate::constants::AUTOCOMPLETE_AUTO_ACTIVATION && (after_dot || word_len >= 2) {
                    self.cursor = (out.cursor_range.map(|r| r.primary.index.into()).unwrap_or(self.cursor.0), out.cursor_range.map(|r| r.secondary.index.into()).unwrap_or(self.cursor.1));
                    self.start_complete(sh);
                } else {
                    self.ac = None;
                }
            }
            if let Some(r) = out.cursor_range {
                self.cursor = (r.primary.index.into(), r.secondary.index.into());
            }
            // autocomplete popup
            if let Some(ac) = &self.ac {
                if !ac.items.is_empty() {
                    let caret = out.galley.pos_from_cursor(CCursor::new(self.cursor.0));
                    let pos = out.galley_pos + egui::vec2(caret.left(), caret.bottom() + 2.0);
                    let mut chosen: Option<usize> = None;
                    egui::Area::new(egui::Id::new(("ac_popup", tab_id))).order(egui::Order::Foreground).fixed_pos(pos).show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_max_height(220.0);
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for (i, it) in ac.items.iter().enumerate() {
                                    let r = ui.selectable_label(i == ac.sel, it);
                                    if i == ac.sel {
                                        r.scroll_to_me(None);
                                    }
                                    if r.clicked() {
                                        chosen = Some(i);
                                    }
                                }
                            });
                        });
                    });
                    if let Some(i) = chosen {
                        let item = ac.items[i].clone();
                        let start = ac.start_char;
                        let a = byte_of_char(&self.text, start);
                        let b = byte_of_char(&self.text, self.cursor.0);
                        self.text.replace_range(a..b, &item);
                        self.set_cursor = Some(start + item.chars().count());
                        self.ac = None;
                        self.dirty = true;
                    }
                }
            }
        });
    }

    fn show_results(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let pl = pal(ui.visuals().dark_mode);
        // result tab strip
        ui.horizontal(|ui| {
            let mut close: Option<usize> = None;
            for (i, g) in self.results.iter().enumerate() {
                let active = matches!(self.sel, ResSel::Data(k) if k == i);
                let mark = if g.pending_changes() > 0 { " •" } else { "" };
                let r = icons::segment(ui, Icon::Grid, &format!("{}{}", g.title, mark), active);
                if r.clicked() {
                    self.sel = ResSel::Data(i);
                }
                r.context_menu(|ui| {
                    if ui.button("Fechar").clicked() {
                        close = Some(i);
                        ui.close();
                    }
                });
            }
            if let Some(i) = close {
                self.results.remove(i);
                self.sel = if self.results.is_empty() { ResSel::Stats } else { ResSel::Data(self.results.len() - 1) };
            }
            if icons::segment(ui, Icon::Plan, "Estatísticas", matches!(self.sel, ResSel::Stats)).clicked() {
                self.sel = ResSel::Stats;
            }
            if icons::segment(ui, Icon::Text, "Saída", matches!(self.sel, ResSel::Output)).clicked() {
                self.sel = ResSel::Output;
            }
            if self.job.is_some() {
                ui.spinner();
                ui.label(RichText::new("executando…").color(pl.weak));
            }
        });
        if let Some(e) = &self.error {
            egui::Frame::new().fill(pl.err.gamma_multiply(0.15)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let s = icons::icon_size(ui);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(s, s), egui::Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Cross, pl.err);
                    ui.label(RichText::new(e).color(pl.err));
                });
            });
        }
        let conn = self.conn.clone().unwrap_or_default();
        match self.sel {
            ResSel::Data(i) if i < self.results.len() => {
                let mut fetch: Option<usize> = None;
                let mut export = false;
                let mut view_val: Option<String> = None;
                let mut do_save = false;
                let mut do_cancel = false;
                let ct = sh.conn_type(&conn);
                let mut vp = sh.store.prefs.show_value_panel;
                {
                    let g = &mut self.results[i];
                    ui.horizontal(|ui| {
                        for (m, ic, t) in [(ViewMode::Grid, Icon::Grid, "Grade"), (ViewMode::Text, Icon::Text, "Texto"), (ViewMode::Record, Icon::Record, "Registro")] {
                            if icons::segment(ui, ic, t, g.mode == m).clicked() {
                                g.mode = m;
                            }
                        }
                        ui.separator();
                        let before = g.filter.clone();
                        ui.add(egui::TextEdit::singleline(&mut g.filter).hint_text("Filtrar linhas…").desired_width(170.0));
                        if before != g.filter {
                            g.dirty = true;
                        }
                        ui.separator();
                        ui.label(RichText::new(format!("{} linhas{} • {} ms", g.qr.rows.len(), if g.qr.has_more { "+" } else { "" }, g.qr.elapsed_ms)).color(pl.weak));
                        if g.qr.has_more {
                            if icons::button(ui, Icon::Play, "Próxima página").on_hover_text("Buscar mais linhas").clicked() {
                                fetch = Some(sh.store.prefs.max_rows);
                            }
                            if icons::button(ui, Icon::PlayAll, "Buscar tudo").clicked() {
                                fetch = Some(10_000_000);
                            }
                        }
                        if icons::button(ui, Icon::Export, "Exportar").clicked() {
                            export = true;
                        }
                        if icons::button(ui, Icon::Refresh, "").on_hover_text("Reexecutar a consulta").clicked() {
                            fetch = Some(0);
                        }
                    });
                    match editing::bar(ui, g, &mut vp) {
                        Some(editing::BarEvent::Save) => do_save = true,
                        Some(editing::BarEvent::Cancel) => do_cancel = true,
                        None => {}
                    }
                    if self.confirm_save == Some(i) {
                        let mut yes = false;
                        let mut back = false;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Ambiente de {}: confirmar a gravação de {} alteração(ões)?", ct.label(), g.pending_changes())).color(pl.warn));
                            yes = ui.button("Confirmar e salvar").clicked();
                            back = ui.button("Voltar").clicked();
                        });
                        if yes {
                            self.confirm_save = None;
                            do_save = true;
                        } else if back {
                            self.confirm_save = None;
                        }
                    }
                    ui.add_space(2.0);
                    let nt = sh.store.prefs.null_text.clone();
                    let acts = grid::show(ui, g, &nt, vp);
                    for a in acts {
                        match a {
                            GridAction::ViewValue(v) => view_val = Some(v),
                            GridAction::FetchMore if self.job.is_none() => fetch = Some(sh.store.prefs.max_rows),
                            _ => {}
                        }
                    }
                }
                sh.store.prefs.show_value_panel = vp;
                if do_cancel {
                    self.results[i].revert();
                }
                if do_save {
                    if ct.confirm_data_change() && self.confirm_save != Some(i) && !self.save_confirmed {
                        self.confirm_save = Some(i);
                    } else {
                        self.save_confirmed = false;
                        let script = match &self.results[i].edit {
                            Some(src) => editing::change_script(&self.results[i], src),
                            None => vec![],
                        };
                        match editing::apply(sh, &conn, &script) {
                            Ok(_) => {
                                sh.status = format!("{} alteração(ões) salva(s)", script.len());
                                self.error = None;
                                fetch = Some(0);
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
                let g_sql = self.results[i].qr.sql.clone();
                let g_off = self.results[i].qr.rows.len();
                if let Some(v) = view_val {
                    sh.req(Req::ViewValue(v));
                }
                if export {
                    sh.req(Req::Export { conn: conn.clone(), sql: g_sql.clone(), name: "result".into(), rows: None });
                }
                if let Some(n) = fetch {
                    if let Some(sess) = sh.session(&conn) {
                        if n == 0 {
                            let max = sh.store.prefs.max_rows;
                            self.clear_on_first = false;
                            self.results.remove(i);
                            self.job = Some(spawn_job(sess, vec![JobStmt { sql: g_sql, limit: max, offset: 0, tag: 0 }], sh.ctx.clone()));
                        } else {
                            self.job = Some(spawn_job(sess, vec![JobStmt { sql: g_sql, limit: n, offset: g_off, tag: i + 1 }], sh.ctx.clone()));
                        }
                    }
                }
            }
            ResSel::Stats | ResSel::Data(_) => {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    if self.stats.is_empty() {
                        ui.label(RichText::new("Execute uma instrução para ver as estatísticas.").color(pl.weak));
                    }
                    egui::Grid::new("stats_grid").striped(true).num_columns(4).spacing([18.0, 6.0]).show(ui, |ui| {
                        for h in ["Consulta", "Linhas afetadas", "Tempo (ms)", "Hora"] {
                            ui.label(RichText::new(h).strong().color(pl.weak));
                        }
                        ui.end_row();
                        for s in self.stats.iter().rev().take(200) {
                            ui.label(one_line(&s.sql));
                            ui.label(s.affected.to_string());
                            ui.label(s.ms.to_string());
                            ui.label(&s.when);
                            ui.end_row();
                        }
                    });
                });
            }
            ResSel::Output => {
                egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for l in &self.output {
                        ui.label(RichText::new(l).monospace());
                    }
                });
            }
        }
    }
}

pub fn one_line(s: &str) -> String {
    let t: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > 140 {
        format!("{}…", t.chars().take(140).collect::<String>())
    } else {
        t
    }
}

// ---------------------------------------------------------------- autocomplete
pub fn complete(sh: &mut Shared, conn: Option<&str>, text: &str, cursor: usize) -> Option<Ac> {
    let start = text[..cursor].rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == '"')).map(|i| i + text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)).unwrap_or(0);
    let word = &text[start..cursor];
    let (qual, part) = match word.rsplit_once('.') {
        Some((q, p)) => (Some(q.trim_matches('"').to_string()), p.to_string()),
        None => (None, word.to_string()),
    };
    let part_l = part.trim_matches('"').to_lowercase();
    let part_start = start + word.len() - part.len();
    let mut items: Vec<String> = Vec::new();
    let mut push = |items: &mut Vec<String>, s: String| {
        let hit = if crate::constants::AUTOCOMPLETE_MATCH_CONTAINS { s.to_lowercase().contains(&part_l) } else { s.to_lowercase().starts_with(&part_l) };
        if hit && !items.contains(&s) {
            items.push(s);
        }
    };
    // statement around the cursor to resolve aliases
    let delim = sh.store.prefs.statement_delimiter.clone();
    let stmt = sqltext::statement_at(text, cursor.saturating_sub(1), &delim).map(|(a, b)| text[a..b].to_string()).unwrap_or_default();
    let refs = sqltext::referenced_tables(&stmt);
    if let Some(conn) = conn {
        if sh.is_connected(conn) {
            let schema = sh.default_schema(conn);
            let tables = sh.ensure_tables(conn, &schema);
            if let Some(q) = &qual {
                let ql = q.to_lowercase();
                let target = refs.iter().find(|(t, a)| a.to_lowercase() == ql || t.to_lowercase() == ql).map(|(t, _)| t.clone()).or_else(|| tables.iter().find(|(t, _)| t.to_lowercase() == ql).map(|(t, _)| t.clone()));
                if let Some(t) = target {
                    let m = sh.ensure_tmeta(conn, &schema, &t);
                    for c in m.columns {
                        push(&mut items, c.name);
                    }
                } else if sh.ensure_schemas(conn).iter().any(|s| s.to_lowercase() == ql) {
                    for (t, _) in tables.iter() {
                        push(&mut items, t.clone());
                    }
                }
            } else {
                for (t, _) in &refs {
                    let m = sh.ensure_tmeta(conn, &schema, t);
                    for c in m.columns {
                        push(&mut items, c.name);
                    }
                }
                for (_, a) in &refs {
                    push(&mut items, a.clone());
                }
                for (t, _) in tables.iter() {
                    push(&mut items, t.clone());
                }
            }
        }
    }
    if qual.is_none() {
        for k in sqltext::KEYWORDS {
            push(&mut items, k.to_string());
        }
    }
    if crate::constants::AUTOCOMPLETE_SORT_ALPHA {
        items.sort_by_key(|s| s.to_lowercase());
    }
    items.truncate(60);
    if items.is_empty() || (items.len() == 1 && items[0].eq_ignore_ascii_case(part.trim_matches('"')) && !part.is_empty()) {
        return None;
    }
    Some(Ac { items, sel: 0, start_char: char_of_byte(text, part_start) })
}

// ---------------------------------------------------------------- table tab
#[derive(Clone, Copy, PartialEq)]
pub enum TableSub {
    Properties,
    Data,
    Ddl,
    Diagram,
}

pub struct TableTab {
    pub conn: String,
    pub schema: String,
    pub table: String,
    pub view: bool,
    pub sub: TableSub,
    pub meta: TableMeta,
    pub src: EditSource,
    pub grid: Option<GridState>,
    pub where_text: String,
    pub applied_where: String,
    pub job: Option<Job>,
    pub error: Option<String>,
    pub loaded: bool,
    pub total: Option<u64>,
    pub er: Option<ErTab>,
    pub confirm_save: bool,
}

impl TableTab {
    pub fn new(sh: &mut Shared, conn: &str, schema: &str, table: &str, view: bool) -> TableTab {
        let mut meta = sh.ensure_tmeta(conn, schema, table);
        if view {
            if let Some(sess) = sh.session(conn) {
                if let Ok(def) = sess.lock().unwrap_or_else(|e| e.into_inner()).object_source("view", schema, table, "") {
                    meta.ddl = def;
                }
            }
        }
        let src = EditSource { conn: conn.into(), schema: schema.into(), table: table.into(), meta: meta.clone(), qualified: sh.qualified(conn, schema, table), kind: sh.kind(conn) };
        TableTab {
            conn: conn.into(),
            schema: schema.into(),
            table: table.into(),
            view,
            sub: TableSub::Data,
            meta,
            src,
            grid: None,
            where_text: String::new(),
            applied_where: String::new(),
            job: None,
            error: None,
            loaded: false,
            total: None,
            er: None,
            confirm_save: false,
        }
    }

    fn base_sql(&self, sh: &Shared) -> String {
        let q = &self.src.qualified;
        let k = sh.kind(&self.conn);
        let w = self.applied_where.trim();
        if k == Kind::Mongo {
            let mut s = format!("{}.find({})", q, if w.is_empty() { "{}" } else { w });
            if let Some(g) = &self.grid {
                if let Some((c, asc)) = g.sort {
                    if let Some(name) = g.qr.columns.get(c) {
                        s.push_str(&format!(".sort({{{}: {}}})", serde_json::to_string(name).unwrap_or_default(), if asc { 1 } else { -1 }));
                    }
                }
            }
            return s;
        }
        let mut sql = format!("SELECT * FROM {}", q);
        if !w.is_empty() {
            let w = w.strip_prefix("WHERE ").or_else(|| w.strip_prefix("where ")).unwrap_or(w);
            sql.push_str(&format!(" WHERE {}", w));
        }
        if let Some(g) = &self.grid {
            if let Some((c, asc)) = g.sort {
                if let Some(name) = g.qr.columns.get(c) {
                    sql.push_str(&format!(" ORDER BY {} {}", k.quote(name), if asc { "ASC" } else { "DESC" }));
                }
            }
        }
        sql
    }

    pub fn reload(&mut self, sh: &mut Shared) {
        if let Err(e) = sh.connect(&self.conn) {
            self.error = Some(e);
            return;
        }
        let Some(sess) = sh.session(&self.conn) else { return };
        let sql = self.base_sql(sh);
        self.error = None;
        self.job = Some(spawn_job(sess, vec![JobStmt { sql, limit: sh.store.prefs.max_rows, offset: 0, tag: 0 }], sh.ctx.clone()));
    }

    pub fn poll(&mut self, sh: &mut Shared) {
        let Some(job) = self.job.take() else { return };
        let mut fin = false;
        while let Ok(m) = job.rx.try_recv() {
            match m {
                JobMsg::Item(it) => match it.res {
                    Ok(qr) => {
                        sh.history(&self.conn, &it.sql, qr.elapsed_ms, format!("{} linhas", qr.rows.len()), true);
                        if it.tag > 0 {
                            if let Some(g) = self.grid.as_mut() {
                                let n = qr.rows.len();
                                g.qr.has_more = qr.has_more;
                                g.qr.rows.extend(qr.rows);
                                g.row_state.extend(std::iter::repeat(RowState::Clean).take(n));
                                g.dirty = true;
                            }
                        } else {
                            let sort = self.grid.as_ref().and_then(|g| g.sort);
                            let mode = self.grid.as_ref().map(|g| g.mode).unwrap_or(ViewMode::Grid);
                            let mut g = GridState::new(qr, &self.table);
                            g.edit = Some(self.src.clone());
                            if self.view {
                                g.readonly_reason = Some("Visões não podem ser editadas".into());
                            } else {
                                g.editable = true;
                            }
                            g.sort = sort;
                            g.mode = mode;
                            self.grid = Some(g);
                        }
                        self.loaded = true;
                    }
                    Err(e) => {
                        sh.history(&self.conn, &it.sql, 0, "erro".into(), false);
                        self.error = Some(e);
                    }
                },
                JobMsg::Finished => fin = true,
            }
        }
        if !fin {
            self.job = Some(job);
        }
    }

    fn save(&mut self, sh: &mut Shared) {
        let Some(g) = &self.grid else { return };
        let script = editing::change_script(g, &self.src);
        if script.is_empty() {
            return;
        }
        match editing::apply(sh, &self.conn, &script) {
            Ok(_) => {
                sh.status = format!("{} alteração(ões) salva(s)", script.len());
                self.error = None;
                if let Some(g) = self.grid.as_mut() {
                    g.row_state.iter_mut().for_each(|s| *s = RowState::Clean);
                    g.undo.clear();
                }
                self.reload(sh);
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, sh: &mut Shared, tab_id: u64) {
        let pl = pal(ui.visuals().dark_mode);
        self.poll(sh);
        if self.job.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        ui.horizontal(|ui| {
            for (s, ic, l) in [(TableSub::Data, Icon::Grid, "Dados"), (TableSub::Properties, Icon::Table, "Propriedades"), (TableSub::Ddl, Icon::Script, "DDL"), (TableSub::Diagram, Icon::Diagram, "Diagrama")] {
                if icons::segment(ui, ic, l, self.sub == s).clicked() {
                    self.sub = s;
                }
            }
            ui.separator();
            ui.label(RichText::new(format!("{}  ›  {}  ›  {}", sh.conn_name(&self.conn), self.schema, self.table)).color(pl.weak));
        });
        ui.add_space(2.0);
        match self.sub {
            TableSub::Properties => self.show_props(ui),
            TableSub::Ddl => {
                let mut t = self.meta.ddl.clone();
                ui.horizontal(|ui| {
                    if icons::button(ui, Icon::Copy, "Copiar DDL").clicked() {
                        ui.ctx().copy_text(t.clone());
                    }
                    if icons::button(ui, Icon::Script, "Abrir no editor SQL").clicked() {
                        sh.req(Req::OpenSql { conn: Some(self.conn.clone()), text: t.clone(), title: None });
                    }
                });
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut t).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
                });
            }
            TableSub::Diagram => {
                if self.er.is_none() {
                    self.er = Some(ErTab::build(sh, &self.conn, &self.schema, Some(&self.table)));
                }
                if let Some(er) = self.er.as_mut() {
                    er.show(ui, sh, tab_id);
                }
            }
            TableSub::Data => self.show_data(ui, sh),
        }
    }

    fn show_props(&self, ui: &mut Ui) {
        let pl = pal(ui.visuals().dark_mode);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            icons::section(ui, "Colunas");
            egui::Grid::new("cols_grid").striped(true).num_columns(6).spacing([22.0, 6.0]).show(ui, |ui| {
                for h in ["#", "Nome", "Tipo", "Obrigatória", "Padrão", "Chave"] {
                    ui.label(RichText::new(h).strong().color(pl.weak));
                }
                ui.end_row();
                for (i, c) in self.meta.columns.iter().enumerate() {
                    ui.label(RichText::new((i + 1).to_string()).color(pl.weak));
                    ui.label(if c.pk { RichText::new(&c.name).strong().color(pl.warn) } else { RichText::new(&c.name) });
                    ui.label(RichText::new(&c.typ).color(pl.accent));
                    ui.label(if c.nullable { "" } else { "✔" });
                    ui.label(c.default.clone().unwrap_or_default());
                    ui.label(if c.pk { "PRI" } else { "" });
                    ui.end_row();
                }
            });
            ui.add_space(14.0);
            icons::section(ui, "Índices");
            egui::Grid::new("idx_grid").striped(true).num_columns(3).spacing([22.0, 6.0]).show(ui, |ui| {
                for h in ["Nome", "Único", "Colunas"] {
                    ui.label(RichText::new(h).strong().color(pl.weak));
                }
                ui.end_row();
                for i in &self.meta.indexes {
                    ui.label(&i.name);
                    ui.label(if i.unique { "✔" } else { "" });
                    ui.label(i.columns.join(", "));
                    ui.end_row();
                }
            });
            ui.add_space(14.0);
            icons::section(ui, "Chaves estrangeiras");
            egui::Grid::new("fk_grid").striped(true).num_columns(3).spacing([22.0, 6.0]).show(ui, |ui| {
                for h in ["Nome", "Colunas", "Referencia"] {
                    ui.label(RichText::new(h).strong().color(pl.weak));
                }
                ui.end_row();
                for f in &self.meta.fks {
                    ui.label(&f.name);
                    ui.label(f.columns.join(", "));
                    ui.label(format!("{}({})", f.ref_table, f.ref_columns.join(", ")));
                    ui.end_row();
                }
            });
        });
    }

    fn show_data(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let pl = pal(ui.visuals().dark_mode);
        if !self.loaded && self.job.is_none() && self.error.is_none() {
            self.reload(sh);
        }
        let kind = sh.kind(&self.conn);
        let mut do_reload = false;
        let mut do_save = false;
        let mut do_count = false;
        let mut do_fetch: Option<usize> = None;
        let mut export = false;
        let mut cancel = false;
        let mut view_val: Option<String> = None;
        let pending = self.grid.as_ref().map(|g| g.pending_changes()).unwrap_or(0);
        ui.horizontal(|ui| {
            let s = icons::icon_size(ui);
            let (r, _) = ui.allocate_exact_size(egui::vec2(s, s), egui::Sense::hover());
            icons::paint(ui.painter(), r, Icon::Filter, pl.weak);
            let hint = if kind == Kind::Mongo { "Filtro JSON, ex.: {idade: {$gt: 18}}  (Enter aplica)" } else { "Condição SQL, ex.: preco > 10 AND pais = 'BR'  (Enter aplica)" };
            let r = ui.add(egui::TextEdit::singleline(&mut self.where_text).hint_text(hint).desired_width((ui.available_width() - 520.0).clamp(220.0, 520.0)));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.applied_where = self.where_text.clone();
                do_reload = true;
            }
            if icons::button(ui, Icon::Search, "Aplicar").clicked() {
                self.applied_where = self.where_text.clone();
                do_reload = true;
            }
            if !self.where_text.is_empty() && icons::button(ui, Icon::Cross, "").on_hover_text("Limpar filtro").clicked() {
                self.where_text.clear();
                self.applied_where.clear();
                do_reload = true;
            }
            ui.separator();
            if let Some(g) = self.grid.as_mut() {
                for (m, ic, t) in [(ViewMode::Grid, Icon::Grid, "Grade"), (ViewMode::Text, Icon::Text, "Texto"), (ViewMode::Record, Icon::Record, "Registro")] {
                    if icons::segment(ui, ic, t, g.mode == m).clicked() {
                        g.mode = m;
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            if icons::button(ui, Icon::Refresh, "Atualizar").on_hover_text("Recarrega as linhas do banco").clicked() {
                do_reload = true;
            }
            if icons::button(ui, Icon::Plan, "Contar").clicked() {
                do_count = true;
            }
            if icons::button(ui, Icon::Export, "Exportar").clicked() {
                export = true;
            }
            if let Some(t) = self.total {
                ui.label(RichText::new(format!("Total: {}", t)).strong());
            }
            if let Some(g) = &self.grid {
                if g.qr.has_more {
                    if icons::button(ui, Icon::Play, "Próxima página").clicked() {
                        do_fetch = Some(sh.store.prefs.max_rows);
                    }
                    if icons::button(ui, Icon::PlayAll, "Buscar tudo").clicked() {
                        do_fetch = Some(10_000_000);
                    }
                }
                ui.label(RichText::new(format!("{} linhas buscadas • {} ms", g.qr.rows.len(), g.qr.elapsed_ms)).color(pl.weak));
            }
            if self.job.is_some() {
                ui.spinner();
            }
        });
        let mut vp = sh.store.prefs.show_value_panel;
        if let Some(g) = self.grid.as_mut() {
            match editing::bar(ui, g, &mut vp) {
                Some(editing::BarEvent::Save) => do_save = true,
                Some(editing::BarEvent::Cancel) => cancel = true,
                None => {}
            }
        }
        sh.store.prefs.show_value_panel = vp;
        if let Some(e) = &self.error {
            egui::Frame::new().fill(pl.err.gamma_multiply(0.15)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.label(RichText::new(e).color(pl.err));
            });
        }
        if do_save && sh.conn_type(&self.conn).confirm_data_change() && !self.confirm_save {
            // DBPConnectionType.java:53,72,91 - test/production ask before saving data changes
            self.confirm_save = true;
            do_save = false;
        }
        if self.confirm_save {
            let env = sh.conn_type(&self.conn).label();
            let mut yes = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Ambiente de {}: confirmar a gravação de {} alteração(ões)?", env, pending)).color(pl.warn));
                yes = ui.button("Confirmar e salvar").clicked();
                if ui.button("Voltar").clicked() {
                    self.confirm_save = false;
                }
            });
            if yes {
                self.confirm_save = false;
                do_save = true;
            }
        }
        ui.add_space(2.0);
        if let Some(g) = self.grid.as_mut() {
            let nt = sh.store.prefs.null_text.clone();
            let acts = grid::show(ui, g, &nt, vp);
            for a in acts {
                match a {
                    GridAction::SortBy => do_reload = true,
                    GridAction::FilterBy(c, v) => {
                        let col = &g.qr.columns[c];
                        let cond = if kind == Kind::Mongo {
                            format!("{{{}: {}}}", serde_json::to_string(col).unwrap_or_default(), crate::drivers::mongo_value(&v, col == "_id"))
                        } else {
                            let qc = kind.quote(col);
                            match v {
                                None => format!("{} IS NULL", qc),
                                Some(v) => format!("{} = {}", qc, value_literal(&Some(v), self.meta.columns.iter().find(|m| m.name.eq_ignore_ascii_case(col)).map(|c| c.is_numeric()).unwrap_or(false))),
                            }
                        };
                        self.where_text = if kind == Kind::Mongo || self.where_text.trim().is_empty() { cond } else { format!("{} AND {}", self.where_text, cond) };
                        self.applied_where = self.where_text.clone();
                        do_reload = true;
                    }
                    GridAction::ViewValue(v) => view_val = Some(v),
                    GridAction::Changed => {}
                    GridAction::FetchMore => {
                        if self.job.is_none() {
                            do_fetch = Some(sh.store.prefs.max_rows);
                        }
                    }
                }
            }
        } else if self.job.is_none() && self.error.is_none() {
            ui.label(RichText::new("Sem dados").color(pl.weak));
        }
        if cancel {
            if let Some(g) = self.grid.as_mut() {
                g.revert();
            }
        }
        if let Some(v) = view_val {
            sh.req(Req::ViewValue(v));
        }
        if do_count {
            if let Some(s) = sh.session(&self.conn) {
                let w = self.applied_where.trim();
                let sql = if kind == Kind::Mongo {
                    format!("{}.countDocuments({})", self.src.qualified, if w.is_empty() { "{}" } else { w })
                } else {
                    let mut sql = format!("SELECT COUNT(*) FROM {}", self.src.qualified);
                    if !w.is_empty() {
                        sql.push_str(&format!(" WHERE {}", w));
                    }
                    sql
                };
                match s.lock().unwrap_or_else(|e| e.into_inner()).run(&sql, 0, 0) {
                    Ok(r) => self.total = r.rows.first().and_then(|r| r[0].as_ref()).and_then(|v| v.parse().ok()),
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        }
        if export {
            let sql = self.base_sql(sh);
            sh.req(Req::Export { conn: self.conn.clone(), sql, name: self.table.clone(), rows: None });
        }
        if let Some(n) = do_fetch {
            if let (Some(sess), Some(g)) = (sh.session(&self.conn), self.grid.as_ref()) {
                let sql = self.base_sql(sh);
                let off = g.qr.rows.len();
                self.job = Some(spawn_job(sess, vec![JobStmt { sql, limit: n, offset: off, tag: 1 }], sh.ctx.clone()));
            }
        }
        if do_save {
            self.save(sh);
        } else if do_reload {
            if pending > 0 {
                sh.status = "Salve ou cancele as alterações pendentes antes de atualizar".into();
            } else {
                self.reload(sh);
            }
        }
    }
}

// ---------------------------------------------------------------- ER diagram
pub struct ErBox {
    pub name: String,
    pub cols: Vec<(String, String, bool, bool)>, // name, type, pk, fk
    pub pos: egui::Vec2,
}

pub struct ErEdge {
    pub from: usize,
    pub from_col: usize,
    pub to: usize,
}

pub struct ErTab {
    pub conn: String,
    pub schema: String,
    pub boxes: Vec<ErBox>,
    pub edges: Vec<ErEdge>,
    pub pan: egui::Vec2,
    pub focus: Option<String>,
}

impl ErTab {
    pub fn title(&self, sh: &Shared) -> String {
        format!("{}{}", sh.conn_name(&self.conn), self.focus.as_ref().map(|f| format!(" — {}", f)).unwrap_or_default())
    }

    pub fn build(sh: &mut Shared, conn: &str, schema: &str, focus: Option<&str>) -> ErTab {
        let all: Vec<String> = sh.ensure_tables(conn, schema).into_iter().map(|t| t.0).collect();
        let metas: Vec<(String, TableMeta)> = all.iter().take(80).map(|t| (t.clone(), sh.ensure_tmeta(conn, schema, t))).collect();
        let chosen: Vec<&(String, TableMeta)> = if let Some(f) = focus {
            metas
                .iter()
                .filter(|(t, m)| {
                    t == f
                        || metas.iter().any(|(t2, m2)| t2 == f && m2.fks.iter().any(|fk| &fk.ref_table == t))
                        || m.fks.iter().any(|fk| fk.ref_table == f)
                })
                .collect()
        } else {
            metas.iter().collect()
        };
        let n = chosen.len().max(1);
        let per_row = ((n as f32).sqrt().ceil() as usize).max(1);
        let mut boxes = Vec::new();
        let mut col_y = vec![0.0f32; per_row];
        for (i, (t, m)) in chosen.iter().enumerate() {
            let c = i % per_row;
            let h = 28.0 + m.columns.len() as f32 * 18.0;
            let fkcols: Vec<&String> = m.fks.iter().flat_map(|f| f.columns.iter()).collect();
            boxes.push(ErBox {
                name: t.clone(),
                cols: m.columns.iter().map(|c| (c.name.clone(), c.typ.clone(), c.pk, fkcols.contains(&&c.name))).collect(),
                pos: egui::vec2(30.0 + c as f32 * 280.0, 30.0 + col_y[c]),
            });
            col_y[c] += h + 50.0;
        }
        let mut edges = Vec::new();
        for (i, (_, m)) in chosen.iter().enumerate() {
            for fk in &m.fks {
                if let Some(j) = boxes.iter().position(|b| b.name == fk.ref_table) {
                    let ci = boxes[i].cols.iter().position(|c| fk.columns.first() == Some(&c.0)).unwrap_or(0);
                    edges.push(ErEdge { from: i, from_col: ci, to: j });
                }
            }
        }
        ErTab { conn: conn.into(), schema: schema.into(), boxes, edges, pan: egui::vec2(0.0, 0.0), focus: focus.map(|s| s.to_string()) }
    }

    pub fn show(&mut self, ui: &mut Ui, sh: &mut Shared, tab_id: u64) {
        let (rect, bg) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if bg.dragged() {
            self.pan += bg.drag_delta();
        }
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals().clone();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
        // dotted grid
        let step = 24.0;
        let mut x = rect.left() + self.pan.x.rem_euclid(step);
        while x < rect.right() {
            let mut y = rect.top() + self.pan.y.rem_euclid(step);
            while y < rect.bottom() {
                painter.circle_filled(egui::pos2(x, y), 0.7, visuals.weak_text_color().gamma_multiply(0.5));
                y += step;
            }
            x += step;
        }
        let origin = rect.min + self.pan;
        let bw = 220.0;
        let rects: Vec<egui::Rect> = self.boxes.iter().map(|b| egui::Rect::from_min_size(origin + b.pos, egui::vec2(bw, 28.0 + b.cols.len() as f32 * 18.0))).collect();
        // edges first (below boxes)
        let line = egui::Stroke::new(1.4, Color32::from_rgb(90, 140, 200));
        for e in &self.edges {
            let a = rects[e.from];
            let b = rects[e.to];
            let ay = a.top() + 28.0 + e.from_col as f32 * 18.0 + 9.0;
            let by = b.top() + 14.0;
            let (ax, bx, dir) = if a.center().x < b.center().x { (a.right(), b.left(), 1.0) } else { (a.left(), b.right(), -1.0) };
            let (ax, bx) = if e.from == e.to { (a.right(), a.right()) } else { (ax, bx) };
            let mid = (ax + bx) / 2.0;
            let pts = vec![egui::pos2(ax, ay), egui::pos2(ax + dir * 14.0, ay), egui::pos2(mid.max(ax + dir * 14.0).min(bx - dir * 14.0).max(if dir > 0.0 { ax } else { bx - 14.0 }), by), egui::pos2(bx - dir * 14.0, by), egui::pos2(bx, by)];
            painter.add(egui::Shape::line(vec![pts[0], pts[1], egui::pos2(pts[1].x, by), pts[3], pts[4]], line));
            // arrow head
            painter.add(egui::Shape::convex_polygon(
                vec![egui::pos2(bx, by), egui::pos2(bx - dir * 8.0, by - 4.0), egui::pos2(bx - dir * 8.0, by + 4.0)],
                Color32::from_rgb(90, 140, 200),
                egui::Stroke::NONE,
            ));
        }
        let mut open: Option<String> = None;
        for (i, b) in self.boxes.iter_mut().enumerate() {
            let r = rects[i];
            let resp = ui.interact(r, egui::Id::new(("er_box", tab_id, i)), Sense::click_and_drag());
            if resp.dragged() {
                b.pos += resp.drag_delta();
            }
            if resp.double_clicked() {
                open = Some(b.name.clone());
            }
            let focus = self.focus.as_deref() == Some(b.name.as_str());
            painter.rect_filled(r, 4.0, visuals.panel_fill);
            painter.rect_stroke(r, 4.0, egui::Stroke::new(if focus { 2.0 } else { 1.0 }, if focus { Color32::from_rgb(220, 150, 40) } else { visuals.widgets.noninteractive.fg_stroke.color.gamma_multiply(0.6) }), egui::StrokeKind::Inside);
            let head = egui::Rect::from_min_size(r.min, egui::vec2(bw, 24.0));
            painter.rect_filled(head, 4.0, Color32::from_rgb(60, 110, 170));
            painter.text(head.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, &b.name, FontId::proportional(13.5), Color32::WHITE);
            for (k, (n, t, pk, fk)) in b.cols.iter().enumerate() {
                let y = r.top() + 28.0 + k as f32 * 18.0;
                let icon = if *pk { "🔑" } else if *fk { "↗" } else { "" };
                painter.text(egui::pos2(r.left() + 6.0, y + 9.0), egui::Align2::LEFT_CENTER, icon, FontId::proportional(11.0), visuals.text_color());
                painter.text(egui::pos2(r.left() + 24.0, y + 9.0), egui::Align2::LEFT_CENTER, n, FontId::proportional(12.5), visuals.text_color());
                painter.text(egui::pos2(r.right() - 6.0, y + 9.0), egui::Align2::RIGHT_CENTER, t, FontId::proportional(11.0), visuals.weak_text_color());
            }
        }
        if let Some(t) = open {
            let conn = self.conn.clone();
            let schema = self.schema.clone();
            sh.req(Req::OpenTable { conn, schema, table: t, view: false });
        }
        painter.text(rect.right_bottom() - egui::vec2(8.0, 8.0), egui::Align2::RIGHT_BOTTOM, "Arraste para mover • duplo clique abre a tabela", FontId::proportional(11.0), visuals.weak_text_color());
    }
}

/// End of a variable name: a quoted name or letters, digits, `_`, `$` and `.` (ScriptParameterRule.tryConsumeParameterName).
fn var_name_end(p: &str) -> Option<usize> {
    let mut chars = p.char_indices();
    let (_, first) = chars.next()?;
    if first == '"' || first == '`' {
        let close = p[1..].find(first)? + 1;
        return Some(close + 1);
    }
    let mut end = 0;
    for (i, c) in p.char_indices() {
        if c.is_alphanumeric() || c == '_' || c == '$' || c == '.' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        None
    } else {
        Some(end)
    }
}

fn unquote_name(n: &str) -> String {
    let b = n.as_bytes();
    if n.len() > 1 && (b[0] == b'"' || b[0] == b'`') {
        n[1..n.len() - 1].to_string()
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;
    use crate::db::{ConnConfig, Kind};
    use crate::store::Store;

    fn setup(name: &str) -> (Shared, String) {
        let p = std::env::temp_dir().join(format!("dbr-ctl-{}.sqlite", name));
        crate::sample::create(&p).unwrap();
        let mut cfg = ConnConfig::new(Kind::Sqlite);
        cfg.database = p.to_string_lossy().to_string();
        let id = cfg.id.clone();
        let mut sh = Shared::new(Store::memory(), false, egui::Context::default());
        sh.store.connections.push(cfg);
        sh.connect(&id).unwrap();
        (sh, id)
    }

    fn run(sh: &mut Shared, tab: &mut SqlTab) {
        tab.execute(sh, 1, true, false, false);
        for _ in 0..500 {
            tab.poll(sh);
            if tab.job.is_none() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the script did not finish");
    }

    #[test]
    fn set_echo_unset_run_in_order() {
        let (mut sh, id) = setup("order");
        let text = "@set n = 3\n@set dobro = ${n}\n@echo n=${n} dobro=${dobro}\nselect ${n} as x, ${dobro} as y;\n@unset n\n@echo depois: ${n}";
        let mut t = SqlTab::new(Some(id), text.into(), "t".into());
        run(&mut sh, &mut t);
        assert_eq!(t.error, None);
        assert_eq!(t.results[0].qr.rows[0], vec![Some("3".to_string()), Some("3".to_string())]);
        assert_eq!(t.output[0], "n=3 dobro=3");
        assert_eq!(t.output[1], "depois: ${n}", "a removed variable is no longer replaced");
        assert!(sh.vars.iter().all(|(k, _)| k != "n"));
    }

    #[test]
    fn set_syntax_errors_use_the_dbeaver_checks() {
        let (mut sh, id) = setup("syntax");
        for (text, want) in [("@set", "Falta o nome"), ("@set a 1", "Sintaxe de @set inválida"), ("@set a b = 1", "Caracteres inesperados"), ("@unset a b", "@unset inválido"), ("@foo x", "desconhecido")] {
            let mut t = SqlTab::new(Some(id.clone()), text.into(), "t".into());
            t.execute(&mut sh, 1, true, false, false);
            assert!(t.error.as_deref().unwrap_or("").contains(want), "{} -> {:?}", text, t.error);
        }
    }

    #[test]
    fn include_runs_the_other_script_and_detects_loops() {
        let (mut sh, id) = setup("include");
        let dir = std::env::temp_dir().join("dbr-include-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("base.sql"), "@set quem = 'mundo'\nselect 42 as resposta;").unwrap();
        std::fs::write(dir.join("a.sql"), "@include b.sql").unwrap();
        std::fs::write(dir.join("b.sql"), "@include a.sql").unwrap();
        let mut t = SqlTab::new(Some(id.clone()), "@include base.sql\n@echo oi ${quem}\nselect ${quem} as q".into(), "t".into());
        t.file = Some(dir.join("main.sql"));
        run(&mut sh, &mut t);
        assert_eq!(t.error, None);
        assert_eq!(t.results[0].qr.rows[0][0].as_deref(), Some("42"));
        assert_eq!(t.results[1].qr.rows[0][0].as_deref(), Some("mundo"));
        assert_eq!(t.output[0], "oi 'mundo'", "@set keeps the quotes as written");
        let mut t2 = SqlTab::new(Some(id.clone()), "@include a.sql".into(), "t".into());
        t2.file = Some(dir.join("main.sql"));
        t2.execute(&mut sh, 1, true, false, false);
        assert!(t2.error.as_deref().unwrap_or("").contains("inclusão recursiva"), "{:?}", t2.error);
        let mut t3 = SqlTab::new(Some(id), "@include nao_existe.sql\n".into(), "t".into());
        t3.execute(&mut sh, 1, true, false, false);
        assert!(t3.error.as_deref().unwrap_or("").contains("não encontrado"), "{:?}", t3.error);
    }

    #[test]
    fn export_opens_the_export_window_for_the_next_query() {
        let (mut sh, id) = setup("export");
        let text = "@export {\"type\": \"csv\", \"consumer\": {\"outputFolder\": \"/tmp/x\", \"outputFilePattern\": \"clientes_${timestamp}\"}, \"processor\": {\"delimiter\": \";\", \"header\": \"none\"}}\nselect * from customers";
        let mut t = SqlTab::new(Some(id.clone()), text.into(), "t".into());
        t.execute(&mut sh, 1, true, false, false);
        assert_eq!(t.error, None);
        assert!(t.job.is_none(), "the query after @export is not run");
        match sh.requests.pop() {
            Some(Req::ExportWith { sql, opts, dir, file, .. }) => {
                assert_eq!(sql, "select * from customers");
                assert_eq!(opts.format, crate::export::Format::Csv);
                assert_eq!(opts.delimiter, ";");
                assert!(!opts.header, "header \"none\"");
                assert_eq!(dir, "/tmp/x");
                assert!(file.starts_with("clientes_2"), "{}", file);
            }
            _ => panic!("no export request"),
        }
        for (text, want) in [("@export {\"consumer\": {}}\nselect 1", "obrigatório"), ("@export {\"type\": \"xyz\"}\nselect 1", "Não existe processador"), ("@export nao-e-json\nselect 1", "Sintaxe inválida")] {
            let mut t = SqlTab::new(Some(id.clone()), text.into(), "t".into());
            t.execute(&mut sh, 1, true, false, false);
            assert!(t.error.as_deref().unwrap_or("").contains(want), "{} -> {:?}", text, t.error);
        }
    }
}
