//! Data export (CSV, JSON, XML, HTML, SQL, Markdown, TXT, DbUnit) and CSV import.
//! Defaults come from org.jkiss.dbeaver.data.transfer/plugin.xml.
use crate::db::{lit, Kind, QueryResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Csv,
    Json,
    Xml,
    Html,
    Sql,
    Markdown,
    Txt,
    DbUnit,
}

impl Format {
    pub const ALL: [Format; 8] = [Format::Csv, Format::Json, Format::Xml, Format::Html, Format::Sql, Format::Markdown, Format::Txt, Format::DbUnit];
    pub fn label(&self) -> &'static str {
        match self {
            Format::Csv => "CSV",
            Format::Json => "JSON",
            Format::Xml => "XML",
            Format::Html => "HTML",
            Format::Sql => "SQL",
            Format::Markdown => "Markdown Table",
            Format::Txt => "TXT",
            Format::DbUnit => "DbUnit",
        }
    }
    pub fn ext(&self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
            Format::Xml => "xml",
            Format::Html => "html",
            Format::Sql => "sql",
            Format::Markdown => "md",
            Format::Txt => "txt",
            Format::DbUnit => "xml",
        }
    }
}

#[derive(Clone)]
pub struct ExportOptions {
    pub format: Format,
    pub delimiter: String,      // ","
    pub header: bool,           // top
    pub quote_char: String,     // "
    pub quote_always: bool,     // false
    pub escape_formulas: bool,  // true
    pub null_string: String,    // ""
    pub rows_in_statement: usize, // 10
    pub keyword_upper: bool,    // upper
    pub omit_schema: bool,      // false
    pub table_name: String,
    pub include_doctype: bool,  // false
    pub show_nulls: bool,       // false (txt)
    pub md_header_sep: bool,    // true
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            format: Format::Csv,
            delimiter: ",".into(),
            header: true,
            quote_char: "\"".into(),
            quote_always: false,
            escape_formulas: true,
            null_string: String::new(),
            rows_in_statement: 10,
            keyword_upper: true,
            omit_schema: false,
            table_name: "table_name".into(),
            include_doctype: false,
            show_nulls: false,
            md_header_sep: true,
        }
    }
}

pub fn render(r: &QueryResult, o: &ExportOptions, kind: Kind) -> String {
    match o.format {
        Format::Csv => csv(r, o),
        Format::Json => json(r, o),
        Format::Xml => xml(r, o),
        Format::Html => html(r, o),
        Format::Sql => sql(r, o, kind),
        Format::Markdown => markdown(r, o),
        Format::Txt => txt(r, o),
        Format::DbUnit => dbunit(r, o),
    }
}

fn csv_cell(v: &Option<String>, o: &ExportOptions) -> String {
    let s = match v {
        None => return o.null_string.clone(),
        Some(s) => s.clone(),
    };
    let q = o.quote_char.chars().next();
    let mut s = s;
    if o.escape_formulas && s.starts_with(|c| matches!(c, '=' | '+' | '-' | '@')) && s.parse::<f64>().is_err() {
        s = format!("'{}", s);
    }
    let Some(q) = q else { return s };
    let need = o.quote_always || s.contains(&o.delimiter) || s.contains(q) || s.contains('\n') || s.contains('\r');
    if need {
        let doubled = s.replace(q, &format!("{}{}", q, q));
        format!("{}{}{}", q, doubled, q)
    } else {
        s
    }
}

fn csv(r: &QueryResult, o: &ExportOptions) -> String {
    let mut s = String::new();
    if o.header {
        let h: Vec<String> = r.columns.iter().map(|c| csv_cell(&Some(c.clone()), &ExportOptions { escape_formulas: false, ..o.clone() })).collect();
        s.push_str(&h.join(&o.delimiter));
        s.push('\n');
    }
    for row in &r.rows {
        let l: Vec<String> = row.iter().map(|v| csv_cell(v, o)).collect();
        s.push_str(&l.join(&o.delimiter));
        s.push('\n');
    }
    s
}

pub fn json_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn json_val(v: &Option<String>) -> String {
    match v {
        None => "null".into(),
        Some(s) => {
            let numeric = s.parse::<f64>().is_ok() && !s.starts_with('+') && !s.starts_with('0') || s == "0" || s.starts_with("0.");
            if numeric && s.parse::<f64>().map(|f| f.is_finite()).unwrap_or(false) {
                s.clone()
            } else {
                format!("\"{}\"", json_escape(s))
            }
        }
    }
}

