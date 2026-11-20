//! `.sqlion` files: a project (or part of it) in one JSON file, for backup and for carrying it to another
//! computer. Unlike a project folder, it may carry passwords (obfuscated), preferences and history.

use crate::db::ConnConfig;
use crate::store::{HistoryEntry, Prefs, Store};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub const BUNDLE_FORMAT: u32 = 1;
pub const EXT: &str = "sqlion";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BundleScript {
    pub name: String,
    pub text: String,
    /// id of the connection the script last ran on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conn: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Appearance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Bundle {
    pub format_version: u32,
    pub app: String,
    pub app_version: String,
    pub exported_at: String,
    #[serde(default)]
    pub project: String,
    /// without icon and color (those go in `appearance`); passwords obfuscated, or empty when left out
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<ConnConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<Vec<BundleScript>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefs: Option<Prefs>,
    /// connection id -> icon and color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<BTreeMap<String, Appearance>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<HistoryEntry>>,
}

/// What goes into the file.
#[derive(Clone, Default)]
pub struct ExportSel {
    /// ids of the connections
    pub conns: Vec<String>,
    pub passwords: bool,
    /// script files
    pub scripts: Vec<PathBuf>,
    pub prefs: bool,
    /// icons and colors of the exported connections
    pub appearance: bool,
    /// queries run on the exported connections
    pub history: bool,
}

pub fn build(store: &Store, project: &str, sel: &ExportSel) -> Result<Bundle, String> {
    let chosen: Vec<&ConnConfig> = store.connections.iter().filter(|c| sel.conns.contains(&c.id)).collect();
    let conns: Vec<ConnConfig> = chosen
        .iter()
        .map(|c| {
            let mut c = (*c).clone();
            c.project.clear();
            c.color = None;
            c.icon = None;
            if !sel.passwords {
                c.password.clear();
            }
            c
        })
        .collect();
    let mut scripts = Vec::new();
    for p in &sel.scripts {
        let text = std::fs::read_to_string(p).map_err(|e| format!("Não foi possível ler {}: {}", p.display(), e))?;
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        scripts.push(BundleScript { name, text, conn: store.script_conn_of(p) });
    }
    let appearance: BTreeMap<String, Appearance> = chosen.iter().filter(|c| c.color.is_some() || c.icon.is_some()).map(|c| (c.id.clone(), Appearance { color: c.color, icon: c.icon.clone() })).collect();
    let names: Vec<&str> = chosen.iter().map(|c| c.name.as_str()).collect();
    let history: Vec<HistoryEntry> = store.history.iter().filter(|h| names.contains(&h.conn.as_str())).cloned().collect();
    Ok(Bundle {
        format_version: BUNDLE_FORMAT,
        app: "Sqlion".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        project: project.to_string(),
        connections: (!sel.conns.is_empty()).then_some(conns),
        scripts: (!sel.scripts.is_empty()).then_some(scripts),
        prefs: sel.prefs.then(|| store.prefs.clone()),
        appearance: (sel.appearance && !sel.conns.is_empty()).then_some(appearance),
        history: (sel.history && !sel.conns.is_empty()).then_some(history),
    })
}

pub fn to_json(b: &Bundle) -> String {
    serde_json::to_string_pretty(b).unwrap_or_default() + "\n"
}

/// Writes the file, adding the `.sqlion` extension when it is missing; returns the path written.
pub fn write(path: &Path, b: &Bundle) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Informe o arquivo".into());
    }
    let path = if path.extension().map(|e| e == EXT).unwrap_or(false) { path.to_path_buf() } else { path.with_extension(EXT) };
    std::fs::write(&path, to_json(b)).map_err(|e| format!("Não foi possível gravar {}: {}", path.display(), e))?;
    Ok(path)
}

pub fn parse(text: &str) -> Result<Bundle, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| "O arquivo não é um .sqlion válido (não é JSON)".to_string())?;
    let ver = v.get("format_version").and_then(|x| x.as_u64()).ok_or("O arquivo não é um .sqlion válido (sem format_version)")?;
    if ver == 0 || ver > BUNDLE_FORMAT as u64 {
        return Err(format!("Versão de formato desconhecida ({}); atualize o Sqlion para importar este arquivo", ver));
    }
    serde_json::from_value(v).map_err(|e| format!("O arquivo não é um .sqlion válido: {}", e))
}

