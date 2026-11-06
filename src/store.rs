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
    /// ask before switching a connection to Auto commit
    pub confirm_auto_commit: bool,
    /// seconds a query may run before it is cancelled (0 = no limit)
    pub query_timeout_secs: u32,
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
            confirm_auto_commit: true,
            query_timeout_secs: 0,
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
    /// organization folders that exist even when empty (folders with connections exist anyway)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<String>,
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
    /// organization folders of the default project that exist even when empty
    #[serde(default)]
    pub default_folders: Vec<String>,
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
        p.folders = pf.folders.clone();
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

    /// Connection a script last ran on, looked up by its path (project scripts have their own key).
    pub fn script_conn_of(&self, path: &std::path::Path) -> Option<String> {
        self.script_conns.get(&self.script_key(path)).cloned()
    }

    /// Where a new script goes: the folder of the connection's project, or `loose` for a loose script.
    pub fn new_script_dir(&self, conn: Option<&str>, loose: &std::path::Path) -> PathBuf {
        let project = conn.and_then(|c| self.connections.iter().find(|x| x.id == c)).map(|c| c.project.as_str()).filter(|p| !p.is_empty());
        match project.and_then(|id| self.projects.iter().find(|p| p.id == id)) {
            Some(p) => p.scripts_dir(),
            None => loose.to_path_buf(),
        }
    }

    /// Moves a script file into `to_dir` (a project's `scripts/` or the loose folder) and carries its connection link.
    pub fn move_script(&mut self, from: &std::path::Path, to_dir: &std::path::Path) -> Result<PathBuf, String> {
        let name = from.file_name().ok_or("Script inválido")?;
        let dest = to_dir.join(name);
        if from.parent() == Some(to_dir) {
            return Ok(dest);
        }
        if dest.exists() {
            return Err(format!("Já existe um script chamado {} no destino", name.to_string_lossy()));
        }
        std::fs::create_dir_all(to_dir).map_err(|e| e.to_string())?;
        let old_key = self.script_key(from);
        if std::fs::rename(from, &dest).is_err() {
            // another volume: copy, then remove the original
            std::fs::copy(from, &dest).map_err(|e| e.to_string())?;
            std::fs::remove_file(from).map_err(|e| e.to_string())?;
        }
        if let Some(conn) = self.script_conns.remove(&old_key) {
            let new_key = self.script_key(&dest);
            self.script_conns.insert(new_key, conn);
        }
        Ok(dest)
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
        if dir.as_os_str().is_empty() {
            return Err("Informe a pasta do projeto".into());
        }
        if dir.join(PROJECT_FILE).exists() {
            return Err("Esta pasta já é um projeto; use Abrir projeto".into());
        }
        if self.projects.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            return Err("Já existe um projeto com esse nome".into());
        }
        std::fs::create_dir_all(dir.join("scripts")).map_err(|e| e.to_string())?;
        let p = Project { id: new_project_id(), name: name.to_string(), path: dir.to_string_lossy().to_string(), folders: vec![] };
        self.projects.push(p.clone());
        self.write_project(&p)?;
        Ok(p)
    }

    /// Opens an existing folder (for instance a clone) as a project.
    pub fn open_project(&mut self, dir: &std::path::Path) -> Result<Project, String> {
        if let Some(p) = self.projects.iter().find(|p| p.dir() == dir) {
            return Ok(p.clone());
        }
        let p = Project { id: new_project_id(), name: dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), path: dir.to_string_lossy().to_string(), folders: vec![] };
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

    /// Moves the connections and scripts of the default project into a new project folder.
    ///
    /// Phase 1 creates the project, phase 2 copies the `.sql` files, phase 3 re-links connections and scripts and only
    /// then removes the originals. A failure before phase 3 undoes the copies and leaves the default project untouched.
    pub fn convert_default_project(&mut self, name: &str, dir: &std::path::Path, scripts_dir: &std::path::Path) -> Result<Project, String> {
        let existed = dir.exists();
        let p = self.create_project(name, dir)?;
        let undo = |s: &mut Store, copied: &[PathBuf], why: String| -> String {
            for f in copied {
                let _ = std::fs::remove_file(f);
            }
            s.projects.retain(|x| x.id != p.id);
            if !existed {
                let _ = std::fs::remove_dir_all(dir);
            } else {
                let _ = std::fs::remove_file(p.file());
            }
            why
        };
        let mut copied: Vec<PathBuf> = Vec::new();
        let sources: Vec<PathBuf> = std::fs::read_dir(scripts_dir)
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.is_file() && f.extension().map(|x| x == "sql").unwrap_or(false)).collect())
            .unwrap_or_default();
        let dest_dir = p.scripts_dir();
        for src in &sources {
            let dest = dest_dir.join(src.file_name().unwrap_or_default());
            if dest.exists() {
                return Err(undo(self, &copied, format!("{} já existe na pasta do projeto", dest.display())));
            }
            match std::fs::copy(src, &dest) {
                Ok(_) => copied.push(dest),
                Err(e) => return Err(undo(self, &copied, format!("Não foi possível copiar {}: {}", src.display(), e))),
            }
        }
        // phase 3: re-link, write the project file, and only then remove the originals
        let before_conns = self.connections.clone();
        let before_links = self.script_conns.clone();
        let before_folders = self.default_folders.clone();
        for c in self.connections.iter_mut().filter(|c| c.project.is_empty()) {
            c.project = p.id.clone();
            if !c.password.is_empty() {
                self.secrets.insert(c.id.clone(), c.password.clone());
            }
        }
        let plain: Vec<String> = self.script_conns.keys().filter(|k| !k.contains('/')).cloned().collect();
        for k in plain {
            if let Some(v) = self.script_conns.remove(&k) {
                self.script_conns.insert(format!("{}/{}", p.id, k), v);
            }
        }
        if let Some(pr) = self.projects.iter_mut().find(|x| x.id == p.id) {
            pr.folders = std::mem::take(&mut self.default_folders);
        }
        let pr = self.projects.iter().find(|x| x.id == p.id).cloned().unwrap_or(p.clone());
        if let Err(e) = self.write_project(&pr) {
            self.connections = before_conns;
            self.script_conns = before_links;
            self.default_folders = before_folders;
            return Err(undo(self, &copied, e));
        }
        for src in &sources {
            let _ = std::fs::remove_file(src);
        }
        Ok(pr)
    }

    /// Moves a connection to another project ("" = the default one). The id, the local password and the links of
    /// scripts (which point at the connection id) are kept; the organization folder is reset.
    pub fn move_connection(&mut self, conn_id: &str, to_project: &str) -> Result<(), String> {
        if !to_project.is_empty() && !self.projects.iter().any(|p| p.id == to_project) {
            return Err("Projeto não encontrado".into());
        }
        let c = self.connections.iter_mut().find(|c| c.id == conn_id).ok_or("Conexão não encontrada")?;
        if c.project == to_project {
            return Ok(());
        }
        if !c.password.is_empty() {
            self.secrets.insert(c.id.clone(), c.password.clone());
        }
        c.project = to_project.to_string();
        c.folder.clear();
        Ok(())
    }

    /// Organization folders of a project: the ones created explicitly plus those used by connections.
    pub fn folders_of(&self, project: &str) -> Vec<String> {
        let mut f: Vec<String> = if project.is_empty() { self.default_folders.clone() } else { self.projects.iter().find(|p| p.id == project).map(|p| p.folders.clone()).unwrap_or_default() };
        f.extend(self.connections.iter().filter(|c| c.project == project && !c.folder.is_empty()).map(|c| c.folder.clone()));
        f.sort();
        f.dedup();
        f
    }

    fn folder_list(&mut self, project: &str) -> Result<&mut Vec<String>, String> {
        if project.is_empty() {
            Ok(&mut self.default_folders)
        } else {
            self.projects.iter_mut().find(|p| p.id == project).map(|p| &mut p.folders).ok_or_else(|| "Projeto não encontrado".to_string())
        }
    }

    fn check_folder_name(&self, project: &str, name: &str, except: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.contains('/') || name.contains('\\') {
            return Err("Nome de pasta inválido".into());
        }
        if self.folders_of(project).iter().any(|f| f.eq_ignore_ascii_case(name) && Some(f.as_str()) != except) {
            return Err("Já existe uma pasta com esse nome".into());
        }
        Ok(name.to_string())
    }

    pub fn add_folder(&mut self, project: &str, name: &str) -> Result<(), String> {
        let name = self.check_folder_name(project, name, None)?;
        self.folder_list(project)?.push(name);
        Ok(())
    }

    pub fn rename_folder(&mut self, project: &str, old: &str, new: &str) -> Result<(), String> {
        let new = self.check_folder_name(project, new, Some(old))?;
        let list = self.folder_list(project)?;
        list.retain(|f| f != old);
        list.push(new.clone());
        for c in self.connections.iter_mut().filter(|c| c.project == project && c.folder == old) {
            c.folder = new.clone();
        }
        Ok(())
    }

    /// Deletes the folder only: its connections go to the root of the project.
    pub fn delete_folder(&mut self, project: &str, name: &str) -> Result<(), String> {
        self.folder_list(project)?.retain(|f| f != name);
        for c in self.connections.iter_mut().filter(|c| c.project == project && c.folder == name) {
            c.folder.clear();
        }
        Ok(())
    }

    /// Drop of a connection on a project or on one of its folders: moves it to the project (if needed) and into the folder.
    pub fn place_connection(&mut self, conn_id: &str, project: &str, folder: &str) -> Result<(), String> {
        self.move_connection(conn_id, project)?;
        self.move_to_folder(conn_id, folder)
    }

    pub fn move_to_folder(&mut self, conn_id: &str, folder: &str) -> Result<(), String> {
        let c = self.connections.iter_mut().find(|c| c.id == conn_id).ok_or("Conexão não encontrada")?;
        c.folder = folder.to_string();
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
        let pf = ProjectFile { format_version: PROJECT_FORMAT, name: p.name.clone(), connections: conns, script_conns, folders: p.folders.clone() };
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
                default_folders: self.default_folders.clone(),
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
        let fake = Project { id: "proj-fake".into(), name: "F".into(), path: base.join("fake").to_string_lossy().to_string(), folders: vec![] };
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

    fn conn(name: &str, pw: &str) -> ConnConfig {
        let mut c = pg(name, "", pw);
        c.folder = String::new();
        c
    }

    fn temp_base(tag: &str) -> PathBuf {
        let b = std::env::temp_dir().join(format!("dbr-test-{}-{}", tag, new_project_id()));
        std::fs::create_dir_all(&b).unwrap();
        b
    }

    #[test]
    fn convert_default_project_moves_connections_and_scripts() {
        let base = temp_base("convert");
        let scripts = base.join("global");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(scripts.join("a.sql"), "select 1").unwrap();
        std::fs::write(scripts.join("b.sql"), "select 2").unwrap();
        let mut s = Store::memory();
        let c = conn("Banco", "segredo");
        let cid = c.id.clone();
        s.connections.push(c);
        s.script_conns.insert("a.sql".into(), cid.clone());
        s.default_folders.push("Vazia".into());
        let p = s.convert_default_project("Meu projeto", &base.join("proj"), &scripts).unwrap();
        assert!(p.scripts_dir().join("a.sql").is_file() && p.scripts_dir().join("b.sql").is_file());
        assert!(!scripts.join("a.sql").exists() && !scripts.join("b.sql").exists(), "originals are removed at the end");
        assert_eq!(s.connections[0].project, p.id);
        assert_eq!(s.script_conns.get(&format!("{}/a.sql", p.id)), Some(&cid), "script link follows the project");
        assert!(!s.script_conns.contains_key("a.sql"));
        assert!(s.default_folders.is_empty() && s.folders_of(&p.id) == vec!["Vazia".to_string()]);
        let text = std::fs::read_to_string(p.file()).unwrap();
        assert!(text.contains("Banco") && !text.contains("segredo"), "no password in the project file");
        assert_eq!(s.secrets.get(&cid).map(|x| x.as_str()), Some(s.connections[0].password.as_str()), "the password stays on this machine");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn convert_failure_leaves_the_default_project_intact() {
        let base = temp_base("convert-fail");
        let scripts = base.join("global");
        std::fs::create_dir_all(&scripts).unwrap();
        std::fs::write(scripts.join("a.sql"), "select 1").unwrap();
        std::fs::write(scripts.join("b.sql"), "select 2").unwrap();
        // the destination already has a directory called b.sql: copying it fails after a.sql was copied
        let dir = base.join("proj");
        std::fs::create_dir_all(dir.join("scripts").join("b.sql")).unwrap();
        let mut s = Store::memory();
        s.connections.push(conn("Banco", ""));
        let before = s.connections.clone();
        let e = s.convert_default_project("X", &dir, &scripts).unwrap_err();
        assert!(e.contains("b.sql"), "{}", e);
        assert!(scripts.join("a.sql").exists() && scripts.join("b.sql").exists(), "originals untouched");
        assert!(!dir.join("scripts").join("a.sql").exists(), "the partial copy is undone");
        assert!(!dir.join(PROJECT_FILE).exists());
        assert!(s.projects.is_empty() && s.connections[0].project == before[0].project);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn move_connection_between_projects_keeps_id_password_and_links() {
        let base = temp_base("move");
        let mut s = Store::memory();
        let a = s.create_project("A", &base.join("a")).unwrap();
        let b = s.create_project("B", &base.join("b")).unwrap();
        let mut c = conn("Banco", "pw");
        c.project = a.id.clone();
        c.folder = "Pasta".into();
        let cid = c.id.clone();
        s.connections.push(c);
        s.script_conns.insert(format!("{}/x.sql", a.id), cid.clone());
        s.move_connection(&cid, &b.id).unwrap();
        assert_eq!(s.connections[0].project, b.id);
        assert!(s.connections[0].folder.is_empty());
        assert_eq!(s.script_conns.values().next(), Some(&cid), "the script keeps pointing at the same connection id");
        assert!(s.secrets.contains_key(&cid));
        s.write_project(&a).unwrap();
        s.write_project(&b).unwrap();
        assert!(!std::fs::read_to_string(a.file()).unwrap().contains("Banco"));
        assert!(std::fs::read_to_string(b.file()).unwrap().contains("Banco"));
        assert!(s.move_connection(&cid, "proj-nao-existe").is_err());
        s.move_connection(&cid, "").unwrap();
        assert!(s.connections[0].project.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn organization_folders_can_be_created_renamed_and_deleted() {
        let base = temp_base("folders");
        let mut s = Store::memory();
        let p = s.create_project("P", &base.join("p")).unwrap();
        s.add_folder(&p.id, "Prod").unwrap();
        assert!(s.add_folder(&p.id, "prod").is_err(), "same name");
        assert!(s.add_folder(&p.id, "a/b").is_err() && s.add_folder(&p.id, " ").is_err());
        let mut c = conn("Banco", "");
        c.project = p.id.clone();
        c.folder = "Prod".into();
        let cid = c.id.clone();
        s.connections.push(c);
        s.rename_folder(&p.id, "Prod", "Producao").unwrap();
        assert_eq!(s.connections[0].folder, "Producao");
        assert_eq!(s.folders_of(&p.id), vec!["Producao".to_string()]);
        let cur = s.projects[0].clone();
        s.write_project(&cur).unwrap();
        let reopened = Store::memory().open_project(&base.join("p")).unwrap();
        assert_eq!(reopened.folders, vec!["Producao".to_string()], "folders survive in the project file");
        s.delete_folder(&p.id, "Producao").unwrap();
        assert!(s.connections.iter().any(|c| c.id == cid), "the connection is kept");
        assert!(s.connections[0].folder.is_empty());
        assert!(s.folders_of(&p.id).is_empty());
        // the default project has its own list
        s.add_folder("", "Soltas").unwrap();
        assert_eq!(s.folders_of(""), vec!["Soltas".to_string()]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn create_and_open_project_validations() {
        let base = temp_base("newproj");
        let mut s = Store::memory();
        // new folder: created together with scripts/
        let p = s.create_project("Novo", &base.join("novo")).unwrap();
        assert!(p.file().is_file() && p.scripts_dir().is_dir());
        // empty path, empty name, name already used, folder that already is a project
        assert!(s.create_project("X", std::path::Path::new("")).unwrap_err().contains("pasta"));
        assert!(s.create_project("  ", &base.join("a")).is_err());
        assert!(s.create_project("novo", &base.join("b")).unwrap_err().contains("nome"));
        assert!(s.create_project("Outro", &base.join("novo")).unwrap_err().contains("Abrir projeto"));
        // a path below a regular file cannot be created
        std::fs::write(base.join("arquivo"), "x").unwrap();
        assert!(s.create_project("Sem permissão", &base.join("arquivo").join("sub")).is_err());
        assert_eq!(s.projects.len(), 1, "failures register nothing");
        // opening: a folder without a project file is refused and nothing changes
        std::fs::create_dir_all(base.join("vazia")).unwrap();
        assert!(Store::memory().open_project(&base.join("vazia")).is_err());
        // opening a clone: connections come without password
        let mut other = Store::memory();
        let opened = other.open_project(&base.join("novo")).unwrap();
        assert_eq!(opened.dir(), base.join("novo"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn project_folder_can_be_versioned_without_secrets() {
        if !crate::git::available() {
            return;
        }
        std::env::set_var("GIT_AUTHOR_NAME", "Teste");
        std::env::set_var("GIT_AUTHOR_EMAIL", "teste@exemplo.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Teste");
        std::env::set_var("GIT_COMMITTER_EMAIL", "teste@exemplo.com");
        let base = temp_base("projgit");
        let mut s = Store::memory();
        let p = s.create_project("Git", &base.join("g")).unwrap();
        let mut c = conn("Banco", "segredo-local");
        c.project = p.id.clone();
        s.connections.push(c);
        s.write_project(&p).unwrap();
        std::fs::write(p.scripts_dir().join("a.sql"), "select 1").unwrap();
        assert!(!crate::git::is_repo(&p.dir()) && crate::git::init(&p.dir()).ok);
        let changed: Vec<String> = crate::git::changed_files(&p.dir()).into_iter().map(|c| c.1).collect();
        assert!(changed.contains(&PROJECT_FILE.to_string()) && changed.iter().any(|f| f.ends_with("a.sql")), "{:?}", changed);
        assert!(crate::git::commit_all(&p.dir(), "primeiro").ok);
        assert!(crate::git::changed_files(&p.dir()).is_empty());
        let tracked = crate::git::run(&p.dir(), &["grep", "-c", "segredo-local"]);
        assert!(!tracked.ok, "the password is not in the repository");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn new_scripts_go_to_the_project_of_the_connection_or_loose() {
        let base = temp_base("newscript");
        let loose = base.join("soltos");
        let mut s = Store::memory();
        let p = s.create_project("P", &base.join("p")).unwrap();
        let mut inside = conn("Dentro", "");
        inside.project = p.id.clone();
        let outside = conn("Fora", "");
        let (iid, oid) = (inside.id.clone(), outside.id.clone());
        s.connections.push(inside);
        s.connections.push(outside);
        assert_eq!(s.new_script_dir(Some(&iid), &loose), p.scripts_dir());
        assert_eq!(s.new_script_dir(Some(&oid), &loose), loose, "a connection of the default project");
        assert_eq!(s.new_script_dir(None, &loose), loose, "no connection");
        assert_eq!(s.new_script_dir(Some("nao-existe"), &loose), loose);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn moving_a_script_between_loose_and_project() {
        let base = temp_base("movescript");
        let loose = base.join("soltos");
        std::fs::create_dir_all(&loose).unwrap();
        let mut s = Store::memory();
        let p = s.create_project("P", &base.join("p")).unwrap();
        let c = conn("Banco", "");
        let cid = c.id.clone();
        s.connections.push(c);
        let f = loose.join("rel.sql");
        std::fs::write(&f, "select 1").unwrap();
        s.script_conns.insert("rel.sql".into(), cid.clone());
        // loose -> project: the link moves to the project's key
        let moved = s.move_script(&f, &p.scripts_dir()).unwrap();
        assert_eq!(moved, p.scripts_dir().join("rel.sql"));
        assert!(!f.exists() && moved.is_file());
        assert_eq!(s.script_conn_of(&moved), Some(cid.clone()));
        assert!(!s.script_conns.contains_key("rel.sql"));
        // conflict: nothing is moved and the link stays
        std::fs::write(&f, "outro").unwrap();
        let e = s.move_script(&moved, &loose).unwrap_err();
        assert!(e.contains("rel.sql"), "{}", e);
        assert!(moved.is_file() && s.script_conn_of(&moved) == Some(cid.clone()));
        std::fs::remove_file(&f).unwrap();
        // project -> loose
        let back = s.move_script(&moved, &loose).unwrap();
        assert_eq!(s.script_conn_of(&back), Some(cid));
        assert!(!moved.exists());
        // same folder: no-op
        assert_eq!(s.move_script(&back, &loose).unwrap(), back);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn dropping_a_connection_on_a_project_or_folder() {
        let base = temp_base("drop");
        let mut s = Store::memory();
        let a = s.create_project("A", &base.join("a")).unwrap();
        let b = s.create_project("B", &base.join("b")).unwrap();
        s.add_folder(&b.id, "Prod").unwrap();
        let mut c = conn("Banco", "");
        c.project = a.id.clone();
        c.folder = "Velha".into();
        let cid = c.id.clone();
        s.connections.push(c);
        s.place_connection(&cid, &b.id, "Prod").unwrap();
        assert_eq!((s.connections[0].project.as_str(), s.connections[0].folder.as_str()), (b.id.as_str(), "Prod"));
        s.place_connection(&cid, &b.id, "").unwrap();
        assert!(s.connections[0].folder.is_empty(), "dropped on the project row: root of the project");
        s.place_connection(&cid, "", "").unwrap();
        assert!(s.connections[0].project.is_empty(), "dropped on the default project");
        assert!(s.place_connection(&cid, "nao-existe", "").is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn project_file_without_folders_still_loads() {
        let base = temp_base("oldfmt");
        std::fs::write(base.join(PROJECT_FILE), r#"{"format_version":1,"name":"Velho","connections":[]}"#).unwrap();
        let p = Store::memory().open_project(&base).unwrap();
        assert!(p.folders.is_empty());
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
