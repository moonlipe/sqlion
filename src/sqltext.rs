//! SQL text helpers: statement splitting, tokenising (for highlighting), formatting, parameters.
use std::collections::HashSet;

pub const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "CREATE", "TABLE", "VIEW",
    "INDEX", "UNIQUE", "DROP", "ALTER", "ADD", "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "CONSTRAINT", "DEFAULT", "CHECK", "JOIN",
    "INNER", "LEFT", "RIGHT", "FULL", "OUTER", "CROSS", "ON", "USING", "AS", "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET", "UNION", "ALL",
    "DISTINCT", "EXISTS", "IN", "BETWEEN", "LIKE", "ILIKE", "IS", "CASE", "WHEN", "THEN", "ELSE", "END", "ASC", "DESC", "COUNT", "SUM", "AVG",
    "MIN", "MAX", "BEGIN", "COMMIT", "ROLLBACK", "TRANSACTION", "EXPLAIN", "ANALYZE", "WITH", "RECURSIVE", "INTEGER", "INT", "TEXT", "VARCHAR",
    "CHAR", "REAL", "FLOAT", "DOUBLE", "NUMERIC", "DECIMAL", "BOOLEAN", "DATE", "TIMESTAMP", "TIME", "BLOB", "TRIGGER", "PROCEDURE", "FUNCTION",
    "RETURNING", "TRUNCATE", "GRANT", "REVOKE", "PRAGMA", "SHOW", "DESCRIBE", "USE", "DATABASE", "SCHEMA", "IF", "REPLACE", "CAST", "COALESCE",
    "EXCEPT", "INTERSECT", "OVER", "PARTITION", "AUTOINCREMENT", "AUTO_INCREMENT", "SERIAL", "TRUE", "FALSE", "CASCADE", "RESTRICT", "TEMP",
    "TEMPORARY", "VACUUM", "ATTACH", "DETACH", "NATURAL", "ESCAPE", "ANY", "SOME", "FETCH", "FIRST", "ROWS", "ONLY", "WINDOW", "LATERAL",
];

pub fn is_keyword(w: &str) -> bool {
    let u = w.to_uppercase();
    KEYWORDS.iter().any(|k| *k == u)
}

pub fn first_word(sql: &str) -> String {
    let mut s = sql.trim_start();
    loop {
        if s.starts_with("--") {
            s = s.split_once('\n').map(|x| x.1).unwrap_or("").trim_start();
        } else if s.starts_with("/*") {
            s = s.split_once("*/").map(|x| x.1).unwrap_or("").trim_start();
        } else {
            break;
        }
    }
    s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tok {
    Keyword,
    Str,
    Number,
    Comment,
    Ident,
    Op,
    Space,
    Param,
}

/// Length of a dollar-quote opening tag such as `$$` or `$body$` at the start of `b`.
fn dollar_tag_len(b: &[u8]) -> Option<usize> {
    if b.first() != Some(&b'$') {
        return None;
    }
    let mut j = 1;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    if j < b.len() && b[j] == b'$' && !(j > 1 && b[1].is_ascii_digit()) {
        Some(j + 1)
    } else {
        None
    }
}

/// Tokenise into (kind, byte range).
pub fn tokenize(s: &str) -> Vec<(Tok, usize, usize)> {
    let b = s.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < n {
        let st = i;
        let c = b[i];
        let kind;
        if c.is_ascii_whitespace() {
            while i < n && b[i].is_ascii_whitespace() {
                i += 1;
            }
            kind = Tok::Space;
        } else if c == b'-' && i + 1 < n && b[i + 1] == b'-' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            kind = Tok::Comment;
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(n);
            kind = Tok::Comment;
        } else if c == b'\'' {
            i += 1;
            while i < n {
                if b[i] == b'\'' {
                    if i + 1 < n && b[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            kind = Tok::Str;
        } else if c == b'"' || c == b'`' {
            i += 1;
            while i < n && b[i] != c {
                i += 1;
            }
            i = (i + 1).min(n);
            kind = Tok::Ident;
        } else if c == b'$' && dollar_tag_len(&b[i..]).is_some() {
            let tl = dollar_tag_len(&b[i..]).unwrap();
            let tag = &s[i..i + tl];
            match s[i + tl..].find(tag) {
                Some(p) => i = i + tl + p + tl,
                None => i = n,
            }
            kind = Tok::Str;
        } else if c.is_ascii_digit() {
            while i < n && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            kind = Tok::Number;
        } else if c == b':' && i + 1 < n && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'_') {
            i += 1;
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            kind = Tok::Param;
        } else if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            while i < n && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 0x80) {
                i += 1;
            }
            kind = if is_keyword(&s[st..i]) { Tok::Keyword } else { Tok::Ident };
        } else {
            // advance one full char
            i += 1;
            while i < n && !s.is_char_boundary(i) {
                i += 1;
            }
            kind = Tok::Op;
        }
        out.push((kind, st, i));
    }
    out
}

