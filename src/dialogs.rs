//! Dialog windows: connection, export, import, preferences, history, parameters, help.
use crate::app::{App, NavSel};
use crate::shared::*;
use crate::db::*;
use crate::export::{self, CsvOptions, ExportOptions, Format};
use crate::store;
use crate::icons::Icon;
use crate::tabs::*;
use egui::{Color32, RichText};

pub enum Dialog {
    Conn { cfg: Box<ConnConfig>, is_new: bool, msg: Option<Result<String, String>>, pass: String },
    Export { conn: String, sql: String, rows: Option<QueryResult>, opts: Box<ExportOptions>, dir: String, file: String, to_clipboard: bool, msg: Option<Result<String, String>> },
    Import { conn: String, schema: String, table: String, path: String, delim: String, header: bool, create: bool, msg: Option<Result<String, String>> },
    Text { title: String, text: String },
    Params { tab: u64, names: Vec<String>, vals: Vec<String>, stmts: Vec<String>, new_tab: bool },
    ConfirmRun { tab: u64, stmts: Vec<String>, new_tab: bool, env: String },
    Prefs,
    History { filter: String },
    ConfirmDelete(String),
    Password { conn: String, pass: String, remember: bool },
    ConfirmDisconnect { conn: String, reconnect: bool, rows: u64 },
    Git { msg: String, output: String, avail: bool, repo: bool, branch: String, changed: usize, dir: std::path::PathBuf },
    RenameProject { id: String, name: String, msg: Option<String> },
    DeleteProject { id: String, msg: Option<String> },
    RenameScript { path: std::path::PathBuf, name: String, msg: Option<String> },
    DeleteScript(std::path::PathBuf),
    About,
    Shortcuts,
}

fn status_label(ui: &mut egui::Ui, m: &Option<Result<String, String>>) {
    if let Some(m) = m {
        match m {
            Ok(v) => ui.colored_label(Color32::from_rgb(40, 150, 70), v.as_str()),
            Err(e) => ui.colored_label(Color32::from_rgb(210, 60, 60), e.as_str()),
        };
    }
}

