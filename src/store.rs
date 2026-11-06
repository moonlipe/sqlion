//! Settings, saved connections and query history stored as small JSON files.
use crate::db::ConnConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn new_id() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("conn-{:x}", t)
}

const KEY: &[u8] = b"sqlion-local-key";

pub fn obfuscate(p: &str) -> String {
    p.bytes().enumerate().map(|(i, b)| format!("{:02x}", b ^ KEY[i % KEY.len()])).collect()
}

pub fn deobfuscate(h: &str) -> String {
    let bytes: Vec<u8> = (0..h.len() / 2)
        .filter_map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok())
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    let d = base.join("sqlion");
    let _ = std::fs::create_dir_all(&d);
    d
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Prefs {
    pub dark: bool,
    pub font_size: f32,
    pub max_rows: usize,       // resultset.maxrows = 200
    pub history_days: u32,     // 90
    pub history_page: usize,   // 200
    pub default_auto_commit: bool,
    pub keyword_upper: bool,
    pub statement_delimiter: String, // ";"
    pub params_enabled: bool,  // true
    pub vars_enabled: bool,    // true
    pub confirm_exit: bool,
    pub show_value_panel: bool,
    pub null_text: String,
    /// folder of the saved scripts; empty = <config>/scripts. Point it inside a git repository to version the scripts.
    pub scripts_dir: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            dark: true,
            font_size: 14.0,
            max_rows: crate::constants::RESULT_SET_MAX_ROWS,
            history_days: crate::constants::HISTORY_DAYS,
            history_page: crate::constants::HISTORY_PAGE,
            default_auto_commit: true,
            keyword_upper: true,
            statement_delimiter: crate::constants::STATEMENT_DELIMITER.into(),
            params_enabled: true,
            vars_enabled: true,
            confirm_exit: false,
            show_value_panel: false,
            null_text: crate::constants::NULL_LABEL.into(),
            scripts_dir: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub when: i64,
    pub conn: String,
    pub sql: String,
    pub ms: u128,
    pub rows: String,
    pub ok: bool,
}

/// A project that lives in a folder: `sqlion-project.json` plus `scripts/*.sql`. The default project is not listed here.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
}

pub const PROJECT_FILE: &str = "sqlion-project.json";
pub const PROJECT_FORMAT: u32 = 1;

impl Project {
    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
    pub fn file(&self) -> PathBuf {
        self.dir().join(PROJECT_FILE)
    }
    pub fn scripts_dir(&self) -> PathBuf {
        self.dir().join("scripts")
    }
}

/// What goes into the project folder (and so into git): no passwords, history or tabs.
#[derive(Serialize, Deserialize, Default)]
pub struct ProjectFile {
    pub format_version: u32,
    pub name: String,
    #[serde(default)]
    pub connections: Vec<ConnConfig>,
    /// script file name -> id of the connection it last ran on
    #[serde(default)]
    pub script_conns: std::collections::BTreeMap<String, String>,
}

pub fn new_project_id() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("proj-{:x}", t)
}

pub fn secrets_path() -> PathBuf {
    config_dir().join("secrets.json")
}

