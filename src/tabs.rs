//! Editor tabs: SQL editor, table editor (properties / data / DDL / diagram) and ER diagram.
use crate::shared::*;
use crate::db::*;
use crate::editing;
use crate::grid::{self, EditSource, GridAction, GridState, RowState, ViewMode};
use crate::icons::{self, Icon};
use crate::theme::{pal, SqlCat, SqlPalette};
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
pub fn closed(sh: &Shared, conn: Option<&str>) -> &'static str {
    match conn {
        Some(c) if !sh.is_connected(c) => " (desc.)",
        _ => "",
    }
}

pub struct Tab {
    pub id: u64,
    pub kind: TabKind,
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
    /// F4: open the object under the cursor or selected
    OpenObject,
}

/// Page Up/Down from char `from`: one visible page (`view_h`, whole rows) up or down, keeping the column
/// (`h_pos`, else the cursor's x). Before the first row it goes to the start, past the last one to the end.
/// Returns the new char index, the column x used and how far (in points) the cursor moved.
pub fn page_target(galley: &egui::Galley, from: usize, view_h: f32, down: bool, h_pos: Option<f32>) -> (usize, f32, f32) {
    let r = galley.pos_from_cursor(CCursor::new(from));
    let row_h = r.height().max(1.0);
    let page = (view_h / row_h).floor().max(1.0) * row_h;
    let x = h_pos.unwrap_or(r.left());
    let y = r.center().y + if down { page } else { -page };
    let to = if y < 0.0 {
        0
    } else if y > galley.rect.bottom() {
        galley.text().chars().count()
    } else {
        galley.cursor_from_pos(egui::vec2(x, y)).index.into()
    };
    let moved = galley.pos_from_cursor(CCursor::new(to)).top() - r.top();
    (to, x, moved)
}

/// Id of the text editor of an SQL tab.
pub fn editor_id(tab_id: u64) -> egui::Id {
    egui::Id::new(("sql_edit", tab_id))
}

// ---------------------------------------------------------------- highlighting
/// Known table/view and column names (lower case) used to colour identifiers.
#[derive(Default, Debug)]
pub struct HlNames {
    pub tables: std::collections::HashSet<String>,
    pub columns: std::collections::HashSet<String>,
}

impl HlNames {
    fn knows(&self, name: &str) -> bool {
        let l = name.to_lowercase();
        self.tables.contains(&l) || self.columns.contains(&l)
    }
}

/// Colour of an identifier: after a `.` it is a column first (`alias.col`), otherwise a table first.
fn ident_cat(word: &str, after_dot: bool, names: Option<&HlNames>) -> SqlCat {
    let Some(n) = names else { return SqlCat::Text };
    let l = word.to_lowercase();
    let (t, c) = (n.tables.contains(&l), n.columns.contains(&l));
    match (after_dot, t, c) {
        (true, _, true) | (false, false, true) => SqlCat::Column,
        (_, true, _) => SqlCat::Table,
        _ => SqlCat::Text,
    }
}

/// Text inside `"..."` / `` `...` `` without the quotes, with doubled quotes undone.
fn unquote(t: &str) -> String {
    let q = t.chars().next().unwrap_or('"');
    let inner = t.strip_prefix(q).unwrap_or(t);
    let inner = inner.strip_suffix(q).unwrap_or(inner);
    let dq: String = [q, q].iter().collect();
    inner.replace(&dq, &q.to_string())
}

pub fn highlight(text: &str, pal: &SqlPalette, size: f32, wrap: f32, names: Option<&HlNames>) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let font = FontId::monospace(size);
    let fmt = |cat: SqlCat| TextFormat::simple(font.clone(), pal.get(cat));
    let mut prev_dot = false;
    let toks = sqltext::tokenize(text);
    // column aliases are coloured as columns, with or without metadata
    let aliases = sqltext::column_aliases(text, &toks);
    for (k, a, b) in toks {
        let t = &text[a..b];
        match k {
            Tok::Str | Tok::QuotedIdent => {
                let base = match k {
                    Tok::Str => SqlCat::Str,
                    _ if names.is_some_and(|n| n.knows(&unquote(t))) || aliases.contains(&unquote(t).to_lowercase()) => SqlCat::QuotedKnown,
                    _ => SqlCat::Quoted,
                };
                for (var, x, y) in sqltext::str_var_spans(text, a, b) {
                    let mut f = fmt(if var { SqlCat::Variable } else { base });
                    if var {
                        f.background = pal.get(SqlCat::VariableBg);
                    }
                    job.append(&text[x..y], 0.0, f);
                }
            }
            _ => {
                let cat = match k {
                    Tok::Keyword => SqlCat::Keyword,
                    Tok::Function => SqlCat::Function,
                    Tok::Number => SqlCat::Number,
                    Tok::Comment => SqlCat::Comment,
                    Tok::Variable => SqlCat::Variable,
                    // `::type` casts are not parameters
                    Tok::Param if !(a > 0 && text.as_bytes()[a - 1] == b':') => SqlCat::Variable,
                    Tok::Ident => match ident_cat(t, prev_dot, names) {
                        SqlCat::Text if !prev_dot && aliases.contains(&t.to_lowercase()) => SqlCat::Column,
                        c => c,
                    },
                    _ => SqlCat::Text,
                };
                let mut f = fmt(cat);
                f.italics = k == Tok::Comment;
                job.append(t, 0.0, f);
            }
        }
        if k != Tok::Space {
            prev_dot = t == ".";
        }
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
    /// autocomplete waiting for metadata loaded in the background: (generation asked at, cursor)
    pub ac_wait: Option<(u64, usize)>,
    /// vertical scroll of the editor; Some in `set_scroll` restores it on the next frame
    pub scroll_y: f32,
    pub set_scroll: Option<f32>,
    /// give the keyboard to the editor on the next frame (a tab restored with its cursor)
    pub focus_editor: bool,
    /// table/column names for the highlighting, rebuilt only when the metadata or the cited tables change
    pub hl: Option<HlCache>,
    /// object under the right click of the open editor menu (resolved once, from the cached metadata)
    pub ctx_obj: Option<ObjRef>,
}