impl App {
    /// Turns a request from the tabs into an open dialog.
    pub fn dialog_request(&mut self, r: Req) {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        match r {
            Req::Export { conn, sql, name, rows } => {
                let opts = ExportOptions { table_name: name.clone(), ..Default::default() };
                // file name pattern ${table}_${timestamp}
                let file = crate::constants::EXPORT_FILE_PATTERN.replace("${table}", &name).replace("${timestamp}", &chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
                self.dialog = Some(Dialog::Export { conn, sql, rows, opts: Box::new(opts), dir: home, file, to_clipboard: false, msg: None });
            }
            Req::ExportWith { conn, sql, opts, dir, file } => {
                self.dialog = Some(Dialog::Export { conn, sql, rows: None, opts, dir, file, to_clipboard: false, msg: None });
            }
            Req::Import { conn, schema, table } => {
                self.dialog = Some(Dialog::Import { conn, schema, table, path: format!("{}/", home), delim: ",".into(), header: true, create: true, msg: None });
            }
            Req::GenSql { title, text } => self.dialog = Some(Dialog::Text { title, text }),
            Req::ViewValue(v) => {
                let text = serde_json::from_str::<serde_json::Value>(&v)
                    .ok()
                    .filter(|j| j.is_object() || j.is_array())
                    .and_then(|j| serde_json::to_string_pretty(&j).ok())
                    .unwrap_or(v);
                self.dialog = Some(Dialog::Text { title: "Valor da célula".into(), text });
            }
            Req::EditConn(id) => {
                if let Some(c) = self.sh.cfg(&id).cloned() {
                    let pass = store::deobfuscate(&c.password);
                    self.dialog = Some(Dialog::Conn { cfg: Box::new(c), is_new: false, msg: None, pass });
                }
            }
            Req::NewConnIn(project) => {
                let mut c = ConnConfig::new(Kind::Sqlite);
                c.project = project;
                c.set_mode(TxMode::from_auto_commit(self.sh.store.prefs.default_auto_commit));
                self.dialog = Some(Dialog::Conn { cfg: Box::new(c), is_new: true, msg: None, pass: String::new() });
            }
            Req::NewConn => {
                let mut c = ConnConfig::new(Kind::Sqlite);
                c.set_mode(TxMode::from_auto_commit(self.sh.store.prefs.default_auto_commit));
                self.dialog = Some(Dialog::Conn { cfg: Box::new(c), is_new: true, msg: None, pass: String::new() });
            }
            Req::DeleteConn(id) => self.dialog = Some(Dialog::ConfirmDelete(id)),
            Req::Params { tab, names, stmts, new_tab } => {
                let vals = names.iter().map(|n| self.sh.vars.iter().find(|(k, _)| k == n).map(|x| x.1.clone()).unwrap_or_default()).collect();
                self.dialog = Some(Dialog::Params { tab, names, vals, stmts, new_tab });
            }
            Req::ConfirmRun { tab, stmts, new_tab, env } => self.dialog = Some(Dialog::ConfirmRun { tab, stmts, new_tab, env }),
            _ => {}
        }
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.dialog.take() else { return };
        let mut keep = true;
        match &mut d {
            Dialog::Conn { cfg, is_new, msg, pass } => {
                let mut open = true;
                let title = if *is_new { "Nova conexão" } else { "Editar conexão" };
                win(ctx, title).open(&mut open).collapsible(false).show(ctx, |ui| {
                    egui::Grid::new("conn_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Tipo de banco");
                        let current = if cfg.flavor.is_empty() { cfg.kind.label().to_string() } else { cfg.flavor.clone() };
                        egui::ComboBox::from_id_salt("kind").selected_text(current).width(320.0).show_ui(ui, |ui| {
                            for p in PRESETS {
                                if ui.selectable_label(cfg.flavor == p.name || (cfg.flavor.is_empty() && cfg.kind == p.kind && p.name == p.kind.label()), p.name).clicked() {
                                    cfg.kind = p.kind;
                                    cfg.flavor = p.name.to_string();
                                    cfg.port = p.port;
                                    cfg.user = p.user.to_string();
                                }
                            }
                        });
                        ui.end_row();
                        if cfg.kind == Kind::Sqlite {
                            ui.label("Arquivo");
                            ui.add(egui::TextEdit::singleline(&mut cfg.database).hint_text("/caminho/para/banco.sqlite").desired_width(320.0));
                            ui.end_row();
                        } else {
                            ui.label(if cfg.kind == Kind::Mongo { "Servidor ou URI" } else { "Servidor" });
                            ui.add(egui::TextEdit::singleline(&mut cfg.host).desired_width(320.0));
                            ui.end_row();
                            ui.label("Porta");
                            let mut p = cfg.port as u32;
                            ui.add(egui::DragValue::new(&mut p).range(1..=65535));
                            cfg.port = p as u16;
                            ui.end_row();
                            ui.label("Banco de dados");
                            ui.add(egui::TextEdit::singleline(&mut cfg.database).desired_width(320.0));
                            ui.end_row();
                            ui.label("Usuário");
                            ui.add(egui::TextEdit::singleline(&mut cfg.user).desired_width(320.0));
                            ui.end_row();
                            ui.label("Senha");
                            ui.add(egui::TextEdit::singleline(pass).password(true).desired_width(320.0));
                            ui.end_row();
                        }
                        ui.label("Nome da conexão");
                        let hint = cfg.display_name();
                        ui.add(egui::TextEdit::singleline(&mut cfg.name).hint_text(hint).desired_width(320.0));
                        ui.end_row();
                        ui.label("Pasta");
                        ui.add(egui::TextEdit::singleline(&mut cfg.folder).desired_width(320.0));
                        ui.end_row();
                        ui.label("Tipo de conexão");
                        let before_t = cfg.conn_type;
                        egui::ComboBox::from_id_salt("conn_type").selected_text(cfg.conn_type.label()).show_ui(ui, |ui| {
                            for t in crate::constants::ConnType::ALL {
                                ui.selectable_value(&mut cfg.conn_type, t, t.label());
                            }
                        });
                        if before_t != cfg.conn_type {
                            cfg.set_mode(TxMode::from_auto_commit(cfg.conn_type.auto_commit()));
                        }
                        ui.end_row();
                        ui.label("Transações");
                        let mut m = cfg.mode();
                        egui::ComboBox::from_id_salt("tx_mode").selected_text(m.label()).show_ui(ui, |ui| {
                            for t in TxMode::ALL {
                                ui.selectable_value(&mut m, t, t.label());
                            }
                        });
                        if m != cfg.mode() {
                            cfg.set_mode(m);
                        }
                        ui.end_row();
                        ui.label("Segurança");
                        ui.checkbox(&mut cfg.read_only, "Conexão somente leitura");
                        ui.end_row();
                        ui.label("Aparência");
                        appearance_editor(ui, cfg);
                        ui.end_row();
                    });
                    status_label(ui, msg);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Testar conexão").clicked() {
                            let mut t = (**cfg).clone();
                            t.password = store::obfuscate(pass);
                            *msg = Some(Shared::test(&t));
                        }
                        if ui.button("Salvar").clicked() {
                            let mut c = (**cfg).clone();
                            c.password = store::obfuscate(pass);
                            if *is_new {
                                self.sh.store.connections.push(c.clone());
                            } else if let Some(x) = self.sh.store.connections.iter_mut().find(|x| x.id == c.id) {
                                *x = c.clone();
                            }
                            self.sh.disconnect(&c.id);
                            self.sh.save();
                            self.nav_sel = Some(NavSel::Conn(c.id.clone()));
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::Password { conn, pass, remember } => {
                let name = self.sh.conn_name(conn);
                let mut done: Option<bool> = None;
                win(ctx, format!("Senha de {}", name)).show(ctx, |ui| {
                    ui.add(egui::Label::new("Esta conexão veio de um projeto e a senha não é guardada na pasta dele. Informe a senha para este computador.").wrap());
                    ui.add_space(4.0);
                    let r = ui.add(egui::TextEdit::singleline(pass).password(true).desired_width(f32::INFINITY));
                    r.request_focus();
                    ui.checkbox(remember, "Guardar neste computador");
                    ui.horizontal(|ui| {
                        if ui.button("Conectar").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                            done = Some(true);
                        }
                        if ui.button("Tentar sem senha").clicked() {
                            done = Some(false);
                        }
                    });
                });
                if let Some(connect) = done {
                    keep = false;
                    let id = conn.clone();
                    if connect {
                        let hidden = store::obfuscate(pass);
                        if *remember {
                            self.sh.store.set_secret(&id, &hidden);
                        } else if let Some(c) = self.sh.store.connections.iter_mut().find(|c| c.id == id) {
                            c.password = hidden;
                        }
                        self.sh.save();
                    } else {
                        self.sh.password_declined.insert(id.clone());
                    }
                    if let Err(e) = self.sh.connect(&id) {
                        self.toast(e);
                    }
                }
            }
            Dialog::ConfirmDisconnect { conn, reconnect, rows } => {
                let name = self.sh.conn_name(conn);
                let verb = if *reconnect { "reconectar" } else { "desconectar" };
                let mut choice: Option<Option<bool>> = None;
                win(ctx, format!("Alterações pendentes em {}", name)).show(ctx, |ui| {
                    ui.add(egui::Label::new(format!("Há {} registro(s) alterado(s) em uma transação aberta. O que fazer antes de {}?", rows, verb)).wrap());
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Commit e continuar").clicked() {
                            choice = Some(Some(true));
                        }
                        if ui.button("Rollback e continuar").clicked() {
                            choice = Some(Some(false));
                        }
                        if ui.button("Cancelar").clicked() {
                            choice = Some(None);
                        }
                    });
                });
                if let Some(c) = choice {
                    keep = false;
                    if let Some(commit) = c {
                        let (id, re) = (conn.clone(), *reconnect);
                        self.finish_disconnect(&id, re, Some(commit));
                    }
                }
            }
            Dialog::ConfirmDelete(id) => {
                let name = self.sh.conn_name(id);
                win(ctx, "Excluir conexão").collapsible(false).show(ctx, |ui| {
                    ui.label(format!("Excluir a conexão “{}”?", name));
                    ui.horizontal(|ui| {
                        if ui.button("Excluir").clicked() {
                            self.sh.disconnect(id);
                            self.sh.store.connections.retain(|c| &c.id != id);
                            self.sh.save();
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
            }
            Dialog::Export { conn, sql, rows, opts, dir, file, to_clipboard, msg } => {
                let mut open = true;
                win(ctx, "Exportar dados").open(&mut open).collapsible(false).show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Formato:");
                        for f in Format::ALL {
                            if ui.selectable_label(opts.format == f, f.label()).clicked() {
                                opts.format = f;
                            }
                        }
                    });
                    ui.separator();
                    egui::Grid::new("exp_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        match opts.format {
                            Format::Csv => {
                                ui.label("Delimitador");
                                ui.add(egui::TextEdit::singleline(&mut opts.delimiter).desired_width(60.0));
                                ui.end_row();
                                ui.label("Aspas");
                                ui.add(egui::TextEdit::singleline(&mut opts.quote_char).desired_width(60.0));
                                ui.end_row();
                                ui.label("Cabeçalho");
                                ui.checkbox(&mut opts.header, "no topo");
                                ui.end_row();
                                ui.label("Sempre usar aspas");
                                ui.checkbox(&mut opts.quote_always, "");
                                ui.end_row();
                                ui.label("Texto para NULL");
                                ui.add(egui::TextEdit::singleline(&mut opts.null_string).desired_width(100.0));
                                ui.end_row();
                                ui.label("Escapar fórmulas");
                                ui.checkbox(&mut opts.escape_formulas, "");
                                ui.end_row();
                            }
                            Format::Sql => {
                                ui.label("Linhas por INSERT");
                                let mut n = opts.rows_in_statement as u32;
                                ui.add(egui::DragValue::new(&mut n).range(1..=10000));
                                opts.rows_in_statement = n as usize;
                                ui.end_row();
                                ui.label("Nome da tabela");
                                ui.add(egui::TextEdit::singleline(&mut opts.table_name).desired_width(200.0));
                                ui.end_row();
                                ui.label("Palavras-chave em maiúsculas");
                                ui.checkbox(&mut opts.keyword_upper, "");
                                ui.end_row();
                                ui.label("Omitir esquema");
                                ui.checkbox(&mut opts.omit_schema, "");
                                ui.end_row();
                            }
                            Format::Xml => {
                                ui.label("Incluir DOCTYPE");
                                ui.checkbox(&mut opts.include_doctype, "");
                                ui.end_row();
                            }
                            Format::Txt => {
                                ui.label("Mostrar cabeçalho");
                                ui.checkbox(&mut opts.header, "");
                                ui.end_row();
                                ui.label("Mostrar NULL");
                                ui.checkbox(&mut opts.show_nulls, "");
                                ui.end_row();
                            }
                            Format::Markdown => {
                                ui.label("Separador de cabeçalho");
                                ui.checkbox(&mut opts.md_header_sep, "");
                                ui.end_row();
                            }
                            _ => {
                                ui.label("Cabeçalho");
                                ui.checkbox(&mut opts.header, "");
                                ui.end_row();
                            }
                        }
                        ui.label("Saída");
                        ui.checkbox(to_clipboard, "Copiar para a área de transferência");
                        ui.end_row();
                        if !*to_clipboard {
                            ui.label("Pasta");
                            ui.add(egui::TextEdit::singleline(dir).desired_width(320.0));
                            ui.end_row();
                            ui.label("Nome do arquivo");
                            ui.horizontal(|ui| {
                                ui.add(egui::TextEdit::singleline(file).desired_width(250.0));
                                ui.label(format!(".{}", opts.format.ext()));
                            });
                            ui.end_row();
                        }
                    });
                    status_label(ui, msg);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Exportar").clicked() {
                            *msg = Some(do_export(&self.sh, conn, sql, rows.as_ref(), opts, dir, file, *to_clipboard, ctx));
                        }
                        if ui.button("Fechar").clicked() {
                            keep = false;
                        }
                    });
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::Import { conn, schema, table, path, delim, header, create, msg } => {
                let mut open = true;
                win(ctx, format!("Importar CSV em {}", table)).open(&mut open).collapsible(false).show(ctx, |ui| {
                    egui::Grid::new("imp_grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Arquivo CSV");
                        ui.add(egui::TextEdit::singleline(path).desired_width(340.0));
                        ui.end_row();
                        ui.label("Delimitador");
                        ui.add(egui::TextEdit::singleline(delim).desired_width(60.0));
                        ui.end_row();
                        ui.label("Cabeçalho");
                        ui.checkbox(header, "primeira linha contém os nomes das colunas");
                        ui.end_row();
                        ui.label("Tabela");
                        ui.checkbox(create, "criar se não existir (tipos pelas primeiras 100 linhas)");
                        ui.end_row();
                    });
                    status_label(ui, msg);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Importar").clicked() {
                            *msg = Some(do_import(&mut self.sh, conn, schema, table, path, delim, *header, *create));
                        }
                        if ui.button("Fechar").clicked() {
                            keep = false;
                        }
                    });
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::Text { title, text } => {
                let mut open = true;
                let t = title.clone();
                win(ctx, t).open(&mut open).default_size([560.0, 380.0]).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Copiar").clicked() {
                            ui.ctx().copy_text(text.clone());
                        }
                        if ui.button("Abrir no editor SQL").clicked() {
                            let c = self.current_conn();
                            self.sh.req(Req::OpenSql { conn: c, text: text.clone(), title: None });
                            keep = false;
                        }
                    });
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(text).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
                    });
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::Params { tab, names, vals, stmts, new_tab } => {
                win(ctx, "Vincular parâmetros").collapsible(false).show(ctx, |ui| {
                    egui::Grid::new("par_grid").num_columns(2).show(ui, |ui| {
                        for (n, v) in names.iter().zip(vals.iter_mut()) {
                            ui.label(format!(":{}", n));
                            ui.add(egui::TextEdit::singleline(v).desired_width(220.0));
                            ui.end_row();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            let pairs: Vec<(String, String)> = names.iter().cloned().zip(vals.iter().cloned()).collect();
                            for (k, v) in &pairs {
                                self.sh.vars.retain(|(n, _)| n != k);
                                self.sh.vars.push((k.clone(), v.clone()));
                            }
                            let final_stmts: Vec<String> = stmts.iter().map(|s| crate::sqltext::substitute_params(s, &pairs)).collect();
                            let nt = *new_tab;
                            let tid = *tab;
                            if let Some(i) = self.tabs.iter().position(|t| t.id == tid) {
                                if let TabKind::Sql(s) = &mut self.tabs[i].kind {
                                    if let Some(c) = s.conn.clone() {
                                        s.start(&mut self.sh, &c, final_stmts, nt, tid);
                                    }
                                }
                            }
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            if let Some(i) = self.tabs.iter().position(|t| t.id == *tab) {
                                if let TabKind::Sql(s) = &mut self.tabs[i].kind {
                                    s.pending_params = None;
                                }
                            }
                            keep = false;
                        }
                    });
                });
            }
            Dialog::ConfirmRun { tab, stmts, new_tab, env } => {
                win(ctx, "Confirmar execução").collapsible(false).show(ctx, |ui| {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), format!("Conexão de {}: esta instrução altera dados.", env));
                    for s in stmts.iter().take(5) {
                        ui.label(RichText::new(one_line(s)).monospace());
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Executar").clicked() {
                            let (tid, nt, list) = (*tab, *new_tab, stmts.clone());
                            if let Some(i) = self.tabs.iter().position(|t| t.id == tid) {
                                if let TabKind::Sql(s) = &mut self.tabs[i].kind {
                                    if let Some(c) = s.conn.clone() {
                                        s.start_confirmed(&mut self.sh, &c, list, nt);
                                    }
                                }
                            }
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
            }
            Dialog::Git { msg, output, avail, repo, branch, changed, dir } => {
                let dir = dir.clone();
                let mut open = true;
                let mut action: Option<u8> = None;
                win(ctx, "Versionamento dos scripts (git)").open(&mut open).collapsible(false).default_width(560.0).show(ctx, |ui| {
                    ui.label(egui::RichText::new("Pasta").small().weak());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(dir.display().to_string()).monospace());
                        if ui.small_button("Copiar").clicked() {
                            ui.ctx().copy_text(dir.display().to_string());
                        }
                        if ui.small_button("Abrir").clicked() {
                            let _ = crate::shared::open_path(&dir);
                        }
                    });
                    ui.add_space(4.0);
                    if !*avail {
                        ui.colored_label(egui::Color32::from_rgb(210, 60, 60), "O programa git não foi encontrado neste computador. Instale o git para usar o versionamento.");
                    } else if *repo {
                        let okc = crate::theme::pal(ui.visuals().dark_mode).ok;
                        crate::icons::chip(ui, Some(Icon::Check), &format!("Repositório git • ramo {} • {} arquivo(s) alterado(s)", branch, changed), okc);
                    } else {
                        let wc = crate::theme::pal(ui.visuals().dark_mode).warn;
                        crate::icons::chip(ui, Some(Icon::Lock), "Esta pasta ainda não é um repositório git", wc);
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if *avail && !*repo && crate::icons::primary(ui, Icon::Plus, "Iniciar repositório").clicked() {
                            action = Some(0);
                        }
                        if *repo {
                            if crate::icons::button(ui, Icon::Refresh, "Status").clicked() {
                                action = Some(1);
                            }
                            if crate::icons::button(ui, Icon::Script, "Histórico").clicked() {
                                action = Some(2);
                            }
                        }
                    });
                    if *repo {
                        ui.add_space(6.0);
                        ui.label("Mensagem do commit");
                        ui.add(egui::TextEdit::singleline(msg).desired_width(f32::INFINITY));
                        if crate::icons::primary(ui, Icon::Save, "Registrar todos os scripts (commit)").clicked() {
                            action = Some(3);
                        }
                        ui.label(egui::RichText::new("Para enviar a um servidor (GitHub, GitLab...), use no terminal: git remote add origin <endereço> e git push.").small().weak());
                    }
                    if !output.is_empty() {
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut output.as_str()).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
                        });
                    }
                });
                if let Some(a) = action {
                    let out = match a {
                        0 => crate::git::init(&dir),
                        1 => crate::git::status(&dir),
                        2 => crate::git::log(&dir),
                        _ => crate::git::commit_all(&dir, msg),
                    };
                    *output = out.text;
                    *repo = crate::git::is_repo(&dir);
                    *branch = crate::git::branch(&dir).unwrap_or_default();
                    *changed = crate::git::changed_files(&dir).len();
                    self.git_cache = None;
                }
                if !open {
                    keep = false;
                }
            }
            Dialog::RenameScript { path, name, msg } => {
                let mut open = true;
                let mut go = false;
                win(ctx, "Renomear script").open(&mut open).collapsible(false).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(name).desired_width(280.0));
                        ui.label(".sql");
                    });
                    if let Some(m) = msg {
                        ui.colored_label(egui::Color32::from_rgb(210, 60, 60), m.as_str());
                    }
                    ui.horizontal(|ui| {
                        go = ui.button("Renomear").clicked();
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
                if go {
                    match self.rename_script(path, name) {
                        Ok(_) => keep = false,
                        Err(e) => *msg = Some(e),
                    }
                }
                if !open {
                    keep = false;
                }
            }
            Dialog::RenameProject { id, name, msg } => {
                let mut open = true;
                let mut go = false;
                win(ctx, "Renomear projeto").open(&mut open).collapsible(false).show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(name).desired_width(300.0));
                    if let Some(m) = msg {
                        ui.colored_label(egui::Color32::from_rgb(210, 60, 60), m.as_str());
                    }
                    ui.horizontal(|ui| {
                        go = ui.button("Renomear").clicked();
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
                if go {
                    match self.sh.store.rename_project(id, name) {
                        Ok(_) => {
                            self.sh.save();
                            keep = false;
                        }
                        Err(e) => *msg = Some(e),
                    }
                }
                if !open {
                    keep = false;
                }
            }
            Dialog::DeleteProject { id, msg } => {
                let proj = self.sh.store.projects.iter().find(|p| &p.id == id).cloned();
                let (nc, ns) = self.sh.store.project_counts(id);
                let unsaved = self.tabs.iter().filter(|t| self.tab_in_project(t, id)).any(|t| match &t.kind {
                    TabKind::Sql(s) => s.dirty,
                    TabKind::Table(tt) => tt.grid.as_ref().map(|g| g.pending_changes() > 0).unwrap_or(false),
                    _ => false,
                });
                win(ctx, "Excluir a pasta do projeto").collapsible(false).show(ctx, |ui| {
                    match &proj {
                        Some(p) => {
                            ui.add(egui::Label::new(format!("Apagar do disco a pasta “{}” e tudo o que há nela?", p.path)).wrap());
                            ui.label(format!("{} conexão(ões) e {} script(s) deixam de existir. Esta ação não pode ser desfeita.", nc, ns));
                        }
                        None => {
                            ui.label("Projeto não encontrado.");
                        }
                    }
                    if unsaved {
                        ui.colored_label(egui::Color32::from_rgb(220, 150, 40), "Há abas deste projeto com alterações não gravadas; elas serão perdidas.");
                    }
                    if let Some(m) = msg.as_ref() {
                        ui.colored_label(egui::Color32::from_rgb(210, 60, 60), m.as_str());
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                        if proj.is_some() && ui.button("Excluir a pasta").clicked() {
                            *msg = Some(String::new());
                        }
                    });
                });
                if msg.as_deref() == Some("") {
                    *msg = None;
                    self.close_tabs_of_project(id);
                    match self.sh.store.delete_project_folder(id) {
                        Ok(_) => {
                            self.sh.save();
                            keep = false;
                        }
                        Err(e) => *msg = Some(e),
                    }
                }
            }
            Dialog::DeleteScript(path) => {
                let fname = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                win(ctx, "Excluir script").collapsible(false).show(ctx, |ui| {
                    ui.label(format!("Excluir o arquivo “{}”? Se a pasta usa git, o histórico ainda guarda as versões anteriores.", fname));
                    ui.horizontal(|ui| {
                        if ui.button("Excluir").clicked() {
                            self.delete_script(path);
                            keep = false;
                        }
                        if ui.button("Cancelar").clicked() {
                            keep = false;
                        }
                    });
                });
            }
            Dialog::Prefs => {
                let mut open = true;
                let before = serde_json::to_string(&self.sh.store.prefs).unwrap_or_default();
                win(ctx, "Preferências").open(&mut open).collapsible(false).show(ctx, |ui| {
                    let okc = crate::theme::pal(ui.visuals().dark_mode).ok;
                    crate::icons::chip(ui, Some(Icon::Save), "As configurações são salvas automaticamente", okc);
                    ui.label(egui::RichText::new(format!("Cada alteração é gravada na hora em {}", store::config_dir().join("workspace.json").display())).small().weak());
                    ui.add_space(6.0);
                    let p = &mut self.sh.store.prefs;
                    egui::Grid::new("pref_grid").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        ui.label("Tema escuro");
                        ui.checkbox(&mut p.dark, "");
                        ui.end_row();
                        ui.label("Tamanho da fonte");
                        ui.add(egui::Slider::new(&mut p.font_size, 10.0..=22.0));
                        ui.end_row();
                        ui.label("Linhas por página de resultado");
                        let mut n = p.max_rows as u32;
                        ui.add(egui::DragValue::new(&mut n).range(1..=1_000_000));
                        p.max_rows = n as usize;
                        ui.end_row();
                        ui.label("Auto-commit em novas conexões");
                        ui.checkbox(&mut p.default_auto_commit, "");
                        ui.end_row();
                        ui.label("Maiúsculas nas palavras-chave ao formatar");
                        ui.checkbox(&mut p.keyword_upper, "");
                        ui.end_row();
                        ui.label("Delimitador de instrução");
                        ui.add(egui::TextEdit::singleline(&mut p.statement_delimiter).desired_width(50.0));
                        ui.end_row();
                        ui.label("Parâmetros :nome ligados");
                        ui.checkbox(&mut p.params_enabled, "");
                        ui.end_row();
                        ui.label("Variáveis ${nome} ligadas");
                        ui.checkbox(&mut p.vars_enabled, "");
                        ui.end_row();
                        ui.label("Texto para NULL");
                        ui.add(egui::TextEdit::singleline(&mut p.null_text).desired_width(100.0));
                        ui.end_row();
                        ui.label("Dias de histórico");
                        let mut dd = p.history_days;
                        ui.add(egui::DragValue::new(&mut dd).range(1..=3650));
                        p.history_days = dd;
                        ui.end_row();
                        ui.label("Pasta dos scripts");
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut p.scripts_dir).hint_text("(padrão) ~/.config/sqlion/scripts").desired_width(300.0));
                            if ui.small_button("Padrão").clicked() {
                                p.scripts_dir.clear();
                            }
                        });
                        ui.end_row();
                    });
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Para versionar com git, aponte a pasta para dentro de um repositório (ou use Arquivo ▸ Versionar scripts). Scripts já salvos não são movidos ao trocar a pasta.").small().weak());
                });
                if serde_json::to_string(&self.sh.store.prefs).unwrap_or_default() != before {
                    self.sh.save();
                }
                if !open {
                    self.sh.save();
                    keep = false;
                }
            }
            Dialog::History { filter } => {
                let mut open = true;
                win(ctx, "Histórico de consultas").open(&mut open).default_size([760.0, 420.0]).show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(filter).hint_text("🔎 Filtrar").desired_width(300.0));
                    ui.separator();
                    let f = filter.to_lowercase();
                    let page = self.sh.store.prefs.history_page;
                    let items: Vec<crate::store::HistoryEntry> = self.sh.store.history.iter().rev().filter(|h| f.is_empty() || h.sql.to_lowercase().contains(&f)).take(page).cloned().collect();
                    let mut load: Option<String> = None;
                    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                        egui::Grid::new("hist_grid").striped(true).num_columns(5).spacing([14.0, 4.0]).show(ui, |ui| {
                            for h in ["Hora", "Conexão", "Tempo", "Resultado", "SQL"] {
                                ui.strong(h);
                            }
                            ui.end_row();
                            for h in &items {
                                let t = chrono::DateTime::from_timestamp(h.when, 0).map(|d| d.with_timezone(&chrono::Local).format("%d/%m %H:%M:%S").to_string()).unwrap_or_default();
                                ui.label(t);
                                ui.label(&h.conn);
                                ui.label(format!("{} ms", h.ms));
                                ui.label(if h.ok { RichText::new(&h.rows) } else { RichText::new("erro").color(Color32::from_rgb(210, 60, 60)) });
                                if ui.link(one_line(&h.sql)).on_hover_text("Clique para abrir no editor").clicked() {
                                    load = Some(h.sql.clone());
                                }
                                ui.end_row();
                            }
                        });
                    });
                    if let Some(s) = load {
                        let c = self.current_conn();
                        self.sh.req(Req::OpenSql { conn: c, text: s, title: None });
                    }
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::Shortcuts => {
                let mut open = true;
                win(ctx, "Atalhos de teclado").open(&mut open).collapsible(false).show(ctx, |ui| {
                    egui::Grid::new("sc_grid").striped(true).num_columns(2).spacing([20.0, 4.0]).show(ui, |ui| {
                        for (k, d) in [
                            ("Ctrl+Enter", "Executar instrução atual"),
                            ("Alt+X", "Executar script"),
                            ("Ctrl+\\", "Executar em nova aba de resultado"),
                            ("Ctrl+Shift+E", "Plano de execução"),
                            ("Ctrl+Shift+F", "Formatar SQL"),
                            ("Ctrl+/", "Comentar / descomentar"),
                            ("Ctrl+Espaço", "Autocompletar"),
                            ("Ctrl+]", "Novo script SQL"),
                            ("Ctrl+S", "Salvar script"),
                            ("Ctrl+W", "Fechar aba"),
                            ("F5", "Atualizar navegador"),
                            ("F2 / duplo clique", "Editar célula"),
                        ] {
                            ui.label(RichText::new(k).strong());
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
                if !open {
                    keep = false;
                }
            }
            Dialog::About => {
                let mut open = true;
                win(ctx, "Sobre").open(&mut open).default_size([480.0, 470.0]).show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                        let weak = crate::theme::pal(ui.visuals().dark_mode).weak;
                        ui.heading("Sqlion");
                        ui.label(RichText::new(format!("Versão {}", env!("CARGO_PKG_VERSION"))).color(weak));
                        ui.add_space(6.0);
                        ui.add(egui::Label::new("Cliente de bancos de dados em Rust (egui): navegador de bancos, editor SQL, grade de dados editável, exportação e importação, diagrama ER, modos de transação (Auto, Manual, Smart) e projetos.").wrap());
                        ui.add_space(6.0);
                        ui.label(RichText::new("Bancos suportados").strong());
                        ui.add(egui::Label::new("SQLite, PostgreSQL, MySQL/MariaDB, SQL Server e MongoDB.").wrap());
                        ui.add_space(6.0);
                        ui.label(RichText::new("Licença").strong());
                        ui.add(egui::Label::new(format!("{} — software livre. O código-fonte e o texto da licença acompanham o projeto (arquivo LICENSE).", env!("CARGO_PKG_LICENSE"))).wrap());
                        ui.add_space(6.0);
                        ui.label(RichText::new("Créditos").strong());
                        ui.add(egui::Label::new("Inspirado no comportamento do DBeaver Community (Apache-2.0, © DBeaver Corp). Sqlion não é afiliado ao DBeaver nem endossado por seus autores; \"DBeaver\" é marca registrada de seus titulares.").wrap());
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(RichText::new("Construído com egui/eframe, rusqlite, postgres, mysql, tiberius e mongodb.").small().color(weak)).wrap());
                    });
                });
                if !open {
                    keep = false;
                }
            }
        }
        if keep {
            self.dialog = Some(d);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn do_export(sh: &Shared, conn: &str, sql: &str, rows: Option<&QueryResult>, opts: &ExportOptions, dir: &str, file: &str, clip: bool, ctx: &egui::Context) -> Result<String, String> {
    let qr = match rows {
        Some(r) => r.clone(),
        None => {
            let s = sh.session(conn).ok_or("Conexão fechada")?;
            let mut g = s.lock().unwrap_or_else(|e| e.into_inner());
            g.run(sql, 0, 0).map_err(|e| e.to_string())?
        }
    };
    let text = export::render(&qr, opts, sh.kind(conn));
    if clip {
        ctx.copy_text(text);
        return Ok(format!("{} linhas copiadas para a área de transferência", qr.rows.len()));
    }
    let path = std::path::Path::new(dir).join(format!("{}.{}", file, opts.format.ext()));
    std::fs::write(&path, text).map_err(|e| format!("Não foi possível gravar: {}", e))?;
    Ok(format!("{} linhas exportadas para {}", qr.rows.len(), path.display()))
}

#[allow(clippy::too_many_arguments)]
fn do_import(sh: &mut Shared, conn: &str, schema: &str, table: &str, path: &str, delim: &str, header: bool, create: bool) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Não foi possível ler o arquivo: {}", e))?;
    let o = CsvOptions { delimiter: delim.chars().next().unwrap_or(','), quote: '"', header };
    let mut rows = export::parse_csv(&text, &o);
    if rows.is_empty() {
        return Err("Arquivo vazio".into());
    }
    let hdr: Vec<String> = if header { rows.remove(0) } else { (1..=rows[0].len()).map(|i| format!("column{}", i)).collect() };
    let kind = sh.kind(conn);
    let sess = sh.session(conn).ok_or("Conexão fechada")?;
    let q = sh.qualified(conn, schema, table);
    if kind == Kind::Mongo {
        let mut g = sess.lock().unwrap_or_else(|e| e.into_inner());
        let mut n = 0usize;
        for chunk in rows.chunks(100) {
            let docs: Vec<String> = chunk
                .iter()
                .map(|r| {
                    let fields: Vec<String> = (0..hdr.len())
                        .filter_map(|i| match r.get(i) {
                            Some(v) if !v.is_empty() => Some(format!("{}: {}", serde_json::to_string(&hdr[i]).unwrap_or_default(), crate::drivers::mongo_value(&Some(v.clone()), false))),
                            _ => None,
                        })
                        .collect();
                    format!("{{{}}}", fields.join(", "))
                })
                .collect();
            g.run(&format!("{}.insertMany([{}])", q, docs.join(", ")), 0, 0).map_err(|e| e.to_string())?;
            n += chunk.len();
        }
        drop(g);
        sh.tables.remove(&(conn.to_string(), schema.to_string()));
        return Ok(format!("{} documentos importados em {}", n, table));
    }
    let exists = sh.ensure_tables(conn, schema).iter().any(|(n, _)| n == table);
    let mut g = sess.lock().unwrap_or_else(|e| e.into_inner());
    if !exists {
        if !create {
            return Err("A tabela não existe".into());
        }
        let defs: Vec<String> = hdr
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let samples: Vec<&str> = rows.iter().take(crate::constants::IMPORT_TYPE_SAMPLES).map(|r| r.get(i).map(|s| s.as_str()).unwrap_or("")).collect();
                format!("{} {}", kind.quote(h), export::guess_type(&samples))
            })
            .collect();
        g.run(&format!("CREATE TABLE {} ({})", q, defs.join(", ")), 0, 0).map_err(|e| e.to_string())?;
    }
    let cols = hdr.iter().map(|h| kind.quote(h)).collect::<Vec<_>>().join(", ");
    let wrap = g.auto_commit();
    if wrap {
        g.run("BEGIN", 0, 0).map_err(|e| e.to_string())?;
    }
    let mut n = 0usize;
    let mut err = None;
    for chunk in rows.chunks(100) {
        let vals: Vec<String> = chunk
            .iter()
            .map(|r| {
                format!(
                    "({})",
                    (0..hdr.len())
                        .map(|i| match r.get(i) {
                            Some(v) if !v.is_empty() => export::sql_value(&Some(v.clone())),
                            _ => "NULL".into(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        if let Err(e) = g.run(&format!("INSERT INTO {} ({}) VALUES {}", q, cols, vals.join(", ")), 0, 0) {
            err = Some(e.to_string());
            break;
        }
        n += chunk.len();
    }
    if wrap {
        let _ = g.run(if err.is_some() { "ROLLBACK" } else { "COMMIT" }, 0, 0);
    }
    drop(g);
    sh.tables.remove(&(conn.to_string(), schema.to_string()));
    sh.tmeta.remove(&(conn.to_string(), schema.to_string(), table.to_string()));
    match err {
        Some(e) => Err(e),
        None => Ok(format!("{} linhas importadas em {}", n, table)),
    }
}

/// Every dialog is built here: it opens centered, then can be dragged by its title and resized from its edges.
fn win<'a>(ctx: &egui::Context, title: impl Into<egui::WidgetText>) -> egui::Window<'a> {
    egui::Window::new(title).default_pos(ctx.content_rect().center()).pivot(egui::Align2::CENTER_CENTER).resizable(true).collapsible(false).constrain(true)
}

const PALETTE: [[u8; 3]; 12] = [
    [60, 140, 200],
    [51, 103, 145],
    [0, 117, 143],
    [66, 150, 80],
    [120, 170, 60],
    [230, 180, 40],
    [235, 130, 40],
    [190, 60, 60],
    [200, 70, 140],
    [130, 90, 190],
    [90, 100, 120],
    [30, 34, 42],
];

/// Colour, icon and label of the connection badge, with a live preview.
fn appearance_editor(ui: &mut egui::Ui, cfg: &mut ConnConfig) {
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            // preview
            let (rc, _) = ui.allocate_exact_size(egui::vec2(34.0, 24.0), egui::Sense::hover());
            crate::icons::conn_badge(ui.painter(), rc, &cfg.badge_style());
            let mut rgb = cfg.color.unwrap_or_else(|| {
                let (r, g, b) = cfg.kind.badge().1;
                [r, g, b]
            });
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                cfg.color = Some(rgb);
            }
            for p in PALETTE {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                ui.painter().rect_filled(r, 3.0, Color32::from_rgb(p[0], p[1], p[2]));
                if cfg.color == Some(p) {
                    ui.painter().rect_stroke(r.expand(1.5), 3.0, egui::Stroke::new(1.5, ui.visuals().strong_text_color()), egui::StrokeKind::Outside);
                }
                if resp.clicked() {
                    cfg.color = Some(p);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Ícone");
            for (name, icon) in crate::icons::PICKABLE {
                let selected = cfg.icon.as_deref() == Some(*name);
                let (r, resp) = ui.allocate_exact_size(egui::vec2(24.0, 22.0), egui::Sense::click());
                if selected || resp.hovered() {
                    ui.painter().rect_filled(r, 4.0, ui.visuals().widgets.hovered.bg_fill);
                }
                crate::icons::paint(ui.painter(), r.shrink(3.0), *icon, if selected { ui.visuals().strong_text_color() } else { ui.visuals().text_color() });
                if resp.on_hover_text(*name).clicked() {
                    cfg.icon = Some((*name).to_string());
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("ou rótulo (até 3 letras)");
            let mut label = cfg.icon.clone().filter(|i| crate::icons::icon_by_name(i).is_none()).unwrap_or_default();
            if ui.add(egui::TextEdit::singleline(&mut label).desired_width(50.0).char_limit(3)).changed() {
                cfg.icon = if label.trim().is_empty() { None } else { Some(label) };
            }
            if ui.button("Restaurar padrão").clicked() {
                cfg.reset_appearance();
            }
        });
    });
}