fn json(r: &QueryResult, o: &ExportOptions) -> String {
    let mut s = String::new();
    s.push_str(&format!("{{\n\"{}\": [\n", json_escape(&o.table_name)));
    for (i, row) in r.rows.iter().enumerate() {
        s.push_str("\t{\n");
        for (j, c) in r.columns.iter().enumerate() {
            s.push_str(&format!("\t\t\"{}\" : {}{}\n", json_escape(c), json_val(&row[j]), if j + 1 < r.columns.len() { "," } else { "" }));
        }
        s.push_str(if i + 1 < r.rows.len() { "\t},\n" } else { "\t}\n" });
    }
    s.push_str("]\n}\n");
    s
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xml_name(s: &str) -> String {
    let mut n: String = s.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    if n.is_empty() || n.chars().next().unwrap().is_ascii_digit() {
        n.insert(0, '_');
    }
    n
}

fn xml(r: &QueryResult, o: &ExportOptions) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let t = xml_name(&o.table_name);
    if o.include_doctype {
        s.push_str(&format!("<!DOCTYPE {}[\n  <!ELEMENT {}(DATA_RECORD*)>\n  <!ELEMENT DATA_RECORD ({})+>\n]>\n", t, t, r.columns.iter().map(|c| xml_name(c)).collect::<Vec<_>>().join(",")));
    }
    s.push_str(&format!("<!--  -->\n<{}>\n", t));
    for row in &r.rows {
        s.push_str("  <DATA_RECORD>\n");
        for (j, c) in r.columns.iter().enumerate() {
            let n = xml_name(c);
            match &row[j] {
                None => s.push_str(&format!("    <{}/>\n", n)),
                Some(v) => s.push_str(&format!("    <{}>{}</{}>\n", n, xml_escape(v), n)),
            }
        }
        s.push_str("  </DATA_RECORD>\n");
    }
    s.push_str(&format!("</{}>\n", t));
    s
}