#[derive(Serialize, Deserialize, Default)]
pub struct Store {
    /// opened projects that live in folders (the default project is implicit)
    #[serde(default)]
    pub projects: Vec<Project>,
    /// passwords of project connections (obfuscated), by connection id; they never go into the project folder
    #[serde(skip)]
    pub secrets: std::collections::HashMap<String, String>,
    #[serde(skip)]
    pub secrets_file: Option<PathBuf>,
    pub connections: Vec<ConnConfig>,
    pub history: Vec<HistoryEntry>,
    pub scripts: Vec<(String, String)>,
    /// script file name -> id of the connection it was last executed on
    #[serde(default)]
    pub script_conns: std::collections::HashMap<String, String>,
    /// connection of the most recent execution anywhere
    #[serde(default)]
    pub last_exec_conn: Option<String>,
    #[serde(default)]
    pub prefs: Prefs,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Store {
    pub fn load() -> Store {
        let path = config_dir().join("workspace.json");
        let mut s = Store::load_from(&path, &secrets_path());
        if std::env::var("DBR_DARK").is_ok() {
            s.prefs.dark = true;
        }
        let cutoff = chrono::Utc::now().timestamp() - s.prefs.history_days as i64 * 86400;
        s.history.retain(|h| h.when >= cutoff);
        s
    }

    /// Loads the workspace file, the local secrets and the connections of every opened project.
    pub fn load_from(path: &std::path::Path, secrets: &std::path::Path) -> Store {
        let mut s: Store = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        s.path = Some(path.to_path_buf());
        s.secrets_file = Some(secrets.to_path_buf());
        s.secrets = std::fs::read_to_string(secrets).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let projects = s.projects.clone();
        s.projects.clear();
        for p in projects {
            let _ = s.attach_project(p);
        }
        s
    }

    /// Reads the project folder and merges its connections; fails (without changing anything) when the file is unreadable or from the future.
    pub fn attach_project(&mut self, mut p: Project) -> Result<(), String> {
        let text = std::fs::read_to_string(p.file()).map_err(|e| format!("Não foi possível ler {}: {}", p.file().display(), e))?;
        let pf: ProjectFile = serde_json::from_str(&text).map_err(|e| format!("Arquivo de projeto inválido: {}", e))?;
        if pf.format_version == 0 || pf.format_version > PROJECT_FORMAT {
            return Err(format!("Versão de formato desconhecida ({}); atualize o Sqlion", pf.format_version));
        }
        if !pf.name.is_empty() {
            p.name = pf.name.clone();
        }
        self.connections.retain(|c| c.project != p.id);
        for mut c in pf.connections {
            c.project = p.id.clone();
            if let Some(pw) = self.secrets.get(&c.id) {
                c.password = pw.clone();
            }
            self.connections.push(c);
        }
        for (name, conn) in pf.script_conns {
            self.script_conns.insert(format!("{}/{}", p.id, name), conn);
        }
        self.projects.retain(|x| x.id != p.id);
        self.projects.push(p);
        Ok(())
    }

    /// Key under which the connection of a script is remembered: the file name, prefixed by the project id outside the default project.
    pub fn script_key(&self, path: &std::path::Path) -> String {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match self.projects.iter().find(|p| path.starts_with(p.scripts_dir())) {
            Some(p) => format!("{}/{}", p.id, name),
            None => name,
        }
    }

    pub fn project_of_script(&self, path: &std::path::Path) -> Option<&Project> {
        self.projects.iter().find(|p| path.starts_with(p.scripts_dir()))
    }

    /// Creates the folder structure of a new project and registers it.
    pub fn create_project(&mut self, name: &str, dir: &std::path::Path) -> Result<Project, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Informe o nome do projeto".into());
        }
        if dir.join(PROJECT_FILE).exists() {
            return Err("Esta pasta já é um projeto; use Abrir projeto".into());
        }
        std::fs::create_dir_all(dir.join("scripts")).map_err(|e| e.to_string())?;
        let p = Project { id: new_project_id(), name: name.to_string(), path: dir.to_string_lossy().to_string() };
        self.projects.push(p.clone());
        self.write_project(&p)?;
        Ok(p)
    }

    /// Opens an existing folder (for instance a clone) as a project.
    pub fn open_project(&mut self, dir: &std::path::Path) -> Result<Project, String> {
        if let Some(p) = self.projects.iter().find(|p| p.dir() == dir) {
            return Ok(p.clone());
        }
        let p = Project { id: new_project_id(), name: dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), path: dir.to_string_lossy().to_string() };
        self.attach_project(p.clone())?;
        let _ = std::fs::create_dir_all(p.scripts_dir());
        Ok(self.projects.iter().find(|x| x.id == p.id).cloned().unwrap_or(p))
    }

    /// Takes the project out of the workspace; the folder stays intact.
    pub fn close_project(&mut self, id: &str) {
        self.connections.retain(|c| c.project != id);
        self.projects.retain(|p| p.id != id);
        self.script_conns.retain(|k, _| !k.starts_with(&format!("{}/", id)));
    }

    /// Renames a project (name only; the folder and the connection ids stay).
    pub fn rename_project(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Informe o nome do projeto".into());
        }
        if self.projects.iter().any(|p| p.id != id && p.name.eq_ignore_ascii_case(name)) {
            return Err("Já existe um projeto com esse nome".into());
        }
        let p = self.projects.iter_mut().find(|p| p.id == id).ok_or("Projeto não encontrado")?;
        p.name = name.to_string();
        let p = p.clone();
        self.write_project(&p)
    }

    /// (connections, scripts) that live in the project.
    pub fn project_counts(&self, id: &str) -> (usize, usize) {
        let conns = self.connections.iter().filter(|c| c.project == id).count();
        let scripts = self
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| std::fs::read_dir(p.scripts_dir()).map(|d| d.filter_map(|e| e.ok()).filter(|e| e.path().extension().map(|x| x == "sql").unwrap_or(false)).count()).unwrap_or(0))
            .unwrap_or(0);
        (conns, scripts)
    }

    /// Deletes the project folder from disk and closes the project. Refuses folders that are not projects, roots and the home folder.
    pub fn delete_project_folder(&mut self, id: &str) -> Result<(), String> {
        let p = self.projects.iter().find(|p| p.id == id).cloned().ok_or("Projeto não encontrado")?;
        let dir = p.dir();
        if !p.file().is_file() {
            return Err("A pasta não tem o arquivo do projeto; nada foi apagado".into());
        }
        let canon = dir.canonicalize().map_err(|e| e.to_string())?;
        let home = std::env::var_os("HOME").map(PathBuf::from).and_then(|h| h.canonicalize().ok());
        if canon.parent().is_none() || home.as_ref().map(|h| h.starts_with(&canon)).unwrap_or(false) {
            return Err("Por segurança, esta pasta não pode ser excluída".into());
        }
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Não foi possível excluir {}: {}", dir.display(), e))?;
        self.close_project(id);
        Ok(())
    }

    /// Writes `sqlion-project.json`: connections without password, in a stable order (by id) so that diffs and merges stay readable.
    pub fn write_project(&self, p: &Project) -> Result<(), String> {
        let mut conns: Vec<ConnConfig> = self.connections.iter().filter(|c| c.project == p.id).cloned().collect();
        conns.sort_by(|a, b| a.id.cmp(&b.id));
        for c in conns.iter_mut() {
            c.password.clear();
            c.project.clear();
        }
        let prefix = format!("{}/", p.id);
        let script_conns = self.script_conns.iter().filter_map(|(k, v)| k.strip_prefix(&prefix).map(|n| (n.to_string(), v.clone()))).collect();
        let pf = ProjectFile { format_version: PROJECT_FORMAT, name: p.name.clone(), connections: conns, script_conns };
        let text = serde_json::to_string_pretty(&pf).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(p.dir()).map_err(|e| e.to_string())?;
        std::fs::write(p.file(), text + "\n").map_err(|e| e.to_string())
    }

    pub fn set_secret(&mut self, conn_id: &str, password: &str) {
        self.secrets.insert(conn_id.to_string(), password.to_string());
        if let Some(c) = self.connections.iter_mut().find(|c| c.id == conn_id) {
            c.password = password.to_string();
        }
    }

    /// Where the scripts are saved (`~/` is expanded).
    pub fn scripts_dir(&self) -> PathBuf {
        let p = self.prefs.scripts_dir.trim();
        if p.is_empty() {
            return config_dir().join("scripts");
        }
        if let Some(rest) = p.strip_prefix("~/") {
            if let Some(h) = std::env::var_os("HOME") {
                return PathBuf::from(h).join(rest);
            }
        }
        PathBuf::from(p)
    }

    pub fn memory() -> Store {
        Store::default()
    }
    pub fn save(&self) {
        if let Some(p) = &self.path {
            // connections of folder projects live in their project file, not here
            let mut w = Store {
                projects: self.projects.clone(),
                connections: self.connections.iter().filter(|c| c.project.is_empty()).cloned().collect(),
                history: self.history.clone(),
                scripts: self.scripts.clone(),
                script_conns: self.script_conns.iter().filter(|(k, _)| !self.projects.iter().any(|p| k.starts_with(&format!("{}/", p.id)))).map(|(k, v)| (k.clone(), v.clone())).collect(),
                last_exec_conn: self.last_exec_conn.clone(),
                prefs: self.prefs.clone(),
                ..Default::default()
            };
            w.path = None;
            if let Ok(t) = serde_json::to_string_pretty(&w) {
                let _ = std::fs::write(p, t);
            }
            for pr in &self.projects {
                let _ = self.write_project(pr);
            }
            if let Some(sp) = &self.secrets_file {
                // only the passwords of project connections are kept here
                let mut all = self.secrets.clone();
                for c in self.connections.iter().filter(|c| !c.project.is_empty() && !c.password.is_empty()) {
                    all.insert(c.id.clone(), c.password.clone());
                }
                let ids: std::collections::HashSet<&String> = self.connections.iter().filter(|c| !c.project.is_empty()).map(|c| &c.id).collect();
                let secrets: std::collections::HashMap<&String, &String> = all.iter().filter(|(k, _)| ids.contains(k)).collect();
                if let Ok(t) = serde_json::to_string_pretty(&secrets) {
                    let _ = std::fs::write(sp, t);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = std::fs::set_permissions(sp, std::fs::Permissions::from_mode(0o600));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Kind;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sqlion-store-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn store_in(d: &std::path::Path) -> Store {
        Store::load_from(&d.join("workspace.json"), &d.join("secrets.json"))
    }

    fn pg(name: &str, project: &str, pw: &str) -> ConnConfig {
        let mut c = ConnConfig::new(Kind::Postgres);
        c.name = name.into();
        c.project = project.into();
        c.password = obfuscate(pw);
        c
    }

    #[test]
    fn project_file_has_connections_but_no_secrets() {
        let cfg = tmp("nosecret");
        let repo = tmp("nosecret-repo");
        let mut s = store_in(&cfg);
        let p = s.create_project("Financeiro", &repo).unwrap();
        s.connections.push(pg("Prod", &p.id, "s3nh4-secreta"));
        s.script_conns.insert(format!("{}/Relatorio.sql", p.id), s.connections[0].id.clone());
        s.save();
        let file = std::fs::read_to_string(repo.join(PROJECT_FILE)).unwrap();
        assert!(file.contains("\"format_version\": 1") && file.contains("Prod") && file.contains("Relatorio.sql"));
        assert!(!file.contains(&obfuscate("s3nh4-secreta")), "no password in the project folder");
        assert!(!file.contains("s3nh4-secreta"));
        assert!(repo.join("scripts").is_dir());
        // the password lives only in the local secrets file
        let sec = std::fs::read_to_string(cfg.join("secrets.json")).unwrap();
        assert!(sec.contains(&obfuscate("s3nh4-secreta")));
        // and the workspace file does not hold the project connection
        let ws = std::fs::read_to_string(cfg.join("workspace.json")).unwrap();
        assert!(!ws.contains("Prod"));
    }

    #[test]
    fn project_round_trip_restores_connections_and_password() {
        let cfg = tmp("rt");
        let repo = tmp("rt-repo");
        let mut s = store_in(&cfg);
        let p = s.create_project("Projeto", &repo).unwrap();
        let c = pg("Dev", &p.id, "abc");
        let id = c.id.clone();
        s.connections.push(c);
        s.save();
        let s2 = store_in(&cfg);
        assert_eq!(s2.projects.len(), 1);
        let c2 = s2.connections.iter().find(|c| c.id == id).expect("connection restored");
        assert_eq!(c2.project, s2.projects[0].id);
        assert_eq!(deobfuscate(&c2.password), "abc", "password comes back from the local secrets");
    }

    #[test]
    fn clone_without_secrets_has_empty_passwords_and_stable_ids() {
        let cfg = tmp("clone-a");
        let repo = tmp("clone-repo");
        let mut a = store_in(&cfg);
        let p = a.create_project("P", &repo).unwrap();
        let c = pg("Prod", &p.id, "x");
        let id = c.id.clone();
        a.connections.push(c);
        a.save();
        // another machine: same folder, empty configuration
        let cfg2 = tmp("clone-b");
        let mut b = store_in(&cfg2);
        let opened = b.open_project(&repo).unwrap();
        let c2 = b.connections.iter().find(|c| c.project == opened.id).unwrap();
        assert_eq!(c2.id, id, "the connection id is the same on both machines");
        assert!(c2.password.is_empty());
        b.set_secret(&id, &obfuscate("minha"));
        b.save();
        assert!(!std::fs::read_to_string(repo.join(PROJECT_FILE)).unwrap().contains("minha"));
    }

    #[test]
    fn project_file_is_written_in_stable_order() {
        let cfg = tmp("order");
        let repo = tmp("order-repo");
        let mut s = store_in(&cfg);
        let p = s.create_project("P", &repo).unwrap();
        let (mut a, mut b) = (pg("B", &p.id, ""), pg("A", &p.id, ""));
        a.id = "conn-2".into();
        b.id = "conn-1".into();
        s.connections.push(a);
        s.connections.push(b);
        s.save();
        let first = std::fs::read_to_string(repo.join(PROJECT_FILE)).unwrap();
        s.connections.reverse();
        s.save();
        assert_eq!(first, std::fs::read_to_string(repo.join(PROJECT_FILE)).unwrap());
        assert!(first.find("conn-1").unwrap() < first.find("conn-2").unwrap());
    }

    #[test]
    fn old_workspace_loads_into_the_default_project() {
        let cfg = tmp("old");
        let old = r#"{"connections":[{"id":"conn-1","name":"Velha","kind":"Postgres","host":"h","port":5432,"database":"d","user":"u","password":"","auto_commit":false}],"history":[],"scripts":[],"script_conns":{"Script.sql":"conn-1"},"prefs":{}}"#;
        std::fs::write(cfg.join("workspace.json"), old).unwrap();
        let s = store_in(&cfg);
        assert_eq!(s.connections.len(), 1);
        assert!(s.connections[0].project.is_empty(), "default project");
        assert!(s.projects.is_empty());
        assert_eq!(s.script_conns.get("Script.sql").map(String::as_str), Some("conn-1"));
        assert_eq!(s.connections[0].mode(), crate::db::TxMode::Manual);
        // the default project still keeps its scripts in the old folder
        assert_eq!(s.script_key(&s.scripts_dir().join("Script.sql")), "Script.sql");
    }

    #[test]
    fn scripts_of_a_project_get_their_own_key() {
        let cfg = tmp("key");
        let repo = tmp("key-repo");
        let mut s = store_in(&cfg);
        let p = s.create_project("P", &repo).unwrap();
        let path = p.scripts_dir().join("A.sql");
        assert_eq!(s.script_key(&path), format!("{}/A.sql", p.id));
        assert_eq!(s.project_of_script(&path).map(|x| x.id.clone()), Some(p.id));
    }

    #[test]
    fn unknown_or_broken_project_files_are_refused_without_changes() {
        let cfg = tmp("bad");
        let repo = tmp("bad-repo");
        let mut s = store_in(&cfg);
        std::fs::write(repo.join(PROJECT_FILE), "{ not json").unwrap();
        assert!(s.open_project(&repo).is_err());
        std::fs::write(repo.join(PROJECT_FILE), r#"{"format_version": 99, "name": "x"}"#).unwrap();
        let e = s.open_project(&repo).unwrap_err();
        assert!(e.contains("99"), "{}", e);
        assert!(s.projects.is_empty() && s.connections.is_empty());
    }

    #[test]
    fn rename_and_delete_project() {
        let base = std::env::temp_dir().join(format!("dbr-test-pm-{}", new_project_id()));
        let mut s = Store::memory();
        let a = s.create_project("Alfa", &base.join("a")).unwrap();
        let b = s.create_project("Beta", &base.join("b")).unwrap();
        assert!(s.rename_project(&a.id, " ").is_err());
        assert!(s.rename_project(&a.id, "beta").is_err(), "name already used");
        s.rename_project(&a.id, "Gama").unwrap();
        assert_eq!(s.projects.iter().find(|p| p.id == a.id).unwrap().name, "Gama");
        let text = std::fs::read_to_string(a.file()).unwrap();
        assert!(text.contains("\"Gama\""), "the name is written to the project file");
        std::fs::write(a.scripts_dir().join("x.sql"), "select 1").unwrap();
        assert_eq!(s.project_counts(&a.id), (0, 1));
        // a folder without a project file is never deleted
        let fake = Project { id: "proj-fake".into(), name: "F".into(), path: base.join("fake").to_string_lossy().to_string() };
        std::fs::create_dir_all(&fake.path).unwrap();
        s.projects.push(fake.clone());
        assert!(s.delete_project_folder("proj-fake").is_err());
        assert!(std::path::Path::new(&fake.path).exists());
        s.projects.retain(|p| p.id != "proj-fake");
        s.delete_project_folder(&a.id).unwrap();
        assert!(!a.dir().exists());
        assert!(s.projects.iter().all(|p| p.id != a.id) && s.projects.iter().any(|p| p.id == b.id));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn closing_a_project_keeps_the_folder() {
        let cfg = tmp("close");
        let repo = tmp("close-repo");
        let mut s = store_in(&cfg);
        let p = s.create_project("P", &repo).unwrap();
        s.connections.push(pg("X", &p.id, ""));
        s.save();
        s.close_project(&p.id);
        s.save();
        assert!(s.projects.is_empty() && s.connections.is_empty());
        assert!(repo.join(PROJECT_FILE).exists(), "files stay");
    }
}