pub struct HlCache {
    conn: String,
    gen: u64,
    text_hash: u64,
    refs: Vec<String>,
    names: std::sync::Arc<HlNames>,
    /// metadata already requested for this connection: a failed load is not asked again on every frame
    asked: std::collections::HashSet<String>,
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
            ac_wait: None,
            scroll_y: 0.0,
            set_scroll: None,
            focus_editor: false,
            hl: None,
            ctx_obj: None,
        }
    }

    /// Names for colouring tables and columns. Only cached metadata is used; what is missing is requested in the
    /// background (once per tab and connection) and shows up when `meta_gen` changes. Never waits for the database.
    pub fn hl_names(&mut self, sh: &mut Shared) -> Option<std::sync::Arc<HlNames>> {
        use std::hash::{Hash, Hasher};
        if !sh.store.prefs.hl_metadata {
            return None;
        }
        let conn = self.conn.clone().filter(|c| sh.is_connected(c))?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        let text_hash = h.finish();
        let same_conn = self.hl.as_ref().is_some_and(|c| c.conn == conn);
        if let Some(c) = self.hl.as_ref().filter(|c| same_conn && c.gen == sh.meta_gen && c.text_hash == text_hash) {
            return Some(c.names.clone());
        }
        let mut refs: Vec<String> = sqltext::referenced_tables(&self.text).into_iter().map(|(t, _)| t.to_lowercase()).collect();
        refs.sort();
        refs.dedup();
        if let Some(c) = self.hl.as_mut().filter(|c| same_conn && c.gen == sh.meta_gen && c.refs == refs) {
            c.text_hash = text_hash;
            return Some(c.names.clone());
        }
        let mut asked = match self.hl.take() {
            Some(c) if same_conn => c.asked,
            _ => Default::default(),
        };
        let gen = sh.meta_gen;
        let mut names = HlNames::default();
        if let Some(schema) = sh.cached_default_schema(&conn) {
            if !sh.tables.contains_key(&(conn.clone(), schema.clone())) && asked.insert(format!("t|{schema}")) {
                sh.want_tables(&conn, &schema);
            }
            let tables = sh.tables.get(&(conn.clone(), schema.clone())).cloned().unwrap_or_default();
            for (t, _) in &tables {
                names.tables.insert(t.to_lowercase());
            }
            for (t, _) in tables.iter().filter(|(t, _)| refs.contains(&t.to_lowercase())) {
                let key = (conn.clone(), schema.clone(), t.clone());
                if !sh.tmeta.contains_key(&key) && asked.insert(format!("m|{schema}|{t}")) {
                    sh.want_tmeta(&conn, &schema, t);
                }
                if let Some(m) = sh.tmeta.get(&key) {
                    names.columns.extend(m.columns.iter().map(|c| c.name.to_lowercase()));
                }
            }
        }
        let names = std::sync::Arc::new(names);
        self.hl = Some(HlCache { conn, gen, text_hash, refs, names: names.clone(), asked });
        Some(names)
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
        // values asked for before running: `:name` parameters and `${name}` variables not defined by @set
        // (variables appear in the list as "${name}")
        let mut names = Vec::new();
        for s in &real {
            if sh.store.prefs.vars_enabled {
                for n in sqltext::find_vars(s) {
                    let n = format!("${{{}}}", n);
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            if sh.store.prefs.params_enabled {
                for n in sqltext::find_params(s) {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
        }
        {
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
                        g.row_state.extend(std::iter::repeat_n(RowState::Clean, added));
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
                        // the previous execution's results are freed before the new ones are kept
                        crate::shared::release_memory();
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
                if sh.store.prefs.ac_insert_single {
                    if let Some(ac) = &self.ac {
                        if ac.items.len() == 1 {
                            let (item, start) = (ac.items[0].clone(), ac.start_char);
                            self.insert_proposal(&item, start);
                        }
                    }
                }
            }
            SqlCmd::Save => self.save_file(),
            SqlCmd::OpenObject => self.open_object(sh),
        }
    }

    /// Right click at char `at`: inside the selection keeps it, elsewhere moves the cursor there (returned).
    /// Resolves the object of the menu from the cached metadata only.
    pub fn right_click_at(&mut self, sh: &mut Shared, at: usize) -> Option<usize> {
        let (lo, hi) = (self.cursor.0.min(self.cursor.1), self.cursor.0.max(self.cursor.1));
        let moved = if lo != hi && (lo..=hi).contains(&at) {
            None
        } else {
            self.cursor = (at, at);
            Some(at)
        };
        self.ctx_obj = match (self.conn.clone(), self.object_name_at_cursor()) {
            (Some(c), Some((schema, name))) if sh.is_connected(&c) => sh.find_object(&c, schema.as_deref(), &name, false),
            _ => None,
        };
        moved
    }

    fn editor_menu(&mut self, ui: &mut Ui, sh: &mut Shared, id: egui::Id, state: &egui::text_edit::TextEditState) {
        let (a, b) = self.sel_range_bytes();
        let has_sel = a != b;
        let item = |ui: &mut Ui, enabled: bool, label: &str, keys: &str| ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(keys)).clicked();
        let mut cmd = None;
        if item(ui, has_sel, "Recortar", "Ctrl+X") {
            ui.ctx().copy_text(self.text[a..b].to_string());
            let start = self.text[..a].chars().count();
            self.text.replace_range(a..b, "");
            self.set_cursor = Some(start);
            self.dirty = true;
            ui.close();
        }
        if item(ui, has_sel, "Copiar", "Ctrl+C") {
            ui.ctx().copy_text(self.text[a..b].to_string());
            ui.close();
        }
        if item(ui, true, "Colar", "Ctrl+V") {
            // the clipboard is read by the platform: it comes back as a paste event into the focused editor
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            ui.close();
        }
        if item(ui, !self.text.is_empty(), "Selecionar tudo", "Ctrl+A") {
            let mut st = state.clone();
            let n = self.text.chars().count();
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(n))));
            st.store(ui.ctx(), id);
            self.cursor = (n, 0);
            ui.close();
        }
        ui.separator();
        for (label, keys, c) in [
            ("Executar instrução", "Ctrl+Enter", SqlCmd::Execute),
            ("Executar script", "Alt+X", SqlCmd::Script),
            ("Formatar", "Ctrl+Shift+F", SqlCmd::Format),
            ("Comentar / descomentar", "Ctrl+/", SqlCmd::Comment),
            ("Abrir objeto", "F4", SqlCmd::OpenObject),
        ] {
            if item(ui, true, label, keys) {
                cmd = Some(c);
                ui.close();
            }
        }
        if let Some(o) = self.ctx_obj.clone() {
            ui.separator();
            ui.label(RichText::new(format!("{} ({})", o.name, o.kind_label())).strong());
            crate::objmenu::object_menu(ui, sh, &o);
        }
        if cmd.is_some() {
            self.cmd = cmd;
        }
        // the editor keeps the keyboard after a menu action (paste needs it)
        self.focus_editor = true;
    }

    /// Name under the cursor, or the whole selection when it is a name.
    pub fn object_name_at_cursor(&self) -> Option<sqltext::ObjName> {
        let (a, b) = self.sel_range_bytes();
        if a != b {
            sqltext::ident_in(&self.text[a..b])
        } else {
            sqltext::ident_at(&self.text, self.cursor.0)
        }
    }

    /// F4: opens the table tab, or the source code of a view, procedure, function...
    fn open_object(&mut self, sh: &mut Shared) {
        let Some((schema, name)) = self.object_name_at_cursor() else {
            sh.status = "F4: nenhum nome de objeto sob o cursor".into();
            return;
        };
        let Some(conn) = self.conn.clone().filter(|c| sh.session(c).is_some()) else {
            sh.status = format!("F4: a conexão do script está fechada; conecte-a para abrir \"{}\"", name);
            return;
        };
        match sh.find_object(&conn, schema.as_deref(), &name, true) {
            Some(o) => {
                sh.status = format!("Abrindo {} {}", o.kind_label(), o.name);
                sh.req(o.open_req());
            }
            None => sh.status = format!("Objeto não encontrado: {}", name),
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
        self.ac_wait = None;
        self.dirty = true;
    }

    fn start_complete(&mut self, sh: &mut Shared) {
        let cur = byte_of_char(&self.text, self.cursor.0);
        let (ac, waiting) = complete(sh, self.conn.as_deref(), &self.text, cur);
        // keep the selection when the list is refreshed with metadata that just arrived
        let sel = self.ac.as_ref().filter(|_| self.ac_wait.is_some()).map(|a| a.sel).unwrap_or(0);
        self.ac = ac.map(|mut a| {
            a.sel = sel.min(a.items.len().saturating_sub(1));
            a
        });
        self.ac_wait = waiting.then_some((sh.meta_gen, self.cursor.0));
    }

    pub fn show(&mut self, ui: &mut Ui, sh: &mut Shared, tab_id: u64, dark: bool) {
        self.poll(sh);
        if self.job.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(cmd) = self.cmd.take() {
            self.apply_cmd(sh, tab_id, cmd);
        }
        self.watch_job(ui, sh);
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
        if let Some((gen, at)) = self.ac_wait {
            if self.cursor.0 != at || self.cursor.0 != self.cursor.1 {
                self.ac_wait = None;
            } else if sh.meta_gen != gen {
                self.start_complete(sh);
            }
        }
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
                // a page of the list (it shows about ten items)
                if i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
                    ac.sel = (ac.sel + 10).min(n - 1);
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
                    ac.sel = ac.sel.saturating_sub(10);
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                    accept = ac.items.get(ac.sel).cloned();
                }
            });
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.ac = None;
        self.ac_wait = None;
            }
        }
        if let Some(item) = accept {
            if let Some(ac) = self.ac.take() {
                self.insert_proposal(&item, ac.start_char);
            }
        }

        // auto-close quotes and brackets (SQLEDITOR_CLOSE_SINGLE_QUOTES / DOUBLE_QUOTES / BRACKETS = true)
        let mut pair_cursor: Option<usize> = None;
        if sh.store.prefs.auto_close_pairs && self.cursor.0 == self.cursor.1 && self.ac.is_none() {
            let cur_b = byte_of_char(&self.text, self.cursor.0);
            let next = self.text[cur_b..].chars().next();
            let prev = self.text[..cur_b].chars().next_back();
            let mut skip = false;
            let mut pair = false;
            // a pair is closed only before a blank, a closing bracket or a separator: typing "(" in front of a word
            // (to wrap it) inserts the bracket alone
            let pair_ok = next.map(|c| c.is_whitespace() || ")]};,".contains(c)).unwrap_or(true);
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
                            '(' if pair_ok => {
                                *tx = "()".into();
                                pair = true;
                            }
                            '[' if pair_ok => {
                                *tx = "[]".into();
                                pair = true;
                            }
                            '{' if pair_ok => {
                                *tx = "{}".into();
                                pair = true;
                            }
                            '\'' | '"' if pair_ok && !prev.map(|p| p.is_alphanumeric()).unwrap_or(false) => {
                                *tx = format!("{}{}", ch, ch);
                                pair = true;
                            }
                            _ => {}
                        }
                        break;
                    }
                }
            });
            if skip {
                // nothing is inserted: the cursor just steps over the closing character
                self.set_cursor = Some(self.cursor.0 + 1);
            }
            if pair {
                // the editor inserts both characters at the cursor; it goes back between them afterwards
                pair_cursor = Some(self.cursor.0 + 1);
            }
        }

        let sql_pal = sh.store.prefs.sql_palette(dark);
        let hl_names = self.hl_names(sh);
        let editor_h = ui.available_height();
        let mut area = egui::ScrollArea::vertical().id_salt(("sql_scroll", tab_id)).auto_shrink([false, false]);
        if let Some(y) = self.set_scroll.take() {
            area = area.vertical_scroll_offset(y);
        }
        // Page Up / Page Down (Shift extends the selection): the text editor does not handle them itself
        let mut page: Option<(bool, bool)> = None;
        if self.ac.is_none() && ui.memory(|m| m.has_focus(editor_id(tab_id))) {
            ui.input_mut(|i| {
                for (key, down) in [(egui::Key::PageDown, true), (egui::Key::PageUp, false)] {
                    // Shift first: a plain consume_key also matches when Shift is held
                    if i.consume_key(egui::Modifiers::SHIFT, key) {
                        page = Some((down, true));
                    } else if i.consume_key(egui::Modifiers::NONE, key) {
                        page = Some((down, false));
                    }
                }
            });
        }
        let scrolled = area.show(ui, |ui| {
            let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                let job = highlight(text.as_str(), &sql_pal, size, wrap, hl_names.as_deref());
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let id = editor_id(tab_id);
            if let Some(pos) = self.set_cursor.take() {
                let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
                st.store(ui.ctx(), id);
            }
            if std::mem::take(&mut self.focus_editor) {
                ui.memory_mut(|m| m.request_focus(id));
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
                if sh.store.prefs.ac_auto_popup && (after_dot || word_len >= 2) {
                    self.cursor = (out.cursor_range.map(|r| r.primary.index.into()).unwrap_or(self.cursor.0), out.cursor_range.map(|r| r.secondary.index.into()).unwrap_or(self.cursor.1));
                    self.start_complete(sh);
                } else {
                    self.ac = None;
        self.ac_wait = None;
                }
            }
            if let Some(r) = out.cursor_range {
                self.cursor = (r.primary.index.into(), r.secondary.index.into());
            }
            if let Some((down, shift)) = page {
                let h_pos = out.state.cursor.char_range().and_then(|r| r.h_pos);
                let (to, x, moved) = page_target(&out.galley, self.cursor.0, editor_h, down, h_pos);
                let anchor = if shift { self.cursor.1 } else { to };
                let mut st = out.state.clone();
                st.cursor.set_char_range(Some(CCursorRange { primary: CCursor::new(to), secondary: CCursor::new(anchor), h_pos: Some(x) }));
                st.store(ui.ctx(), id);
                self.cursor = (to, anchor);
                // the view moves by the same distance, so the cursor stays at the same place on the screen
                self.set_scroll = Some((self.scroll_y + moved).max(0.0));
                ui.ctx().request_repaint();
            }
            if let Some(pos) = pair_cursor.filter(|_| out.response.changed()) {
                let mut st = out.state.clone();
                st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos))));
                st.store(ui.ctx(), id);
                self.cursor = (pos, pos);
            }
            // context menu: a right click outside the selection moves the cursor first
            if out.response.secondary_clicked() {
                if let Some(p) = ui.ctx().pointer_interact_pos() {
                    let at: usize = out.galley.cursor_from_pos(p - out.galley_pos).index.into();
                    if let Some(c) = self.right_click_at(sh, at) {
                        let mut st = out.state.clone();
                        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(c))));
                        st.store(ui.ctx(), id);
                    }
                }
            }
            out.response.context_menu(|ui| self.editor_menu(ui, sh, id, &out.state));
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
        self.ac_wait = None;
                        self.dirty = true;
                    }
                }
            }
        });
        self.scroll_y = scrolled.state.offset.y;
    }

    /// Asks the running job to stop. When the driver cannot interrupt the statement the tab stops waiting for it.
    pub fn cancel_job(&mut self, sh: &mut Shared, reason: &str) {
        let Some(job) = &self.job else { return };
        match job.cancel(reason) {
            Ok(true) => {}
            Ok(false) => {
                // the driver cannot interrupt: forget the job, the server may still finish it
                self.job = None;
                self.error = Some(crate::shared::cancelled_message(reason));
                self.log("Consulta cancelada (este banco não permite interromper no servidor; a execução pode continuar até terminar)");
                sh.status = "Consulta cancelada".into();
            }
            Err(e) => {
                self.log(format!("Não foi possível cancelar: {}", e));
                sh.status = format!("Não foi possível cancelar: {}", e);
            }
        }
    }

    /// Cancels the job when it passes the time limit from the preferences, or when Esc is pressed.
    fn watch_job(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let Some(job) = &self.job else { return };
        if job.cancel_requested() {
            return;
        }
        let limit = sh.store.prefs.query_timeout_secs;
        let over = limit > 0 && job.started.elapsed().as_secs() >= limit as u64;
        let esc = self.ac.is_none() && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if over {
            self.cancel_job(sh, &format!("limite de {} s atingido", limit));
        } else if esc {
            self.cancel_job(sh, "");
        }
    }

    fn show_results(&mut self, ui: &mut Ui, sh: &mut Shared) {
        let pl = pal(ui.visuals().dark_mode);
        let mut cancel_clicked = false;
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
                crate::shared::release_memory();
                self.sel = if self.results.is_empty() { ResSel::Stats } else { ResSel::Data(self.results.len() - 1) };
            }
            if icons::segment(ui, Icon::Plan, "Estatísticas", matches!(self.sel, ResSel::Stats)).clicked() {
                self.sel = ResSel::Stats;
            }
            if icons::segment(ui, Icon::Text, "Saída", matches!(self.sel, ResSel::Output)).clicked() {
                self.sel = ResSel::Output;
            }
            if let Some(j) = &self.job {
                ui.spinner();
                let secs = j.started.elapsed().as_secs_f32();
                let txt = if j.cancel_requested() { format!("cancelando… {:.0} s", secs) } else { format!("executando… {:.0} s", secs) };
                ui.label(RichText::new(txt).color(pl.weak));
                if !j.cancel_requested() && icons::button(ui, Icon::Cross, "Cancelar").on_hover_text("Interrompe a consulta no servidor (Esc)").clicked() {
                    cancel_clicked = true;
                }
            }
        });
        if cancel_clicked {
            self.cancel_job(sh, "");
        }
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
/// Proposals for the word before `cursor` (a byte offset). Only cached metadata is used: what is missing is
/// requested in the background and the second value is true, so that the caller asks again when it arrives.
/// Typing never waits for the database.
pub fn complete(sh: &mut Shared, conn: Option<&str>, text: &str, cursor: usize) -> (Option<Ac>, bool) {
    let start = text[..cursor].rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == '"')).map(|i| i + text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1)).unwrap_or(0);
    let word = &text[start..cursor];
    let (qual, part) = match word.rsplit_once('.') {
        Some((q, p)) => (Some(q.trim_matches('"').to_string()), p.to_string()),
        None => (None, word.to_string()),
    };
    let part_l = part.trim_matches('"').to_lowercase();
    let part_start = start + word.len() - part.len();
    let mut waiting = false;
    // (name, prefix match); the set keeps names unique without scanning the list for each one
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut hits: Vec<(String, bool)> = Vec::new();
    let mut push = |s: &str| {
        let l = s.to_lowercase();
        let prefix = l.starts_with(&part_l);
        let hit = prefix || (crate::constants::AUTOCOMPLETE_MATCH_CONTAINS && l.contains(&part_l));
        if hit && seen.insert(s.to_string()) {
            hits.push((s.to_string(), prefix));
        }
    };
    // statement around the cursor to resolve aliases
    let delim = sh.store.prefs.statement_delimiter.clone();
    let stmt = sqltext::statement_at(text, cursor.saturating_sub(1), &delim).map(|(a, b)| text[a..b].to_string()).unwrap_or_default();
    let refs = sqltext::referenced_tables(&stmt);
    if let Some(conn) = conn.filter(|c| sh.is_connected(c)) {
        match sh.cached_default_schema(conn) {
            None => waiting = true,
            Some(schema) => {
                let have_tables = sh.want_tables(conn, &schema);
                waiting |= !have_tables;
                let key = (conn.to_string(), schema.clone());
                // tables named in the statement that really exist (not the half-typed word under the cursor)
                let real_refs: Vec<(String, String)> = match sh.tables.get(&key) {
                    Some(tables) => refs.iter().filter_map(|(t, a)| tables.iter().find(|(x, _)| x.eq_ignore_ascii_case(t)).map(|(x, _)| (x.clone(), a.clone()))).collect(),
                    None => vec![],
                };
                let columns_of = |sh: &mut Shared, t: &str, waiting: &mut bool, push: &mut dyn FnMut(&str)| {
                    if sh.want_tmeta(conn, &schema, t) {
                        if let Some(m) = sh.tmeta.get(&(conn.to_string(), schema.clone(), t.to_string())) {
                            for c in &m.columns {
                                push(&c.name);
                            }
                        }
                    } else {
                        *waiting = true;
                    }
                };
                if let Some(q) = &qual {
                    let ql = q.to_lowercase();
                    let target = real_refs
                        .iter()
                        .find(|(t, a)| a.to_lowercase() == ql || t.to_lowercase() == ql)
                        .map(|(t, _)| t.clone())
                        .or_else(|| sh.tables.get(&key).and_then(|ts| ts.iter().find(|(t, _)| t.to_lowercase() == ql).map(|(t, _)| t.clone())));
                    if let Some(t) = target {
                        columns_of(sh, &t, &mut waiting, &mut push);
                    } else if let Some(sch) = sh.schemas.get(conn).and_then(|v| v.iter().find(|s| s.to_lowercase() == ql).cloned()) {
                        // schema.table: the tables of that schema
                        if sh.want_tables(conn, &sch) {
                            for (t, _) in sh.tables.get(&(conn.to_string(), sch)).into_iter().flatten() {
                                push(t);
                            }
                        } else {
                            waiting = true;
                        }
                    }
                } else {
                    for (t, _) in &real_refs {
                        columns_of(sh, t, &mut waiting, &mut push);
                    }
                    // aliases of tables that exist (a half-typed name is not an alias)
                    for (_, a) in &real_refs {
                        push(a);
                    }
                    for (t, _) in sh.tables.get(&key).into_iter().flatten() {
                        push(t);
                    }
                }
            }
        }
    }
    if qual.is_none() {
        for k in sqltext::KEYWORDS {
            push(k);
        }
    }
    // names that start with the typed text come first
    if crate::constants::AUTOCOMPLETE_SORT_ALPHA {
        hits.sort_by_cached_key(|(s, prefix)| (!*prefix, s.to_lowercase()));
    } else {
        hits.sort_by_key(|(_, prefix)| !*prefix);
    }
    hits.truncate(60);
    let items: Vec<String> = hits.into_iter().map(|(s, _)| s).collect();
    if items.is_empty() || (items.len() == 1 && items[0].eq_ignore_ascii_case(part.trim_matches('"')) && !part.is_empty()) {
        return (None, waiting);
    }
    (Some(Ac { items, sel: 0, start_char: char_of_byte(text, part_start) }), waiting)
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
    /// restored from the last session: the metadata is read when the tab is first shown, not at startup
    pub meta_pending: bool,
    /// sort and view mode from the last session, applied to the first grid that loads
    pub restore_view: Option<(Option<(usize, bool)>, ViewMode)>,
}