/// Split a script into statements: `;` delimiter (SQLConstants.java:31), quote/comment aware,
/// blank line also ends a statement (BLANK_LINE_AND_SEPARATOR, ModelPreferences.java:332).
/// Returns byte ranges (trimmed, without the trailing delimiter).
pub fn split_statements(s: &str, delim: &str) -> Vec<(usize, usize)> {
    let toks = tokenize(s);
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_end = 0;
    let mut depth_block = 0i32;
    let mut starts_create = false;
    let mut first_kw: Option<String> = None;
    let delim_c = delim.chars().next().unwrap_or(';');
    let mut dollar_open = false;
    let mut first_tok = true;
    let mut ctl = false; // the statement is a control command (@set, @echo...): it ends at the end of the line
    for (k, a, b) in toks {
        let t = &s[a..b];
        match k {
            Tok::Space => {
                if ctl && start.is_some() && t.contains('\n') {
                    out.push((start.unwrap(), last_end));
                    start = None;
                    first_kw = None;
                    first_tok = true;
                    ctl = false;
                    continue;
                }
                if t.matches('\n').count() >= 2 && start.is_some() && depth_block == 0 && !dollar_open {
                    out.push((start.unwrap(), last_end));
                    start = None;
                    first_kw = None;
                    first_tok = true;
                    ctl = false;
                    starts_create = false;
                }
                continue;
            }
            Tok::Comment => {
                if start.is_none() {
                    // comment before statement: include it so that it travels with it
                    start = Some(a);
                    last_end = b;
                } else {
                    last_end = b;
                }
                continue;
            }
            _ => {}
        }
        if start.is_none() {
            start = Some(a);
        }
        if first_tok {
            first_tok = false;
            ctl = t == "@";
        }
        if first_kw.is_none() && (k == Tok::Keyword || k == Tok::Ident) {
            first_kw = Some(t.to_uppercase());
            starts_create = t.eq_ignore_ascii_case("create");
        }
        if starts_create && k == Tok::Keyword {
            if t.eq_ignore_ascii_case("begin") {
                depth_block += 1;
            } else if t.eq_ignore_ascii_case("case") {
                depth_block += 1;
            } else if t.eq_ignore_ascii_case("end") && depth_block > 0 {
                depth_block -= 1;
            }
        }
        if k == Tok::Op && t.starts_with(delim_c) && depth_block == 0 && !dollar_open {
            out.push((start.unwrap(), last_end.max(a)));
            start = None;
            first_kw = None;
                    first_tok = true;
                    ctl = false;
            starts_create = false;
            continue;
        }
        last_end = b;
    }
    if let Some(st) = start {
        if last_end > st {
            out.push((st, last_end));
        }
    }
    out.retain(|(a, b)| !strip_comments(&s[*a..*b]).trim().is_empty());
    out
}

pub fn strip_comments(s: &str) -> String {
    tokenize(s).into_iter().filter(|(k, _, _)| *k != Tok::Comment).map(|(_, a, b)| &s[a..b]).collect()
}

/// The statement under the cursor (byte offset).
pub fn statement_at(s: &str, cursor: usize, delim: &str) -> Option<(usize, usize)> {
    let v = split_statements(s, delim);
    if v.is_empty() {
        return None;
    }
    for (a, b) in &v {
        if cursor >= *a && cursor <= *b {
            return Some((*a, *b));
        }
    }
    // cursor in whitespace: previous statement
    let mut best = v[0];
    for (a, b) in &v {
        if *b <= cursor {
            best = (*a, *b);
        }
    }
    Some(best)
}

/// Names of `:param` placeholders not inside strings/comments (and not `::cast`).
pub fn find_params(s: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (k, a, b) in tokenize(s) {
        if k == Tok::Param {
            if a > 0 && s.as_bytes()[a - 1] == b':' {
                continue;
            }
            let n = s[a + 1..b].to_string();
            if seen.insert(n.clone()) {
                out.push(n);
            }
        }
    }
    out
}