fn html(r: &QueryResult, o: &ExportOptions) -> String {
    let mut s = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<style>table{border-collapse:collapse}td,th{border:1px solid #999;padding:2px 6px}</style>\n</head>\n<body>\n<table>\n");
    s.push_str(&format!("<caption>{}</caption>\n", xml_escape(&o.table_name)));
    if o.header {
        s.push_str("<thead><tr>");
        for c in &r.columns {
            s.push_str(&format!("<th>{}</th>", xml_escape(c)));
        }
        s.push_str("</tr></thead>\n");
    }
    s.push_str("<tbody>\n");
    for row in &r.rows {
        s.push_str("<tr>");
        for v in row {
            s.push_str(&format!("<td>{}</td>", v.as_deref().map(xml_escape).unwrap_or_else(|| o.null_string.clone())));
        }
        s.push_str("</tr>\n");
    }
    s.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    s
}

pub fn sql_value(v: &Option<String>) -> String {
    match v {
        None => "NULL".into(),
        Some(s) => {
            if s.parse::<f64>().is_ok() && !s.starts_with('+') && (!s.starts_with('0') || s == "0" || s.starts_with("0.")) && s.len() < 19 {
                s.clone()
            } else {
                lit(s)
            }
        }
    }
}

fn sql(r: &QueryResult, o: &ExportOptions, kind: Kind) -> String {
    let kw = |s: &str| if o.keyword_upper { s.to_string() } else { s.to_lowercase() };
    let table = if o.omit_schema { o.table_name.rsplit('.').next().unwrap_or(&o.table_name).to_string() } else { o.table_name.clone() };
    let cols = r.columns.iter().map(|c| kind.quote(c)).collect::<Vec<_>>().join(", ");
    let n = o.rows_in_statement.max(1);
    let mut s = String::new();
    for chunk in r.rows.chunks(n) {
        s.push_str(&format!("{} {} {}({}) {}\n", kw("INSERT"), kw("INTO"), table, cols, kw("VALUES")));
        for (i, row) in chunk.iter().enumerate() {
            let vals = row.iter().map(sql_value).collect::<Vec<_>>().join(", ");
            s.push_str(&format!("\t({}){}\n", vals, if i + 1 < chunk.len() { "," } else { ";" }));
        }
    }
    s
}

fn md_esc(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn markdown(r: &QueryResult, o: &ExportOptions) -> String {
    let mut s = String::new();
    s.push('|');
    for c in &r.columns {
        s.push_str(&format!("{}|", md_esc(c)));
    }
    s.push('\n');
    if o.md_header_sep {
        s.push('|');
        for _ in &r.columns {
            s.push_str("-|");
        }
        s.push('\n');
    }
    for row in &r.rows {
        s.push('|');
        for v in row {
            s.push_str(&format!("{}|", v.as_deref().map(md_esc).unwrap_or_else(|| o.null_string.clone())));
        }
        s.push('\n');
    }
    s
}

fn txt(r: &QueryResult, o: &ExportOptions) -> String {
    let show = |v: &Option<String>| match v {
        None => if o.show_nulls { "[NULL]".to_string() } else { String::new() },
        Some(s) => s.replace('\n', " ").replace('\t', " "),
    };
    let mut w: Vec<usize> = r.columns.iter().map(|c| c.chars().count()).collect();
    for row in &r.rows {
        for (j, v) in row.iter().enumerate() {
            w[j] = w[j].max(show(v).chars().count());
        }
    }
    let line = |cells: Vec<String>| -> String {
        let mut s = String::from("|");
        for (j, c) in cells.iter().enumerate() {
            s.push_str(&format!("{:<width$}|", c, width = w[j]));
        }
        s.push('\n');
        s
    };
    let sep = {
        let mut s = String::from("|");
        for x in &w {
            s.push_str(&"-".repeat(*x));
            s.push('|');
        }
        s.push('\n');
        s
    };
    let mut out = String::new();
    if o.header {
        out.push_str(&sep);
        out.push_str(&line(r.columns.clone()));
    }
    out.push_str(&sep);
    for row in &r.rows {
        out.push_str(&line(row.iter().map(show).collect()));
    }
    out.push_str(&sep);
    out
}

fn dbunit(r: &QueryResult, o: &ExportOptions) -> String {
    let t = o.table_name.to_uppercase();
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<dataset>\n");
    for row in &r.rows {
        s.push_str(&format!("    <{}", t));
        for (j, c) in r.columns.iter().enumerate() {
            let v = match &row[j] {
                None => "[NULL]".to_string(),
                Some(v) => xml_escape(v),
            };
            s.push_str(&format!(" {}=\"{}\"", c.to_uppercase(), v));
        }
        s.push_str("/>\n");
    }
    s.push_str("</dataset>\n");
    s
}

// ------------------------------------------------------------------ import
pub struct CsvOptions {
    pub delimiter: char, // ","
    pub quote: char,     // "
    pub header: bool,    // true
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions { delimiter: ',', quote: '"', header: true }
    }
}

pub fn parse_csv(text: &str, o: &CsvOptions) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut inq = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        any = true;
        if inq {
            if c == o.quote {
                if chars.peek() == Some(&o.quote) {
                    cell.push(o.quote);
                    chars.next();
                } else {
                    inq = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == o.quote {
            inq = true;
        } else if c == o.delimiter {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' {
            row.push(std::mem::take(&mut cell));
            rows.push(std::mem::take(&mut row));
            any = false;
        } else if c == '\r' {
        } else {
            cell.push(c);
        }
    }
    if any || !cell.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// Column type guess from the first 100 sample rows (columnTypeSamplesCount=100).
pub fn guess_type(samples: &[&str]) -> &'static str {
    let vals: Vec<&&str> = samples.iter().filter(|s| !s.is_empty()).collect();
    if vals.is_empty() {
        return "TEXT";
    }
    if vals.iter().all(|s| s.parse::<i64>().is_ok()) {
        "INTEGER"
    } else if vals.iter().all(|s| s.parse::<f64>().is_ok()) {
        "DOUBLE PRECISION"
    } else {
        "TEXT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> QueryResult {
        QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec![Some("1".into()), Some("a,\"b\"".into())], vec![Some("2".into()), None]],
            ..Default::default()
        }
    }

    #[test]
    fn csv_quotes() {
        let t = render(&sample(), &ExportOptions::default(), Kind::Sqlite);
        assert_eq!(t, "id,name\n1,\"a,\"\"b\"\"\"\n2,\n");
    }

    #[test]
    fn csv_roundtrip() {
        let t = render(&sample(), &ExportOptions::default(), Kind::Sqlite);
        let r = parse_csv(&t, &CsvOptions::default());
        assert_eq!(r.len(), 3);
        assert_eq!(r[1][1], "a,\"b\"");
    }

    #[test]
    fn sql_and_json() {
        let o = ExportOptions { format: Format::Sql, table_name: "t".into(), ..Default::default() };
        let t = render(&sample(), &o, Kind::Sqlite);
        assert!(t.contains("INSERT INTO t(\"id\", \"name\") VALUES"), "{}", t);
        assert!(t.contains("NULL"));
        let o = ExportOptions { format: Format::Json, table_name: "t".into(), ..Default::default() };
        let t = render(&sample(), &o, Kind::Sqlite);
        let v: serde_json::Value = serde_json::from_str(&t).expect("valid json");
        assert_eq!(v["t"][1]["name"], serde_json::Value::Null);
    }

    #[test]
    fn all_formats_render() {
        for f in Format::ALL {
            let o = ExportOptions { format: f, table_name: "t".into(), ..Default::default() };
            assert!(!render(&sample(), &o, Kind::Sqlite).is_empty());
        }
    }
}
