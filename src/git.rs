//! Thin wrapper over the `git` command line, used to version the folder of saved scripts.
//! Every call runs `git` with fixed arguments in the scripts folder (no shell involved).
use std::path::Path;
use std::process::Command;

pub struct Out {
    pub ok: bool,
    pub text: String,
}

pub fn run(dir: &Path, args: &[&str]) -> Out {
    match Command::new("git").args(args).current_dir(dir).env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C").output() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).to_string();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            Out { ok: o.status.success(), text }
        }
        Err(e) => Out { ok: false, text: format!("Não foi possível executar o git ({}). Instale o git para versionar os scripts.", e) },
    }
}

pub fn available() -> bool {
    Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// True when `dir` is inside a git work tree.
pub fn is_repo(dir: &Path) -> bool {
    let o = run(dir, &["rev-parse", "--is-inside-work-tree"]);
    o.ok && o.text.trim() == "true"
}

pub fn init(dir: &Path) -> Out {
    let _ = std::fs::create_dir_all(dir);
    run(dir, &["init"])
}

pub fn status(dir: &Path) -> Out {
    run(dir, &["status", "--short", "--branch"])
}

pub fn log(dir: &Path) -> Out {
    run(dir, &["log", "--oneline", "--decorate", "-n", "40"])
}

/// Stages every script and commits them.
pub fn commit_all(dir: &Path, message: &str) -> Out {
    let add = run(dir, &["add", "-A", "."]);
    if !add.ok {
        return add;
    }
    let msg = if message.trim().is_empty() { "Atualiza scripts" } else { message.trim() };
    let c = run(dir, &["commit", "-m", msg]);
    if !c.ok && c.text.contains("nothing to commit") {
        return Out { ok: true, text: "Nada para registrar: nenhum script mudou desde o último commit.".into() };
    }
    if !c.ok && (c.text.contains("Please tell me who you are") || c.text.contains("empty ident")) {
        return Out { ok: false, text: format!("O git precisa saber quem você é. Rode no terminal:\n  git config --global user.name \"Seu Nome\"\n  git config --global user.email \"voce@exemplo.com\"\n\n{}", c.text) };
    }
    c
}

/// (status code, file) pairs from `git status --porcelain`, paths relative to `dir`.
pub fn changed_files(dir: &Path) -> Vec<(String, String)> {
    let o = run(dir, &["status", "--porcelain", "--untracked-files=all", "."]);
    if !o.ok {
        return vec![];
    }
    o.text
        .lines()
        .filter(|l| l.len() > 3)
        .map(|l| (l[..2].trim().to_string(), l[3..].trim().trim_matches('"').to_string()))
        .collect()
}

pub fn branch(dir: &Path) -> Option<String> {
    let o = run(dir, &["rev-parse", "--abbrev-ref", "HEAD"]);
    if o.ok {
        Some(o.text.trim().to_string())
    } else {
        // a fresh repository has no commit yet
        let o = run(dir, &["symbolic-ref", "--short", "HEAD"]);
        if o.ok {
            Some(o.text.trim().to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_commit_status_log() {
        if !available() {
            return;
        }
        let dir = std::env::temp_dir().join("dbr-git-test");
        let _ = std::fs::create_dir_all(&dir);
        // a clean folder for the test
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            if e.path().is_dir() {
                let _ = std::fs::remove_dir_all(e.path());
            } else {
                let _ = std::fs::remove_file(e.path());
            }
        }
        std::env::set_var("GIT_AUTHOR_NAME", "Teste");
        std::env::set_var("GIT_AUTHOR_EMAIL", "teste@exemplo.com");
        std::env::set_var("GIT_COMMITTER_NAME", "Teste");
        std::env::set_var("GIT_COMMITTER_EMAIL", "teste@exemplo.com");
        assert!(!is_repo(&dir));
        assert!(init(&dir).ok);
        assert!(is_repo(&dir));
        std::fs::write(dir.join("Script.sql"), "select 1;\n").unwrap();
        let ch = changed_files(&dir);
        assert_eq!(ch, vec![("??".to_string(), "Script.sql".to_string())]);
        let c = commit_all(&dir, "primeiro script");
        assert!(c.ok, "{}", c.text);
        assert!(changed_files(&dir).is_empty());
        assert!(commit_all(&dir, "nada").text.contains("Nada para registrar"));
        std::fs::write(dir.join("Script.sql"), "select 2;\n").unwrap();
        assert_eq!(changed_files(&dir)[0].0, "M");
        assert!(commit_all(&dir, "muda").ok);
        let l = log(&dir);
        assert!(l.text.contains("muda") && l.text.contains("primeiro script"), "{}", l.text);
        assert!(branch(&dir).is_some());
        assert!(status(&dir).ok);
    }
}
