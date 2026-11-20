//! Dialogs that export a project to a `.sqlion` file and import one.

use crate::app::App;
use crate::bundle::{self, Bundle, ExportSel, ImportSel, Resolve, Target};
use crate::dialogs::{win, Dialog};
use egui::{Color32, RichText};
use std::path::{Path, PathBuf};

const WARN: Color32 = Color32::from_rgb(220, 150, 40);
const ERR: Color32 = Color32::from_rgb(210, 60, 60);
const OK: Color32 = Color32::from_rgb(40, 150, 70);

pub struct ExportState {
    /// project id; empty = the default project
    pub project: String,
    /// (id, name, selected)
    pub conns: Vec<(String, String, bool)>,
    pub passwords: bool,
    pub scripts: Vec<(PathBuf, bool)>,
    pub prefs: bool,
    pub appearance: bool,
    pub history: bool,
    pub file: String,
    pub msg: Option<Result<String, String>>,
}

pub struct ImportState {
    pub file: String,
    pub bundle: Option<Box<Bundle>>,
    pub sel: Option<ImportSel>,
    pub new_project: bool,
    pub name: String,
    pub dir: String,
    /// destination when importing into an open project; empty = the default project
    pub target: String,
    pub msg: Option<Result<String, String>>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn sql_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().map(|e| e == "sql").unwrap_or(false)).collect();
    v.sort();
    v
}