pub fn substitute_params(s: &str, vals: &[(String, String)]) -> String {
    let mut out = String::new();
    for (k, a, b) in tokenize(s) {
        let t = &s[a..b];
        if k == Tok::Param && !(a > 0 && s.as_bytes()[a - 1] == b':') {
            if let Some((_, v)) = vals.iter().find(|(n, _)| n == &t[1..]) {
                out.push_str(&param_literal(v));
                continue;
            }
        }
        out.push_str(t);
    }
    out
}

fn param_literal(v: &str) -> String {
    let t = v.trim();
    if t.eq_ignore_ascii_case("null") || t.parse::<f64>().is_ok() {
        t.to_string()
    } else {
        format!("'{}'", t.replace('\'', "''"))
    }
}

/// Replace `${var}` with values.
pub fn substitute_vars(s: &str, vars: &[(String, String)]) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{}}}", k), v);
    }
    out
}

pub fn toggle_comment(s: &str) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
    let all = lines.iter().filter(|l| !l.trim().is_empty()).all(|l| l.trim_start().starts_with("--"));
    lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else if all {
                let ind = l.len() - l.trim_start().len();
                let rest = &l.trim_start()[2..];
                format!("{}{}", &l[..ind], rest.strip_prefix(' ').unwrap_or(rest))
            } else {
                let ind = l.len() - l.trim_start().len();
                format!("{}-- {}", &l[..ind], l.trim_start())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// SQL formatter that follows DBeaver's tokenized formatter
/// (model.sql/format/tokenized/SQLFormatterTokenized.java and IndentFormatter.java):
/// content of SELECT/FROM/WHERE/GROUP BY/ORDER BY/HAVING goes on its own indented line (4 spaces),
/// AND/OR/WHEN/ELSE start a line, a comma starts a line except inside functions, `IN (...)` lists and constraints,
/// JOINs start a line, `ON` breaks after itself, VALUES/LIMIT go back one level, `;` is followed by a blank line.
/// No line break is ever added while inside a function-like bracket.
pub fn format_sql(s: &str, upper: bool) -> String {
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum K {
        Kw,
        Name,
        Value,
        Comment,
        Sym,
    }
    struct T {
        text: String,
        kind: K,
        ws: bool,
    }
    const FUNCS: &[&str] = &["COUNT", "SUM", "AVG", "MIN", "MAX", "COALESCE", "CAST", "REPLACE", "VALUES", "EXISTS", "ROUND", "LOWER", "UPPER", "LENGTH", "SUBSTR", "SUBSTRING", "TRIM", "NOW", "DATE", "IFNULL", "NULLIF", "ABS"];
    const JOIN_BEGIN: &[&str] = &["LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "NATURAL", "JOIN"];

    // ---- tokens
    let raw = tokenize(s);
    let mut toks: Vec<T> = Vec::new();
    let mut ws = false;
    for (k, a, b) in raw {
        let text = &s[a..b];
        match k {
            Tok::Space => {
                ws = true;
                continue;
            }
            Tok::Comment => toks.push(T { text: text.to_string(), kind: K::Comment, ws }),
            Tok::Keyword => toks.push(T { text: text.to_string(), kind: K::Kw, ws }),
            Tok::Ident | Tok::Param => toks.push(T { text: text.to_string(), kind: K::Name, ws }),
            Tok::Str | Tok::Number => toks.push(T { text: text.to_string(), kind: K::Value, ws }),
            Tok::Op => toks.push(T { text: text.to_string(), kind: K::Sym, ws }),
        }
        ws = false;
    }
    if toks.is_empty() {
        return s.trim().to_string();
    }
    // names joined by dots: a.b, schema."Table", t.*
    let mut merged: Vec<T> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let mut cur = T { text: toks[i].text.clone(), kind: toks[i].kind, ws: toks[i].ws };
        while matches!(cur.kind, K::Name) || (matches!(cur.kind, K::Kw) && false) {
            if i + 2 < toks.len() && toks[i + 1].text == "." && !toks[i + 1].ws && !toks[i + 2].ws && matches!(toks[i + 2].kind, K::Name | K::Kw | K::Sym) && (toks[i + 2].kind != K::Sym || toks[i + 2].text == "*") {
                cur.text.push('.');
                cur.text.push_str(&toks[i + 2].text);
                i += 2;
            } else {
                break;
            }
        }
        // a keyword written like a name right before a dot (e.g. `date.x`) stays a keyword; only joined names matter here
        merged.push(cur);
        i += 1;
    }
    // ORDER BY, GROUP BY, CONNECT BY, START WITH are one keyword
    let mut toks: Vec<T> = Vec::new();
    let mut i = 0;
    while i < merged.len() {
        let up = merged[i].text.to_uppercase();
        if merged[i].kind == K::Kw && i + 1 < merged.len() && merged[i + 1].kind == K::Kw {
            let n = merged[i + 1].text.to_uppercase();
            if (matches!(up.as_str(), "ORDER" | "GROUP" | "CONNECT") && n == "BY") || (up == "START" && n == "WITH") {
                let text = format!("{} {}", merged[i].text, merged[i + 1].text);
                toks.push(T { text, kind: K::Kw, ws: merged[i].ws });
                i += 2;
                continue;
            }
        }
        toks.push(T { text: merged[i].text.clone(), kind: merged[i].kind, ws: merged[i].ws });
        i += 1;
    }
    let n = toks.len();
    for t in toks.iter_mut() {
        if t.kind == K::Kw {
            t.text = if upper { t.text.to_uppercase() } else { t.text.to_lowercase() };
        }
    }
    let up: Vec<String> = toks.iter().map(|t| t.text.to_uppercase()).collect();

    // ---- line breaks: brk[i] = Some(indent) puts a line break before token i (-1 = blank line)
    let mut brk: Vec<Option<i32>> = vec![None; n + 1];
    let mut indent: i32 = 0;
    let mut depth = 0usize;
    let mut stack: Vec<(i32, bool, bool, bool)> = Vec::new(); // saved indent, function, condition, constraint
    let mut blocks: Vec<bool> = Vec::new(); // true = CASE, false = BEGIN block
    let mut between = false;
    let mut first_cond = false;
    let prev_kw = |idx: usize| -> Option<String> { (0..idx).rev().find(|&j| toks[j].kind == K::Kw).map(|j| up[j].clone()) };
    let prev_dml = |idx: usize| -> Option<String> { (0..idx).rev().find(|&j| toks[j].kind == K::Kw && matches!(up[j].as_str(), "SELECT" | "UPDATE" | "INSERT" | "DELETE")).map(|j| up[j].clone()) };
    macro_rules! brk {
        ($pos:expr, $ind:expr) => {{
            let pos: usize = $pos;
            let keep_blank = brk.get(pos).copied().flatten() == Some(-1) && $ind == 0; // an existing break is only replaced by a longer one
            if !stack.iter().any(|b| b.1) && pos > 0 && pos < n && !keep_blank {
                brk[pos] = Some($ind);
            }
        }};
    }
    for i in 0..n {
        let t = &toks[i];
        let u = up[i].as_str();
        match t.kind {
            K::Sym => match u {
                "(" => {
                    let prev = if i > 0 { Some(&toks[i - 1]) } else { None };
                    let is_func = prev.map(|p| p.kind == K::Name || (p.kind == K::Kw && FUNCS.contains(&up[i - 1].as_str()))).unwrap_or(false);
                    let cond = (0..i).any(|j| toks[j].kind == K::Kw && matches!(up[j].as_str(), "WHERE" | "ON" | "HAVING"));
                    let constraint = i > 0 && (up[i - 1] == "UNIQUE" || (i > 1 && (up[i - 2] == "REFERENCES" || (up[i - 1] == "KEY" && matches!(up[i - 2].as_str(), "PRIMARY" | "FOREIGN")))));
                    stack.push((indent, is_func, cond, constraint));
                    first_cond = true;
                    depth += 1;
                }
                ")" => {
                    if let Some((saved, ..)) = stack.pop() {
                        indent = saved;
                        depth = depth.saturating_sub(1);
                    }
                }
                "," => {
                    let in_func = stack.last().map(|b| b.1).unwrap_or(false);
                    let after_in = depth > 0 && prev_kw(i).as_deref() == Some("IN");
                    let in_constraint = stack.last().map(|b| b.3).unwrap_or(false);
                    if !in_func && !after_in && !in_constraint {
                        brk!(i + 1, indent);
                    }
                }
                ";" => {
                    indent = 0;
                    brk!(i + 1, -1);
                }
                _ => {}
            },
            K::Comment => {
                if t.text.starts_with("--") {
                    brk!(i, indent);
                } else {
                    brk!(i + 1, indent);
                }
            }
            K::Kw => match u {
                "BEGIN" if prev_dml(i).as_deref() != Some("SELECT") => {
                    brk!(i, indent - 1);
                    indent += 1;
                    brk!(i + 1, indent);
                    blocks.push(false);
                }
                "END" => {
                    if blocks.pop() == Some(false) {
                        indent -= 1;
                        brk!(i, indent);
                    } else {
                        indent -= 1;
                        brk!(i, indent);
                    }
                }
                "DELETE" | "SELECT" | "UPDATE" | "INSERT" | "INTO" | "TRUNCATE" => {
                    if depth > 0 {
                        brk!(i, indent);
                    } else if i > 0 {
                        indent = 0;
                        brk!(i, 0);
                    }
                    indent += 1;
                    brk!(i + 1, indent);
                }
                "CASE" => {
                    indent += 1;
                    brk!(i + 1, indent);
                    blocks.push(true);
                }
                "FROM" | "WHERE" | "START WITH" | "CONNECT BY" | "ORDER BY" | "GROUP BY" | "HAVING" => {
                    brk!(i, indent - 1);
                    brk!(i + 1, indent);
                    first_cond = false;
                }
                "LEFT" | "RIGHT" | "INNER" | "OUTER" | "FULL" | "CROSS" | "NATURAL" | "JOIN" => {
                    let begins = !(0..i).rev().find(|&j| toks[j].kind != K::Sym).map(|j| JOIN_BEGIN.contains(&up[j].as_str())).unwrap_or(false);
                    let ends_in_join = (i..n).filter(|&j| toks[j].kind != K::Sym).find_map(|j| if up[j] == "JOIN" { Some(true) } else if !JOIN_BEGIN.contains(&up[j].as_str()) { Some(false) } else { None }).unwrap_or(false);
                    if begins && ends_in_join {
                        brk!(i, indent - 1);
                    }
                }
                "VALUES" | "LIMIT" => {
                    indent -= 1;
                    brk!(i, indent);
                }
                "OR" | "WHEN" | "ELSE" => {
                    let pk = prev_kw(i);
                    let mut go = true;
                    if u == "OR" {
                        if pk.as_deref() == Some("CREATE") {
                            go = false;
                        } else if first_cond && stack.last().map(|b| b.2).unwrap_or(false) {
                            indent += 1;
                            brk!(i, indent);
                            first_cond = false;
                        }
                    }
                    if go && u != "ELSE" && pk.as_deref() == Some("CASE") {
                        go = false;
                    }
                    if go {
                        brk!(i, indent);
                    }
                }
                "SET" => {
                    let pk = prev_kw(i);
                    if pk.as_deref() != Some("CHARACTER") {
                        if i > 1 && pk.as_deref() == Some("UPDATE") {
                            brk!(i, indent - 1);
                        }
                        brk!(i + 1, indent);
                    }
                }
                "ON" => {
                    brk!(i + 1, indent);
                }
                "USING" => {
                    brk!(i, indent + 1);
                }
                "TOP" => {
                    brk!(i, indent);
                }
                "UNION" | "INTERSECT" | "EXCEPT" => {
                    indent -= 2;
                    brk!(i, indent);
                    indent += 1;
                }
                "BETWEEN" => between = true,
                "AND" => {
                    if !between {
                        brk!(i, indent);
                        if first_cond && stack.last().map(|b| b.2).unwrap_or(false) {
                            indent += 1;
                            brk!(i, indent);
                            first_cond = false;
                        }
                    }
                    between = false;
                }
                _ => {}
            },
            _ => {}
        }
    }

    // ---- spacing
    let mut nospace = vec![false; n + 1];
    for i in 0..n.saturating_sub(2) {
        // "( TOKEN )" loses the spaces inside the brackets
        if toks[i].text == "(" && toks[i + 2].text == ")" && toks[i + 1].kind != K::Comment {
            nospace[i + 1] = true;
            nospace[i + 2] = true;
        }
    }
    let mut out = String::new();
    for i in 0..n {
        if i > 0 {
            let prev = &toks[i - 1];
            let cur = &toks[i];
            let after_line_comment = prev.kind == K::Comment && prev.text.starts_with("--");
            if let Some(ind) = brk[i] {
                if ind < 0 {
                    out.push_str("\n\n");
                } else {
                    out.push('\n');
                    out.push_str(&"    ".repeat(ind as usize));
                }
            } else if after_line_comment {
                out.push('\n');
            } else if nospace[i] {
                // no space
            } else if cur.ws {
                out.push(' ');
            } else {
                // two tokens that touch each other: insert a space unless one of DBeaver's exceptions applies
                let (p, c) = (prev.text.as_str(), cur.text.as_str());
                let skip = p == "(" || c.starts_with('(') || p == ")" || c == ")" || c == "," || c == ";" || (cur.kind == K::Value && prev.kind == K::Name) || (prev.kind == K::Sym && cur.kind == K::Sym) || p == ":" || c == ":" || p == "." || c == ".";
                if !skip {
                    out.push(' ');
                }
            }
        }
        out.push_str(&toks[i].text);
    }
    let mut res = out.trim().to_string();
    if s.ends_with('\n') {
        res.push('\n');
    }
    res
}

pub fn selected_columns_hint(sql: &str) -> Vec<String> {
    let _ = sql;
    Vec::new()
}

/// Names after FROM/JOIN: (table, alias)
pub fn referenced_tables(sql: &str) -> Vec<(String, String)> {
    let toks: Vec<String> = tokenize(sql).into_iter().filter(|(k, _, _)| !matches!(k, Tok::Space | Tok::Comment)).map(|(_, a, b)| sql[a..b].to_string()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let u = toks[i].to_uppercase();
        if (u == "FROM" || u == "JOIN" || u == "UPDATE" || u == "INTO") && i + 1 < toks.len() {
            let mut name = toks[i + 1].trim_matches(|c| c == '"' || c == '`').to_string();
            let mut j = i + 2;
            if j + 1 < toks.len() && toks[j] == "." {
                name = toks[j + 1].trim_matches(|c| c == '"' || c == '`').to_string();
                j += 2;
            }
            let mut alias = name.clone();
            if j < toks.len() && toks[j].eq_ignore_ascii_case("as") && j + 1 < toks.len() {
                alias = toks[j + 1].clone();
            } else if j < toks.len() && !is_keyword(&toks[j]) && toks[j].chars().all(|c| c.is_alphanumeric() || c == '_') {
                alias = toks[j].clone();
            }
            out.push((name, alias));
        }
        i += 1;
    }
    out
}

/// Keyword case chosen from the text itself (SQL_FORMAT_KEYWORD_CASE_AUTO): the majority wins, a tie keeps `default_upper`.
pub fn auto_upper(text: &str, default_upper: bool) -> bool {
    let (mut up, mut low) = (0, 0);
    for (k, a, b) in tokenize(text) {
        if k == Tok::Keyword {
            let w = &text[a..b];
            if w.chars().all(|c| !c.is_lowercase()) {
                up += 1;
            } else if w.chars().all(|c| !c.is_uppercase()) {
                low += 1;
            }
        }
    }
    if up > low {
        true
    } else if low > up {
        false
    } else {
        default_upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_semicolon_and_blank_line() {
        let s = "select 1;\nselect ';' as x; -- c\n\nselect 3\nfrom t";
        let v: Vec<String> = split_statements(s, ";").into_iter().map(|(a, b)| s[a..b].trim().to_string()).collect();
        assert_eq!(v.len(), 3, "{:?}", v);
        assert_eq!(v[1], "select ';' as x");
        assert!(v[2].starts_with("select 3"));
    }

    #[test]
    fn trigger_body_is_one_statement() {
        let s = "CREATE TRIGGER t AFTER INSERT ON a BEGIN INSERT INTO b VALUES (1); INSERT INTO b VALUES (2); END;\nselect 1;";
        let v = split_statements(s, ";");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn params_found() {
        assert_eq!(find_params("select * from t where a = :id and b::int = :name"), vec!["id", "name"]);
        assert_eq!(substitute_params("a = :id", &[("id".into(), "5".into())]), "a = 5");
        assert_eq!(substitute_params("a = :n", &[("n".into(), "x'y".into())]), "a = 'x''y'");
    }

    #[test]
    fn keyword_case_follows_the_text() {
        assert!(!auto_upper("select a from t where b = 1", true));
        assert!(auto_upper("SELECT a FROM t", false));
        assert!(auto_upper("a b c", true));
        assert!(!auto_upper("a b c", false));
    }

    #[test]
    fn comment_toggle() {
        assert_eq!(toggle_comment("select 1"), "-- select 1");
        assert_eq!(toggle_comment("-- select 1"), "select 1");
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn dollar_quoted_function_stays_in_one_statement() {
        let s = "CREATE OR REPLACE FUNCTION public.soma(a integer, b integer)\n RETURNS integer\n LANGUAGE plpgsql\nAS $function$\nBEGIN\n  RAISE NOTICE 'x';\n\n  RETURN a + b;\nEND;\n$function$\n;\nSELECT 1;";
        let v = split_statements(s, ";");
        assert_eq!(v.len(), 2, "{:?}", v.iter().map(|(a, b)| &s[*a..*b]).collect::<Vec<_>>());
        assert!(s[v[0].0..v[0].1].contains("RETURN a + b;"));
    }

    #[test]
    fn mysql_and_tsql_procedures_stay_in_one_statement() {
        let my = "CREATE PROCEDURE p()\nBEGIN\n  SELECT 1;\n\n  SELECT 2;\nEND;\nSELECT 3;";
        let v = split_statements(my, ";");
        assert_eq!(v.len(), 2, "{:?}", v.iter().map(|(a, b)| &my[*a..*b]).collect::<Vec<_>>());
        let ms = "CREATE PROCEDURE [dbo].[p]\nAS\nBEGIN\n  SELECT 1;\n  SELECT 2;\nEND";
        assert_eq!(split_statements(ms, ";").len(), 1);
    }
}

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn select_follows_the_dbeaver_layout() {
        let f = format_sql("select c.id, c.name, count(o.id) as n from customers c left join orders o on o.cid = c.id where c.x between 1 and 2 and c.y in (select z from w) group by c.id order by n desc limit 10;", true);
        println!("{}", f);
        let expected = "SELECT\n    c.id,\n    c.name,\n    COUNT(o.id) AS n\nFROM\n    customers c\nLEFT JOIN orders o ON\n    o.cid = c.id\nWHERE\n    c.x BETWEEN 1 AND 2\n    AND c.y IN (\n    SELECT\n        z\n    FROM\n        w)\nGROUP BY\n    c.id\nORDER BY\n    n DESC\nLIMIT 10;";
        assert_eq!(f, expected);
    }

    #[test]
    fn update_insert_case_and_spacing() {
        assert_eq!(format_sql("update t set a=1, b=2 where id=3", true), "UPDATE\n    t\nSET\n    a = 1,\n    b = 2\nWHERE\n    id = 3");
        let ins = format_sql("insert into t (a, b) values (1, 2)", true);
        println!("{}", ins);
        assert_eq!(ins, "INSERT\nINTO\n    t (a, b)\nVALUES (1, 2)");
        assert_eq!(
            format_sql("select case when a=1 then 'x' else 'y' end from t", true),
            "SELECT\n    CASE\n        WHEN a = 1 THEN 'x'\n        ELSE 'y'\n    END\nFROM\n    t"
        );
        assert_eq!(format_sql("select a from t where x in (1, 2, 3) and ( y )", false), "select\n    a\nfrom\n    t\nwhere\n    x in (1, 2, 3)\n    and (y)");
        // statements are separated by a blank line
        assert_eq!(format_sql("select 1; select 2", true), "SELECT\n    1;\n\nSELECT\n    2");
        // union starts at the left margin
        let u = format_sql("select a from t union select b from u", true);
        assert!(u.contains("\nUNION\n") || u.contains("\nUNION"), "{}", u);
    }

    #[test]
    fn comments_and_functions_do_not_break() {
        let f = format_sql("select coalesce(a, b), max(c) -- fim\nfrom t", true);
        println!("{}", f);
        assert!(f.contains("COALESCE(a, b),\n    MAX(c)"), "{}", f);
        assert!(f.contains("-- fim\nFROM") || f.contains("-- fim\n"), "{}", f);
    }
}

#[cfg(test)]
mod control_split_tests {
    use super::*;

    #[test]
    fn control_command_ends_at_the_end_of_the_line() {
        let s = "@set a = 1\n@echo oi ${a}\nselect ${a};\nselect 2";
        let v: Vec<String> = split_statements(s, ";").into_iter().map(|(a, b)| s[a..b].to_string()).collect();
        assert_eq!(v, vec!["@set a = 1", "@echo oi ${a}", "select ${a}", "select 2"]);
    }
}