pub fn read(path: &Path) -> Result<Bundle, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Não foi possível ler {}: {}", path.display(), e))?;
    parse(&text)
}

/// What to do with an item that already exists in the destination.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Resolve {
    /// keep both: the imported one gets a new name (and a new id for connections)
    Rename,
    Replace,
    Skip,
}

impl Resolve {
    pub fn label(&self) -> &'static str {
        match self {
            Resolve::Rename => "Manter os dois (renomear)",
            Resolve::Replace => "Substituir",
            Resolve::Skip => "Ignorar",
        }
    }
}

pub enum Target {
    NewProject { name: String, dir: PathBuf },
    /// id of an open project; empty = the default project
    Existing(String),
}

/// What to import; one entry per item of the bundle, in its order.
pub struct ImportSel {
    pub conns: Vec<(bool, Resolve)>,
    pub scripts: Vec<(bool, Resolve)>,
    pub prefs: bool,
    pub appearance: bool,
    pub history: bool,
}

impl ImportSel {
    /// Everything selected, conflicts kept as both.
    pub fn all(b: &Bundle) -> ImportSel {
        ImportSel {
            conns: b.connections.iter().flatten().map(|_| (true, Resolve::Rename)).collect(),
            scripts: b.scripts.iter().flatten().map(|_| (true, Resolve::Rename)).collect(),
            prefs: b.prefs.is_some(),
            appearance: b.appearance.is_some(),
            history: b.history.is_some(),
        }
    }
}

/// Connection in the store that the imported one collides with: same id anywhere, or same name in the project.
pub fn conn_conflict<'a>(store: &'a Store, project: &str, c: &ConnConfig) -> Option<&'a ConnConfig> {
    store.connections.iter().find(|x| x.id == c.id).or_else(|| store.connections.iter().find(|x| x.project == project && x.name == c.name))
}

/// The folder where the destination keeps its scripts.
pub fn scripts_dir_of(store: &Store, project: &str, loose: &Path) -> PathBuf {
    store.projects.iter().find(|p| p.id == project).map(|p| p.scripts_dir()).unwrap_or_else(|| loose.to_path_buf())
}

fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    (2..).map(|n| format!("{} ({})", base, n)).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
}

fn unique_script(dir: &Path, name: &str) -> PathBuf {
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Script".into());
    let ext = p.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "sql".into());
    (1..).map(|n| dir.join(format!("{}-{}.{}", stem, n, ext))).find(|p| !p.exists()).unwrap_or_else(|| dir.join(name))
}

fn fresh_conn_id(store: &Store) -> String {
    loop {
        let id = crate::store::new_id();
        if !store.connections.iter().any(|c| c.id == id) {
            return id;
        }
    }
}

/// A script name from the file must stay a plain file name inside the scripts folder.
fn safe_script_name(name: &str) -> Option<String> {
    let n = Path::new(name).file_name()?.to_string_lossy().to_string();
    (n == name && !n.starts_with('.') && !n.is_empty()).then_some(n)
}