impl App {
    fn project_name(&self, id: &str) -> String {
        self.sh.store.projects.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| "Projeto padrão".into())
    }

    /// Everything of the project selected, passwords left out.
    fn export_state(&self, project: &str) -> ExportState {
        let conns = self.sh.store.connections.iter().filter(|c| c.project == project).map(|c| (c.id.clone(), c.name.clone(), true)).collect();
        let dir = bundle::scripts_dir_of(&self.sh.store, project, &self.scripts_dir);
        let scripts = sql_files(&dir).into_iter().map(|p| (p, true)).collect();
        let file = home().join(format!("{}.{}", self.project_name(project), bundle::EXT)).display().to_string();
        ExportState { project: project.to_string(), conns, passwords: false, scripts, prefs: true, appearance: true, history: true, file, msg: None }
    }

    pub fn open_export_dialog(&mut self, project: &str) {
        self.dialog = Some(Dialog::ExportBundle(Box::new(self.export_state(project))));
    }

    pub fn open_import_dialog(&mut self) {
        self.dialog = Some(Dialog::ImportBundle(Box::new(ImportState { file: String::new(), bundle: None, sel: None, new_project: true, name: String::new(), dir: String::new(), target: String::new(), msg: None })));
    }

    /// Returns false when the dialog closes.
    pub fn export_bundle_dialog(&mut self, ctx: &egui::Context, st: &mut ExportState) -> bool {
        let mut open = true;
        let mut keep = true;
        let mut go = false;
        let mut switch: Option<String> = None;
        let projects: Vec<(String, String)> = std::iter::once((String::new(), "Projeto padrão".to_string())).chain(self.sh.store.projects.iter().map(|p| (p.id.clone(), p.name.clone()))).collect();
        win(ctx, "Exportar projeto (.sqlion)").open(&mut open).default_width(520.0).show(ctx, |ui| {
            ui.add(egui::Label::new("Um arquivo .sqlion leva o projeto para outro computador ou serve de cópia de segurança. Escolha o que incluir.").wrap());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Projeto");
                let cur = projects.iter().find(|p| p.0 == st.project).map(|p| p.1.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("exp_proj").selected_text(cur).show_ui(ui, |ui| {
                    for (id, name) in &projects {
                        if ui.selectable_label(*id == st.project, name).clicked() {
                            switch = Some(id.clone());
                        }
                    }
                });
            });
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                ui.strong(format!("Conexões ({})", st.conns.len()));
                if st.conns.is_empty() {
                    ui.label(RichText::new("Nenhuma conexão neste projeto").weak());
                }
                for (_, name, on) in st.conns.iter_mut() {
                    ui.checkbox(on, name.as_str());
                }
                ui.add_space(4.0);
                ui.strong(format!("Scripts ({})", st.scripts.len()));
                if st.scripts.is_empty() {
                    ui.label(RichText::new("Nenhum script neste projeto").weak());
                }
                for (p, on) in st.scripts.iter_mut() {
                    ui.checkbox(on, p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                }
                ui.add_space(4.0);
                ui.strong("Também incluir");
                ui.checkbox(&mut st.passwords, "Senhas das conexões selecionadas");
                if st.passwords {
                    ui.add(egui::Label::new(RichText::new("As senhas vão ofuscadas, e ofuscar não é criptografar: quem tiver o arquivo consegue recuperá-las. Guarde-o como guardaria as senhas.").color(WARN)).wrap());
                }
                ui.checkbox(&mut st.appearance, "Aparência (ícones e cores das conexões)");
                ui.checkbox(&mut st.history, "Histórico de consultas das conexões selecionadas");
                ui.checkbox(&mut st.prefs, "Preferências");
            });
            ui.add_space(6.0);
            ui.label("Arquivo");
            ui.add(egui::TextEdit::singleline(&mut st.file).desired_width(f32::INFINITY));
            match &st.msg {
                Some(Ok(m)) => {
                    ui.colored_label(OK, m.as_str());
                }
                Some(Err(m)) => {
                    ui.colored_label(ERR, m.as_str());
                }
                None => {}
            }
            ui.horizontal(|ui| {
                go = ui.button("Exportar").clicked();
                if ui.button("Fechar").clicked() {
                    keep = false;
                }
            });
        });
        if let Some(p) = switch {
            *st = self.export_state(&p);
        }
        if go {
            let sel = ExportSel {
                conns: st.conns.iter().filter(|c| c.2).map(|c| c.0.clone()).collect(),
                passwords: st.passwords,
                scripts: st.scripts.iter().filter(|s| s.1).map(|s| s.0.clone()).collect(),
                prefs: st.prefs,
                appearance: st.appearance,
                history: st.history,
            };
            let name = self.project_name(&st.project);
            st.msg = Some(bundle::build(&self.sh.store, &name, &sel).and_then(|b| bundle::write(Path::new(st.file.trim()), &b)).map(|p| format!("Exportado para {}", p.display())));
        }
        keep && open
    }

    /// Returns false when the dialog closes.
    pub fn import_bundle_dialog(&mut self, ctx: &egui::Context, st: &mut ImportState) -> bool {
        let mut open = true;
        let mut keep = true;
        let mut load = false;
        let mut go = false;
        let projects: Vec<(String, String)> = std::iter::once((String::new(), "Projeto padrão".to_string())).chain(self.sh.store.projects.iter().map(|p| (p.id.clone(), p.name.clone()))).collect();
        let store = &self.sh.store;
        let loose = self.scripts_dir.clone();
        win(ctx, "Importar projeto (.sqlion)").open(&mut open).default_width(560.0).show(ctx, |ui| {
            ui.label("Arquivo");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.file).hint_text("/caminho/projeto.sqlion").desired_width(400.0));
                load = ui.button("Ler arquivo").clicked();
            });
            if let (Some(b), Some(sel)) = (st.bundle.as_ref(), st.sel.as_mut()) {
                ui.label(RichText::new(format!("Projeto “{}”, exportado pelo {} {} em {}", b.project, b.app, b.app_version, b.exported_at.get(..16).unwrap_or(&b.exported_at).replace('T', " "))).small().weak());
                ui.add_space(4.0);
                ui.strong("Destino");
                ui.radio_value(&mut st.new_project, true, "Novo projeto em uma pasta");
                if st.new_project {
                    ui.indent("imp_new", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Nome");
                            ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(320.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Pasta");
                            ui.add(egui::TextEdit::singleline(&mut st.dir).hint_text("/caminho/da/pasta").desired_width(320.0));
                        });
                    });
                }
                ui.radio_value(&mut st.new_project, false, "Projeto já aberto");
                if !st.new_project {
                    ui.indent("imp_existing", |ui| {
                        let cur = projects.iter().find(|p| p.0 == st.target).map(|p| p.1.clone()).unwrap_or_default();
                        egui::ComboBox::from_id_salt("imp_proj").selected_text(cur).show_ui(ui, |ui| {
                            for (id, name) in &projects {
                                ui.selectable_value(&mut st.target, id.clone(), name);
                            }
                        });
                    });
                }
                // conflicts exist only against an existing destination (a new project starts empty)
                let target = if st.new_project { None } else { Some(st.target.clone()) };
                let dir = target.as_ref().map(|t| bundle::scripts_dir_of(store, t, &loose));
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let resolve = |ui: &mut egui::Ui, salt: (&str, usize), how: &mut Resolve| {
                        egui::ComboBox::from_id_salt(salt).selected_text(how.label()).show_ui(ui, |ui| {
                            for r in [Resolve::Rename, Resolve::Replace, Resolve::Skip] {
                                ui.selectable_value(how, r, r.label());
                            }
                        });
                    };
                    if let Some(conns) = &b.connections {
                        ui.strong(format!("Conexões ({})", conns.len()));
                        for (i, (c, (on, how))) in conns.iter().zip(sel.conns.iter_mut()).enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(on, format!("{} ({})", c.name, c.kind.label()));
                                if let Some(x) = target.as_ref().and_then(|t| bundle::conn_conflict(store, t, c)) {
                                    ui.label(RichText::new(format!("já existe: {}", x.name)).small().color(WARN));
                                    resolve(ui, ("imp_c", i), how);
                                }
                            });
                        }
                    }
                    if let Some(scripts) = &b.scripts {
                        ui.strong(format!("Scripts ({})", scripts.len()));
                        for (i, (s, (on, how))) in scripts.iter().zip(sel.scripts.iter_mut()).enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(on, s.name.as_str());
                                if dir.as_ref().map(|d| d.join(&s.name).exists()).unwrap_or(false) {
                                    ui.label(RichText::new("já existe").small().color(WARN));
                                    resolve(ui, ("imp_s", i), how);
                                }
                            });
                        }
                    }
                    if b.appearance.is_some() || b.history.is_some() || b.prefs.is_some() {
                        ui.strong("Também incluir");
                    }
                    if b.appearance.is_some() {
                        ui.checkbox(&mut sel.appearance, "Aparência (ícones e cores das conexões)");
                    }
                    if b.history.is_some() {
                        ui.checkbox(&mut sel.history, format!("Histórico de consultas ({} entradas)", b.history.as_ref().map(|h| h.len()).unwrap_or(0)));
                    }
                    if b.prefs.is_some() {
                        ui.checkbox(&mut sel.prefs, "Preferências (substituem as atuais)");
                    }
                    if b.connections.iter().flatten().any(|c| !c.password.is_empty()) {
                        ui.add(egui::Label::new(RichText::new("O arquivo traz senhas das conexões; elas ficam só neste computador, fora da pasta do projeto.").small().weak()).wrap());
                    }
                });
            }
            match &st.msg {
                Some(Ok(m)) => {
                    ui.colored_label(OK, m.as_str());
                }
                Some(Err(m)) => {
                    ui.colored_label(ERR, m.as_str());
                }
                None => {}
            }
            ui.horizontal(|ui| {
                if st.bundle.is_some() {
                    go = ui.button("Importar").clicked();
                }
                if ui.button("Fechar").clicked() {
                    keep = false;
                }
            });
        });
        if load {
            match bundle::read(Path::new(st.file.trim())) {
                Ok(b) => {
                    st.sel = Some(ImportSel::all(&b));
                    if st.name.is_empty() {
                        st.name = if b.project.is_empty() { "Projeto importado".into() } else { b.project.clone() };
                    }
                    if st.dir.is_empty() {
                        st.dir = home().join(&st.name).display().to_string();
                    }
                    st.bundle = Some(Box::new(b));
                    st.msg = None;
                }
                Err(e) => {
                    // nothing changes: the previous contents (if any) are dropped
                    st.bundle = None;
                    st.sel = None;
                    st.msg = Some(Err(e));
                }
            }
        }
        if go {
            if let (Some(b), Some(sel)) = (st.bundle.as_ref(), st.sel.as_ref()) {
                let target = if st.new_project { Target::NewProject { name: st.name.clone(), dir: PathBuf::from(st.dir.trim()) } } else { Target::Existing(st.target.clone()) };
                match bundle::apply(&mut self.sh.store, b, target, sel, &loose) {
                    Ok(m) => {
                        self.sh.save();
                        self.git_cache = None;
                        self.toast(m);
                        return false;
                    }
                    Err(e) => st.msg = Some(Err(e)),
                }
            }
        }
        keep && open
    }
}