impl TableTab {
    pub fn new(sh: &mut Shared, conn: &str, schema: &str, table: &str, view: bool) -> TableTab {
        let mut t = TableTab::restored(sh, conn, schema, table, view);
        t.load_meta(sh);
        t
    }

    /// A tab that does not touch the connection: the metadata and the data load when it is shown.
    pub fn restored(sh: &Shared, conn: &str, schema: &str, table: &str, view: bool) -> TableTab {
        let meta = TableMeta::default();
        let src = EditSource { meta: meta.clone(), qualified: sh.qualified(conn, schema, table), kind: sh.kind(conn) };
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
            meta_pending: true,
            restore_view: None,
        }
    }

    fn load_meta(&mut self, sh: &mut Shared) {
        let (conn, schema, table) = (self.conn.clone(), self.schema.clone(), self.table.clone());
        let mut meta = sh.ensure_tmeta(&conn, &schema, &table);
        if self.view {
            if let Some(sess) = sh.session(&conn) {
                if let Ok(def) = sess.lock().unwrap_or_else(|e| e.into_inner()).object_source("view", &schema, &table, "") {
                    meta.ddl = def;
                }
            }
        }
        self.src = EditSource { meta: meta.clone(), qualified: sh.qualified(&conn, &schema, &table), kind: sh.kind(&conn) };
        self.meta = meta;
        self.meta_pending = false;
        if let Some(g) = self.grid.as_mut() {
            // a grid that loaded first must edit with the real keys
            g.edit = Some(self.src.clone());
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
                                g.row_state.extend(std::iter::repeat_n(RowState::Clean, n));
                                g.dirty = true;
                            }
                        } else {
                            let restored = self.restore_view.take();
                            let sort = self.grid.as_ref().map(|g| g.sort).unwrap_or_else(|| restored.and_then(|r| r.0));
                            let mode = self.grid.as_ref().map(|g| g.mode).or(restored.map(|r| r.1)).unwrap_or(ViewMode::Grid);
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
                            crate::shared::release_memory();
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
        if self.meta_pending && self.error.is_none() && self.job.is_none() {
            // a tab restored from the last session connects only now that it is shown
            match sh.connect(&self.conn) {
                Ok(_) => self.load_meta(sh),
                Err(e) => self.error = Some(e),
            }
        }
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
    /// restored from the last session: the diagram is built when the tab is first shown
    pub pending: bool,
    pub error: Option<String>,
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
        ErTab { conn: conn.into(), schema: schema.into(), boxes, edges, pan: egui::vec2(0.0, 0.0), focus: focus.map(|s| s.to_string()), pending: false, error: None }
    }

    /// A diagram tab that does not touch the connection until it is shown.
    pub fn restored(conn: &str, schema: &str, focus: Option<&str>) -> ErTab {
        ErTab { conn: conn.into(), schema: schema.into(), boxes: vec![], edges: vec![], pan: egui::vec2(0.0, 0.0), focus: focus.map(|s| s.to_string()), pending: true, error: None }
    }

    pub fn show(&mut self, ui: &mut Ui, sh: &mut Shared, tab_id: u64) {
        if self.pending {
            match sh.connect(&self.conn) {
                Ok(_) => *self = ErTab::build(sh, &self.conn.clone(), &self.schema.clone(), self.focus.clone().as_deref()),
                Err(e) => {
                    self.pending = false;
                    self.error = Some(e);
                }
            }
        }
        if let Some(e) = self.error.clone() {
            ui.colored_label(egui::Color32::from_rgb(210, 60, 60), format!("Não foi possível carregar o diagrama: {}", e));
            if ui.button("Tentar de novo").clicked() {
                self.error = None;
                self.pending = true;
            }
            return;
        }
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
            let pts = [egui::pos2(ax, ay), egui::pos2(ax + dir * 14.0, ay), egui::pos2(mid.max(ax + dir * 14.0).min(bx - dir * 14.0).max(if dir > 0.0 { ax } else { bx - 14.0 }), by), egui::pos2(bx - dir * 14.0, by), egui::pos2(bx, by)];
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

#[cfg(test)]
mod editor_tests {
    use super::*;
    use crate::db::{ConnConfig, Kind};
    use crate::store::Store;
    use egui::{Event, Pos2, RawInput, Rect};

    fn sqlite() -> (Shared, String) {
        let p = std::env::temp_dir().join(format!("dbr-editor-{}.sqlite", crate::store::new_project_id()));
        crate::sample::create(&p).unwrap();
        let mut cfg = ConnConfig::new(Kind::Sqlite);
        cfg.database = p.to_string_lossy().to_string();
        let id = cfg.id.clone();
        let mut sh = Shared::new(Store::memory(), false, egui::Context::default());
        sh.store.connections.push(cfg);
        sh.connect(&id).unwrap();
        (sh, id)
    }

    fn wait_meta(sh: &mut Shared) {
        for _ in 0..500 {
            sh.poll_meta();
            if sh.meta_loading.is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("metadata did not arrive");
    }

    /// colour of each occurrence of `word` as a whole token (adjacent sections with the same format are merged)
    fn colour_of(job: &LayoutJob, word: &str) -> Vec<Color32> {
        let t = &job.text;
        let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == ':');
        t.match_indices(word)
            .filter(|(i, _)| !ident(t[..*i].chars().last()) && !ident(t[i + word.len()..].chars().next()))
            .map(|(i, _)| job.sections.iter().find(|s| s.byte_range.start.0 <= i && i < s.byte_range.end.0).unwrap().format.color)
            .collect()
    }

    #[test]
    fn highlight_tables_columns_and_quoted_names() {
        let pal = crate::theme::default_sql_palette(true);
        let mut n = HlNames::default();
        n.tables.insert("clientes".into());
        n.columns.insert("nome".into());
        let job = highlight("select c.nome from clientes c", &pal, 14.0, 100.0, Some(&n));
        assert_eq!(colour_of(&job, "clientes"), vec![pal.get(SqlCat::Table)]);
        assert_eq!(colour_of(&job, "nome"), vec![pal.get(SqlCat::Column)]);
        assert_eq!(colour_of(&job, "c"), vec![pal.get(SqlCat::Text); 2]);
        let q = r#"select "Nome", "Outro" from "Clientes""#;
        let job = highlight(q, &pal, 14.0, 100.0, Some(&n));
        assert_eq!(colour_of(&job, "\"Nome\""), vec![pal.get(SqlCat::QuotedKnown)]);
        assert_eq!(colour_of(&job, "\"Clientes\""), vec![pal.get(SqlCat::QuotedKnown)]);
        assert_eq!(colour_of(&job, "\"Outro\""), vec![pal.get(SqlCat::Quoted)]);
        let job = highlight(q, &pal, 14.0, 100.0, None);
        for w in ["\"Nome\"", "\"Clientes\""] {
            assert_eq!(colour_of(&job, w), vec![pal.get(SqlCat::Quoted)]);
        }
        let job = highlight("select c.nome from clientes c", &pal, 14.0, 100.0, None);
        assert_eq!(colour_of(&job, "clientes"), vec![pal.get(SqlCat::Text)]);
        // column aliases, where defined and where used, even without metadata
        let job = highlight(r#"select count(*) as qtd, v "Bruto" from t order by qtd"#, &pal, 14.0, 100.0, None);
        assert_eq!(colour_of(&job, "qtd"), vec![pal.get(SqlCat::Column); 2]);
        assert_eq!(colour_of(&job, "\"Bruto\""), vec![pal.get(SqlCat::QuotedKnown)]);
        assert_eq!(colour_of(&job, "t"), vec![pal.get(SqlCat::Text)]);
    }

    #[test]
    fn highlight_variables_inside_strings_and_categories() {
        let pal = crate::theme::default_sql_palette(false);
        let job = highlight("exec p @a, 'arquivo_${data}.csv', convert(int, x)::int", &pal, 14.0, 100.0, None);
        let var = job.sections.iter().find(|s| job.text[s.byte_range.start.0..s.byte_range.end.0].starts_with("${data}")).unwrap();
        assert_eq!(var.format.color, pal.get(SqlCat::Variable));
        assert_eq!(var.format.background, pal.get(SqlCat::VariableBg));
        assert_eq!(colour_of(&job, "'arquivo_"), vec![pal.get(SqlCat::Str)]);
        assert_eq!(colour_of(&job, "@a"), vec![pal.get(SqlCat::Variable)]);
        assert_eq!(colour_of(&job, "convert"), vec![pal.get(SqlCat::Function)]);
        assert_eq!(colour_of(&job, "exec"), vec![pal.get(SqlCat::Keyword)]);
        let at = job.text.rfind(":int").unwrap();
        let sec = job.sections.iter().find(|s| s.byte_range.start.0 <= at && at < s.byte_range.end.0).unwrap();
        assert_eq!(sec.format.color, pal.get(SqlCat::Text), "a ::cast is not a parameter");
    }

    #[test]
    fn single_proposal_is_listed_when_insert_single_is_off() {
        let (mut sh, id) = sqlite();
        let text = "select * from customer_t";
        let n = text.chars().count();
        let mut tab = SqlTab::new(Some(id.clone()), text.into(), "t".into());
        tab.cursor = (n, n);
        tab.apply_cmd(&mut sh, 1, SqlCmd::Complete);
        wait_meta(&mut sh);
        sh.store.prefs.ac_insert_single = false;
        tab.apply_cmd(&mut sh, 1, SqlCmd::Complete);
        assert_eq!(tab.text, text, "nothing is inserted");
        assert_eq!(tab.ac.as_ref().map(|a| a.items.clone()), Some(vec!["customer_totals".to_string()]));
        sh.store.prefs.ac_insert_single = true;
        tab.apply_cmd(&mut sh, 1, SqlCmd::Complete);
        assert!(tab.text.starts_with("select * from customer_totals"), "{}", tab.text);
    }

    #[test]
    fn tabs_of_closed_connections_are_marked() {
        let (mut sh, id) = sqlite();
        assert_eq!(closed(&sh, Some(&id)), "");
        assert_eq!(closed(&sh, None), "", "a tab without connection is not marked");
        sh.disconnect(&id);
        assert_eq!(closed(&sh, Some(&id)), " (desc.)");
    }

    #[test]
    fn highlight_names_come_from_cache_in_background() {
        let (mut sh, id) = sqlite();
        let mut tab = SqlTab::new(Some(id.clone()), "select first_name from customers".into(), "t".into());
        // nothing cached: answers right away, without the names
        let n = tab.hl_names(&mut sh).unwrap();
        assert!(n.tables.is_empty() && n.columns.is_empty());
        for _ in 0..3 {
            wait_meta(&mut sh);
            tab.hl_names(&mut sh);
        }
        let n = tab.hl_names(&mut sh).unwrap();
        assert!(n.tables.contains("customers"));
        assert!(n.columns.contains("first_name"), "{n:?}");
        // the same text and metadata reuse the cached names
        assert!(std::sync::Arc::ptr_eq(&n, &tab.hl_names(&mut sh).unwrap()));
        sh.store.prefs.hl_metadata = false;
        assert!(tab.hl_names(&mut sh).is_none());
    }

    #[test]
    fn completion_never_waits_for_the_database() {
        let (mut sh, id) = sqlite();
        let text = "select * from cust";
        // first call: nothing cached yet; it answers right away (keywords only) and asks for the tables
        let (ac, waiting) = complete(&mut sh, Some(&id), text, text.len());
        assert!(waiting);
        assert!(ac.map(|a| !a.items.contains(&"customers".to_string())).unwrap_or(true));
        wait_meta(&mut sh);
        let (ac, waiting) = complete(&mut sh, Some(&id), text, text.len());
        assert!(!waiting);
        let items = ac.unwrap().items;
        assert!(items.contains(&"customers".to_string()));
        let first_other = items.iter().position(|i| !i.to_lowercase().starts_with("cust")).unwrap_or(items.len());
        assert!(items[first_other..].iter().all(|i| !i.to_lowercase().starts_with("cust")), "names that start with the text come first: {:?}", items);
        // the half-typed table name is not looked up in the database
        assert!(!sh.tmeta.keys().any(|k| k.2 == "cust"));
        // columns of a table named in the statement arrive the same way
        let text = "select c. from customers c";
        let at = "select c.".len();
        let (_, waiting) = complete(&mut sh, Some(&id), text, at);
        assert!(waiting, "the columns are requested in the background");
        wait_meta(&mut sh);
        let (ac, waiting) = complete(&mut sh, Some(&id), text, at);
        assert!(!waiting);
        assert!(ac.unwrap().items.contains(&"first_name".to_string()));
    }

    #[test]
    fn completion_with_many_tables_is_fast() {
        let (mut sh, id) = sqlite();
        let schema = "main".to_string();
        let many: Vec<(String, bool)> = (0..30_000).map(|i| (format!("tabela_{:05}", i), i % 7 == 0)).collect();
        sh.tables.insert((id.clone(), schema), many);
        let text = "select * from tab";
        let t0 = std::time::Instant::now();
        let (ac, _) = complete(&mut sh, Some(&id), text, text.len());
        let took = t0.elapsed();
        assert_eq!(ac.unwrap().items.len(), 60);
        assert!(took < std::time::Duration::from_millis(400), "took {:?}", took);
    }

    /// Runs one frame of the editor with the given typed events.
    fn frame(ctx: &egui::Context, sh: &mut Shared, t: &mut SqlTab, events: Vec<Event>, time: &mut f64) {
        *time += 0.05;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 600.0))), time: Some(*time), events, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| t.show(ui, sh, 7, true));
        out.textures_delta.clear();
    }

    fn type_at(text: &str, at: usize, typed: &str) -> (String, usize) {
        let (mut sh, _) = sqlite();
        let ctx = egui::Context::default();
        let mut t = SqlTab::new(None, text.into(), "t".into());
        t.set_cursor = Some(at);
        t.cursor = (at, at);
        t.focus_editor = true;
        let mut time = 0.0;
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        for ch in typed.chars() {
            frame(&ctx, &mut sh, &mut t, vec![Event::Text(ch.to_string())], &mut time);
        }
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        (t.text.clone(), t.cursor.0)
    }

    #[test]
    fn brackets_close_around_the_cursor_without_touching_the_next_character() {
        // at the end of a line: the pair is closed and the cursor stays between them
        assert_eq!(type_at("select count", 12, "("), ("select count()".to_string(), 13));
        // typing the closing bracket steps over it
        assert_eq!(type_at("select count", 12, "(*)"), ("select count(*)".to_string(), 15));
        // before a space the pair is closed in place, the space is kept after it
        assert_eq!(type_at("where x in  and y", 11, "("), ("where x in () and y".to_string(), 12));
        // in front of a word: only the bracket, the word is not moved or eaten
        assert_eq!(type_at("select abc", 7, "("), ("select (abc".to_string(), 8));
        assert_eq!(type_at("select abc", 7, "'"), ("select 'abc".to_string(), 8));
    }

    #[test]
    fn undefined_variables_are_asked_for_with_the_parameters() {
        let (mut sh, id) = sqlite();
        let mut t = SqlTab::new(Some(id), "@set fixo = 3\nselect ${fixo}, ${tabela_x}, :p from customers where id = ${tabela_x}".into(), "t".into());
        t.execute(&mut sh, 1, true, false, false);
        let req = sh.requests.iter().find_map(|r| match r {
            Req::Params { names, stmts, .. } => Some((names.clone(), stmts.clone())),
            _ => None,
        });
        let (names, stmts) = req.expect("the dialog is asked for");
        assert_eq!(names, vec!["${tabela_x}".to_string(), "p".to_string()], "a variable defined by @set is not asked for");
        assert!(stmts[0].starts_with("select 3, ${tabela_x}"));
    }

    fn f4(sh: &mut Shared, conn: Option<String>, text: &str, at: usize, sel_end: Option<usize>) -> Option<Req> {
        let mut t = SqlTab::new(conn, text.into(), "t".into());
        t.cursor = (sel_end.unwrap_or(at), at);
        sh.requests.clear();
        t.apply_cmd(sh, 1, SqlCmd::OpenObject);
        sh.requests.pop()
    }

    #[test]
    fn f4_opens_the_table_or_the_definition() {
        let (mut sh, id) = sqlite();
        let c = Some(id.clone());
        let q = "SELECT * FROM orders o JOIN customer_totals t ON 1 = 1";
        let at = |w: &str| q.find(w).unwrap() + 2;
        assert!(matches!(f4(&mut sh, c.clone(), q, at("orders"), None), Some(Req::OpenTable { ref table, view: false, .. }) if table == "orders"));
        assert!(matches!(f4(&mut sh, c.clone(), q, at("customer_totals"), None), Some(Req::OpenSource { ref key, ref name, .. }) if key == "view" && name == "customer_totals"));
        // a selection made of a name
        let s = q.find("orders").unwrap();
        assert!(matches!(f4(&mut sh, c.clone(), q, s, Some(s + 6)), Some(Req::OpenTable { .. })));
        // a trigger opens its source
        assert!(matches!(f4(&mut sh, c.clone(), "trg_order_items_stock", 3, None), Some(Req::OpenSource { ref key, .. }) if key == "trigger"));
        // unknown name, keyword, closed connection: nothing opens and the status says why
        assert!(f4(&mut sh, c.clone(), "SELECT * FROM nada_disso", 17, None).is_none());
        assert!(sh.status.contains("não encontrado"), "{}", sh.status);
        assert!(f4(&mut sh, c.clone(), q, 2, None).is_none());
        assert!(f4(&mut sh, None, q, at("orders"), None).is_none());
        assert!(sh.status.contains("fechada"), "{}", sh.status);
    }

    #[test]
    fn right_click_keeps_the_selection_and_finds_the_object_from_the_cache() {
        let (mut sh, id) = sqlite();
        let q = "SELECT * FROM orders";
        let mut t = SqlTab::new(Some(id.clone()), q.into(), "t".into());
        t.cursor = (20, 14);
        assert_eq!(t.right_click_at(&mut sh, 16), None, "inside the selection: nothing moves");
        assert_eq!(t.cursor, (20, 14));
        assert_eq!(t.ctx_obj, None, "metadata not loaded yet: the menu does not wait for it");
        assert!(sh.tables.is_empty(), "and does not query");
        sh.find_object(&id, None, "orders", true);
        t.right_click_at(&mut sh, 16);
        assert_eq!(t.ctx_obj.as_ref().map(|o| (o.name.as_str(), o.kind_label())), Some(("orders", "tabela")));
        assert_eq!(t.right_click_at(&mut sh, 2), Some(2), "outside the selection the cursor moves");
        assert_eq!((t.cursor, t.ctx_obj.clone()), ((2, 2), None), "a keyword has no object section");
    }

    #[test]
    fn a_right_click_in_the_editor_resolves_the_word_clicked() {
        let (mut sh, id) = sqlite();
        sh.find_object(&id, None, "orders", true);
        let text = "orders ".repeat(40) + "\n";
        let mut t = SqlTab::new(Some(id.clone()), text.repeat(20), "t".into());
        let ctx = egui::Context::default();
        let mut time = 0.0;
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        let at = Pos2::new(300.0, 100.0);
        let btn = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE };
        frame(&ctx, &mut sh, &mut t, vec![Event::PointerMoved(at)], &mut time);
        frame(&ctx, &mut sh, &mut t, vec![btn(true)], &mut time);
        frame(&ctx, &mut sh, &mut t, vec![btn(false)], &mut time);
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        assert!(t.cursor.0 > 0 && t.cursor.0 == t.cursor.1, "the cursor went to the click: {:?}", t.cursor);
        assert_eq!(t.ctx_obj.map(|o| o.name), Some("orders".into()));
    }

    fn line_col(text: &str, at: usize) -> (usize, usize) {
        let before: String = text.chars().take(at).collect();
        (before.matches('\n').count(), before.chars().rev().take_while(|c| *c != '\n').count())
    }

    fn page_key(ctx: &egui::Context, sh: &mut Shared, t: &mut SqlTab, key: egui::Key, shift: bool, time: &mut f64) {
        let modifiers = if shift { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
        frame(ctx, sh, t, vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }], time);
        frame(ctx, sh, t, vec![], time);
    }

    fn paged_editor(at: usize) -> (Shared, egui::Context, SqlTab, f64) {
        let (mut sh, _) = sqlite();
        let text: String = (0..200).map(|i| format!("{:03} abcdefghij\n", i)).collect();
        let mut t = SqlTab::new(None, text, "t".into());
        t.set_cursor = Some(at);
        t.cursor = (at, at);
        t.focus_editor = true;
        let ctx = egui::Context::default();
        let mut time = 0.0;
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        frame(&ctx, &mut sh, &mut t, vec![], &mut time);
        (sh, ctx, t, time)
    }

    #[test]
    fn page_down_and_up_move_one_visible_page_keeping_the_column() {
        let start = 10 * 15 + 5; // line 10, column 5 (each line is 15 chars)
        let (mut sh, ctx, mut t, mut time) = paged_editor(start);
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        let (l1, c1) = line_col(&t.text, t.cursor.0);
        assert!(l1 > 15, "a page is several lines: {}", l1);
        assert_eq!(c1, 5, "same column");
        assert_eq!(t.cursor.0, t.cursor.1, "no selection without Shift");
        assert!(t.scroll_y > 0.0, "the view follows the cursor");
        let page = l1 - 10;
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        assert_eq!(line_col(&t.text, t.cursor.0), (l1 + page, 5), "every page has the same size");
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageUp, false, &mut time);
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageUp, false, &mut time);
        assert_eq!(line_col(&t.text, t.cursor.0), (10, 5));
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageUp, false, &mut time);
        assert_eq!(t.cursor.0, 0, "less than a page from the top: the start of the text");
    }

    #[test]
    fn shift_page_down_selects_and_the_end_is_the_limit() {
        let start = 10 * 15 + 5;
        let (mut sh, ctx, mut t, mut time) = paged_editor(start);
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, true, &mut time);
        assert_eq!(t.cursor.1, start, "the selection starts where the cursor was");
        assert!(t.cursor.0 > start);
        let n = t.text.chars().count();
        let (mut sh, ctx, mut t, mut time) = paged_editor(n - 20);
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        assert_eq!(t.cursor.0, n, "less than a page from the end: the end of the text");
    }

    #[test]
    fn page_keys_move_the_completion_list_when_it_is_open() {
        let (mut sh, ctx, mut t, mut time) = paged_editor(3);
        t.ac = Some(Ac { items: (0..30).map(|i| format!("item{}", i)).collect(), sel: 0, start_char: 3 });
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        assert_eq!(t.ac.as_ref().map(|a| a.sel), Some(10));
        assert_eq!(t.cursor.0, 3, "the text cursor does not move");
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageDown, false, &mut time);
        assert_eq!(t.ac.as_ref().map(|a| a.sel), Some(29), "stops at the last item");
        page_key(&ctx, &mut sh, &mut t, egui::Key::PageUp, false, &mut time);
        assert_eq!(t.ac.as_ref().map(|a| a.sel), Some(19));
    }
}