/// Applies the import. Everything that can fail is checked before the first change; only items the user chose
/// to replace overwrite anything. Returns a summary for the user.
pub fn apply(store: &mut Store, b: &Bundle, target: Target, sel: &ImportSel, loose: &Path) -> Result<String, String> {
    let scripts = b.scripts.clone().unwrap_or_default();
    for (s, (on, _)) in scripts.iter().zip(&sel.scripts) {
        if *on && safe_script_name(&s.name).is_none() {
            return Err(format!("Nome de script inválido no arquivo: {}", s.name));
        }
    }
    let pid = match target {
        Target::NewProject { name, dir } => store.create_project(&name, &dir)?.id,
        Target::Existing(id) => {
            if !id.is_empty() && !store.projects.iter().any(|p| p.id == id) {
                return Err("Projeto de destino não encontrado".into());
            }
            id
        }
    };
    let dir = scripts_dir_of(store, &pid, loose);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Não foi possível criar {}: {}", dir.display(), e))?;
    let (mut n_conns, mut n_scripts) = (0, 0);
    // old id -> id in this store, for script links and appearance
    let mut ids: HashMap<String, String> = HashMap::new();
    for (c, (on, how)) in b.connections.iter().flatten().zip(&sel.conns) {
        if !on {
            continue;
        }
        let mut c = c.clone();
        let old = c.id.clone();
        c.project = pid.clone();
        match conn_conflict(store, &pid, &c).map(|x| x.id.clone()) {
            None => {}
            Some(existing) => match how {
                Resolve::Skip => {
                    ids.insert(old, existing);
                    continue;
                }
                Resolve::Replace => {
                    // the imported settings take the place (and the id) of the existing connection, so its links hold
                    let prev = store.connections.iter().position(|x| x.id == existing).expect("conflict found it");
                    let prev = store.connections.remove(prev);
                    c.id = existing.clone();
                    c.folder = if c.folder.is_empty() { prev.folder } else { c.folder };
                    if c.password.is_empty() {
                        c.password = prev.password;
                    }
                }
                Resolve::Rename => {
                    if store.connections.iter().any(|x| x.id == c.id) {
                        c.id = fresh_conn_id(store);
                    }
                    let p = pid.clone();
                    if store.connections.iter().any(|x| x.project == p && x.name == c.name) {
                        c.name = unique_name(&c.name, |n| store.connections.iter().any(|x| x.project == p && x.name == n));
                    }
                }
            },
        }
        ids.insert(old, c.id.clone());
        store.connections.push(c);
        n_conns += 1;
    }
    for (s, (on, how)) in scripts.iter().zip(&sel.scripts) {
        if !on {
            continue;
        }
        let name = safe_script_name(&s.name).expect("checked above");
        let mut path = dir.join(&name);
        if path.exists() {
            match how {
                Resolve::Skip => continue,
                Resolve::Replace => {}
                Resolve::Rename => path = unique_script(&dir, &name),
            }
        }
        std::fs::write(&path, &s.text).map_err(|e| format!("Não foi possível gravar {}: {}", path.display(), e))?;
        if let Some(conn) = s.conn.as_ref().and_then(|c| ids.get(c).cloned().or_else(|| store.connections.iter().any(|x| &x.id == c).then(|| c.clone()))) {
            let key = store.script_key(&path);
            store.script_conns.insert(key, conn);
        }
        n_scripts += 1;
    }
    if sel.appearance {
        for (old, a) in b.appearance.iter().flatten() {
            let id = ids.get(old).cloned().unwrap_or_else(|| old.clone());
            if let Some(c) = store.connections.iter_mut().find(|c| c.id == id) {
                c.color = a.color;
                c.icon = a.icon.clone();
            }
        }
    }
    if sel.prefs {
        if let Some(p) = &b.prefs {
            // the scripts folder is a path of the other computer
            let scripts_dir = std::mem::take(&mut store.prefs.scripts_dir);
            store.prefs = p.clone();
            store.prefs.scripts_dir = scripts_dir;
        }
    }
    if sel.history {
        let have: std::collections::HashSet<(i64, String)> = store.history.iter().map(|h| (h.when, h.sql.clone())).collect();
        store.history.extend(b.history.iter().flatten().filter(|h| !have.contains(&(h.when, h.sql.clone()))).cloned());
        store.history.sort_by_key(|h| h.when);
        let n = store.history.len();
        if n > 2000 {
            store.history.drain(0..n - 2000);
        }
    }
    if let Some(p) = store.projects.iter().find(|p| p.id == pid).cloned() {
        store.write_project(&p)?;
    }
    Ok(format!("Importado: {} conexão(ões) e {} script(s)", n_conns, n_scripts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Kind;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dbr-test-bundle-{}-{}", tag, crate::store::new_project_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A project with two connections (one with password, icon and color), two scripts and some history.
    fn sample(base: &Path) -> (Store, String, Vec<String>, Vec<PathBuf>) {
        let mut s = Store::memory();
        let p = s.create_project("Origem", &base.join("origem")).unwrap();
        let mut a = ConnConfig::new(Kind::Postgres);
        a.name = "Financeiro".into();
        a.project = p.id.clone();
        a.password = crate::store::obfuscate("s3nh4");
        a.color = Some([190, 60, 60]);
        a.icon = Some("lock".into());
        let mut b = ConnConfig::new(Kind::Sqlite);
        b.id = format!("{}-b", a.id);
        b.name = "Local".into();
        b.project = p.id.clone();
        let ids = vec![a.id.clone(), b.id.clone()];
        s.connections.push(a);
        s.connections.push(b);
        let s1 = p.scripts_dir().join("relatorio.sql");
        let s2 = p.scripts_dir().join("limpeza.sql");
        std::fs::write(&s1, "select * from vendas").unwrap();
        std::fs::write(&s2, "delete from tmp").unwrap();
        s.script_conns.insert(s.script_key(&s1), ids[0].clone());
        for (i, conn) in ["Financeiro", "Local", "Outra"].iter().enumerate() {
            s.history.push(HistoryEntry { when: 100 + i as i64, conn: conn.to_string(), sql: format!("select {}", i), ms: 1, rows: "1 linhas".into(), ok: true });
        }
        s.prefs.max_rows = 777;
        (s, p.id, ids, vec![s1, s2])
    }

    fn everything(ids: &[String], scripts: &[PathBuf]) -> ExportSel {
        ExportSel { conns: ids.to_vec(), passwords: true, scripts: scripts.to_vec(), prefs: true, appearance: true, history: true }
    }

    #[test]
    fn full_export_has_every_section() {
        let base = temp("full");
        let (s, _, ids, scripts) = sample(&base);
        let b = build(&s, "Origem", &everything(&ids, &scripts)).unwrap();
        let text = to_json(&b);
        let back = parse(&text).unwrap();
        assert_eq!((back.format_version, back.app.as_str(), back.app_version.as_str()), (BUNDLE_FORMAT, "Sqlion", env!("CARGO_PKG_VERSION")));
        assert_eq!(back.connections.as_ref().unwrap().len(), 2);
        assert_eq!(back.scripts.as_ref().unwrap().len(), 2);
        assert_eq!(back.prefs.as_ref().unwrap().max_rows, 777);
        assert_eq!(back.appearance.as_ref().unwrap().get(&ids[0]), Some(&Appearance { color: Some([190, 60, 60]), icon: Some("lock".into()) }));
        assert_eq!(back.history.as_ref().unwrap().len(), 2, "only the history of the exported connections");
        let fin = &back.connections.as_ref().unwrap()[0];
        assert_eq!(crate::store::deobfuscate(&fin.password), "s3nh4", "passwords go obfuscated");
        assert!(!text.contains("s3nh4"), "never in clear text");
        assert!(fin.color.is_none() && fin.icon.is_none() && fin.project.is_empty(), "appearance lives in its own section");
        assert_eq!(back.scripts.as_ref().unwrap()[0].conn.as_deref(), Some(ids[0].as_str()), "the script keeps its connection");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn without_passwords_and_partial_selection() {
        let base = temp("partial");
        let (s, _, ids, scripts) = sample(&base);
        let sel = ExportSel { conns: vec![ids[0].clone()], passwords: false, scripts: vec![scripts[1].clone()], ..Default::default() };
        let b = build(&s, "Origem", &sel).unwrap();
        let text = to_json(&b);
        assert!(!text.contains(&crate::store::obfuscate("s3nh4")), "no password at all");
        assert!(b.connections.as_ref().unwrap().iter().all(|c| c.password.is_empty()));
        assert_eq!(b.connections.as_ref().unwrap().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Financeiro"]);
        assert_eq!(b.scripts.as_ref().unwrap().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["limpeza.sql"]);
        assert!(b.prefs.is_none() && b.appearance.is_none() && b.history.is_none(), "unchecked sections are left out");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn invalid_files_and_unknown_versions_are_refused() {
        assert!(parse("isto não é json").err().unwrap().contains("não é um .sqlion válido"));
        assert!(parse("{}").err().unwrap().contains("format_version"));
        assert!(parse(r#"{"format_version": 9, "app": "Sqlion"}"#).err().unwrap().contains("Versão de formato desconhecida (9)"));
        assert!(parse(r#"{"format_version": 1}"#).is_err(), "required fields missing");
        // nothing changes when the file is refused: there is nothing to apply
    }

    #[test]
    fn round_trip_into_an_empty_workspace() {
        let base = temp("roundtrip");
        let (s, _, ids, scripts) = sample(&base);
        let b = parse(&to_json(&build(&s, "Origem", &everything(&ids, &scripts)).unwrap())).unwrap();
        let mut empty = Store::memory();
        let loose = base.join("soltos");
        let msg = apply(&mut empty, &b, Target::NewProject { name: "Destino".into(), dir: base.join("destino") }, &ImportSel::all(&b), &loose).unwrap();
        assert!(msg.contains("2 conexão(ões) e 2 script(s)"));
        let p = empty.projects.iter().find(|p| p.name == "Destino").unwrap().clone();
        assert!(p.file().is_file(), "the project is created in the folder");
        assert_eq!(std::fs::read_to_string(p.scripts_dir().join("relatorio.sql")).unwrap(), "select * from vendas");
        let fin = empty.connections.iter().find(|c| c.name == "Financeiro").unwrap();
        assert_eq!((fin.id.as_str(), fin.project.as_str()), (ids[0].as_str(), p.id.as_str()), "ids are kept when free");
        assert_eq!(crate::store::deobfuscate(&fin.password), "s3nh4");
        assert_eq!((fin.color, fin.icon.as_deref()), (Some([190, 60, 60]), Some("lock")));
        assert_eq!(empty.script_conn_of(&p.scripts_dir().join("relatorio.sql")), Some(ids[0].clone()));
        assert_eq!(empty.prefs.max_rows, 777);
        assert_eq!(empty.history.len(), 2);
        // the project file reloads with the imported connections, without the password
        let file = std::fs::read_to_string(p.file()).unwrap();
        assert!(file.contains("Financeiro") && !file.contains(&crate::store::obfuscate("s3nh4")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn conflicts_are_renamed_replaced_or_ignored() {
        let base = temp("conflicts");
        let (s, pid, ids, scripts) = sample(&base);
        let b = build(&s, "Origem", &everything(&ids, &scripts)).unwrap();
        let loose = base.join("soltos");
        let dir = scripts_dir_of(&s, &pid, &loose);
        // importing into the same project: every connection and script already exists
        let run = |how: Resolve| {
            let (mut t, pid) = (Store::memory(), pid.clone());
            for c in &s.connections {
                t.connections.push(c.clone());
            }
            t.projects = s.projects.clone();
            std::fs::write(dir.join("relatorio.sql"), "select * from vendas").unwrap();
            std::fs::write(dir.join("limpeza.sql"), "local, não importado").unwrap();
            let sel = ImportSel { conns: vec![(true, how); 2], scripts: vec![(true, how); 2], prefs: false, appearance: false, history: false };
            apply(&mut t, &b, Target::Existing(pid), &sel, &loose).unwrap();
            t
        };
        let t = run(Resolve::Skip);
        assert_eq!(t.connections.len(), 2, "ignored: nothing added");
        assert_eq!(std::fs::read_to_string(dir.join("limpeza.sql")).unwrap(), "local, não importado", "ignored: the local file is kept");

        let t = run(Resolve::Rename);
        assert_eq!(t.connections.len(), 4, "kept both");
        let names: Vec<&str> = t.connections.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Financeiro (2)") && names.contains(&"Local (2)"));
        let all_ids: std::collections::HashSet<&str> = t.connections.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(all_ids.len(), 4, "the copies get new ids");
        assert!(dir.join("limpeza-1.sql").is_file());
        assert_eq!(std::fs::read_to_string(dir.join("limpeza.sql")).unwrap(), "local, não importado");
        let copy = t.connections.iter().find(|c| c.name == "Financeiro (2)").unwrap();
        assert_eq!(t.script_conn_of(&dir.join("relatorio-1.sql")), Some(copy.id.clone()), "links follow the new id");

        let t = run(Resolve::Replace);
        assert_eq!(t.connections.len(), 2, "replaced in place");
        assert!(t.connections.iter().any(|c| c.id == ids[0]), "the replaced connection keeps its id");
        assert_eq!(std::fs::read_to_string(dir.join("limpeza.sql")).unwrap(), "delete from tmp");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn unsafe_script_names_change_nothing() {
        let base = temp("unsafe");
        let (s, _, ids, scripts) = sample(&base);
        let mut b = build(&s, "Origem", &everything(&ids, &scripts)).unwrap();
        b.scripts.as_mut().unwrap()[1].name = "../../fora.sql".into();
        let mut t = Store::memory();
        let r = apply(&mut t, &b, Target::NewProject { name: "X".into(), dir: base.join("x") }, &ImportSel::all(&b), &base.join("soltos"));
        assert!(r.unwrap_err().contains("Nome de script inválido"));
        assert!(t.projects.is_empty() && t.connections.is_empty() && !base.join("x").exists(), "refused before any change");
        let _ = std::fs::remove_dir_all(&base);
    }
}
