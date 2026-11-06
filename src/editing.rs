//! Grid editing: the toolbar, the statements generated from the pending changes, and the check that
//! decides whether a query result can be edited (single table, plain columns).
use crate::db::{value_literal, ColInfo, Kind};
use crate::drivers::mongo_value;
use crate::grid::{EditSource, GridState, RowState};
use crate::icons::{self, Icon};
use crate::shared::Shared;
use crate::sqltext;
use crate::theme::pal;
use egui::{RichText, Ui};

pub enum BarEvent {
    Save,
    Cancel,
}

/// The editing toolbar shown above every grid.
pub fn bar(ui: &mut Ui, g: &mut GridState, value_panel: &mut bool) -> Option<BarEvent> {
    let pl = pal(ui.visuals().dark_mode);
    let mut ev = None;
    let pending = g.pending_changes();
    let mut left = |ui: &mut Ui| {
      ui.horizontal_wrapped(|ui| {
        if g.editable {
            if icons::button(ui, Icon::Plus, "Linha").on_hover_text("Adicionar linha (Ins)").clicked() {
                let n = g.qr.columns.len();
                g.add_row(vec![None; n]);
            }
            let has_sel = g.sel.is_some();
            if ui.add_enabled(has_sel, |ui: &mut Ui| icons::button(ui, Icon::Copy, "Duplicar")).on_hover_text("Duplicar a linha selecionada").clicked() {
                if let Some((r, _)) = g.sel {
                    let row = g.qr.rows[r].clone();
                    g.add_row(row);
                }
            }
            if ui.add_enabled(has_sel, |ui: &mut Ui| icons::button(ui, Icon::Trash, "Excluir")).on_hover_text("Excluir as linhas selecionadas (Del) — clique de novo para restaurar").clicked() {
                let rows = g.target_rows();
                g.toggle_delete(&rows);
            }
            if ui.add_enabled(has_sel, |ui: &mut Ui| icons::button(ui, Icon::Null, "NULL")).on_hover_text("Definir a célula como NULL").clicked() {
                if let Some((r, c)) = g.sel {
                    g.set_cell(r, c, None);
                }
            }
            if ui.add_enabled(!g.undo.is_empty(), |ui: &mut Ui| icons::button(ui, Icon::Undo, "Desfazer")).on_hover_text("Ctrl+Z").clicked() {
                g.undo_last();
            }
            ui.separator();
            let label = if pending > 0 { format!("Salvar ({})", pending) } else { "Salvar".to_string() };
            if ui.add_enabled(pending > 0, |ui: &mut Ui| icons::primary(ui, Icon::Save, &label)).on_hover_text("Grava as alterações no banco").clicked() {
                ev = Some(BarEvent::Save);
            }
            if ui.add_enabled(pending > 0, |ui: &mut Ui| icons::button(ui, Icon::Cross, "Cancelar")).on_hover_text("Descarta as alterações pendentes").clicked() {
                ev = Some(BarEvent::Cancel);
            }
            if pending == 0 {
                ui.label(RichText::new("Duplo clique ou F2 edita • digite direto na célula • Tab/Enter navega").small().color(pl.weak));
            } else {
                icons::chip(ui, Some(Icon::Save), &format!("{} pendente(s)", pending), pl.warn);
            }
        } else {
            let why = g.readonly_reason.clone().unwrap_or_else(|| "Resultado somente leitura".to_string());
            icons::chip(ui, Some(Icon::Lock), "Somente leitura", pl.weak);
            ui.label(RichText::new(why).small().color(pl.weak));
        }
    });
    };
    let toggle = |ui: &mut Ui, value_panel: &mut bool| {
        if icons::segment(ui, Icon::Panel, "Painel de valor", *value_panel).on_hover_text("Ver/editar o valor completo da célula").clicked() {
            *value_panel = !*value_panel;
        }
    };
    // wide window: the button sits in a fixed column at the right edge and the rest of the bar wraps beside it
    let full = ui.available_rect_before_wrap();
    let bw = 150.0;
    if full.width() > 560.0 {
        let right = egui::Rect::from_min_size(egui::pos2(full.right() - bw, full.top()), egui::vec2(bw, 28.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| toggle(ui, value_panel));
        let lr = egui::Rect::from_min_max(full.min, egui::pos2(full.right() - bw - 8.0, full.bottom()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(lr), |ui| left(ui));
    } else {
        left(ui);
        ui.horizontal(|ui| toggle(ui, value_panel));
    }
    ev
}

fn meta_index(meta: &[ColInfo], name: &str) -> Option<usize> {
    meta.iter().position(|m| m.name.eq_ignore_ascii_case(name))
}

/// Statements that write the pending changes back.
pub fn change_script(g: &GridState, src: &EditSource) -> Vec<String> {
    let k = src.kind;
    let q = &src.qualified;
    // grid column -> meta column
    let map: Vec<Option<usize>> = g.qr.columns.iter().map(|c| meta_index(&src.meta.columns, c)).collect();
    let numeric = |j: usize| map[j].map(|m| src.meta.columns[m].is_numeric()).unwrap_or(false);
    let is_pk = |j: usize| map[j].map(|m| src.meta.columns[m].pk).unwrap_or(false);
    // columns that identify a row: the primary key when it is in the grid, else every column
    let pk_cols: Vec<usize> = (0..g.qr.columns.len()).filter(|&j| is_pk(j)).collect();
    let pk_complete = !pk_cols.is_empty() && src.meta.columns.iter().filter(|c| c.pk).count() == pk_cols.len();
    let key_cols: Vec<usize> = if pk_complete { pk_cols } else { (0..g.qr.columns.len()).filter(|&j| map[j].is_some()).collect() };
    let lit = |j: usize, v: &Option<String>| -> String {
        if k == Kind::Mongo {
            mongo_value(v, g.qr.columns[j] == "_id")
        } else {
            value_literal(v, numeric(j))
        }
    };
    let where_of = |row: &[Option<String>]| -> String {
        if k == Kind::Mongo {
            let parts: Vec<String> = key_cols.iter().map(|&j| format!("{}: {}", serde_json::to_string(&g.qr.columns[j]).unwrap_or_default(), lit(j, &row[j]))).collect();
            return format!("{{{}}}", parts.join(", "));
        }
        key_cols
            .iter()
            .map(|&j| match &row[j] {
                None => format!("{} IS NULL", k.quote(&g.qr.columns[j])),
                v => format!("{} = {}", k.quote(&g.qr.columns[j]), lit(j, v)),
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let mut out = Vec::new();
    for (i, st) in g.row_state.iter().enumerate() {
        match st {
            RowState::Clean => {}
            RowState::New => {
                let filled: Vec<usize> = (0..g.qr.columns.len()).filter(|&j| g.qr.rows[i][j].is_some()).collect();
                if k == Kind::Mongo {
                    let parts: Vec<String> = filled.iter().map(|&j| format!("{}: {}", serde_json::to_string(&g.qr.columns[j]).unwrap_or_default(), lit(j, &g.qr.rows[i][j]))).collect();
                    out.push(format!("{}.insertOne({{{}}})", q, parts.join(", ")));
                } else if filled.is_empty() {
                    out.push(match k {
                        Kind::Mysql => format!("INSERT INTO {} () VALUES ()", q),
                        _ => format!("INSERT INTO {} DEFAULT VALUES", q),
                    });
                } else {
                    let cols = filled.iter().map(|&j| k.quote(&g.qr.columns[j])).collect::<Vec<_>>().join(", ");
                    let vals = filled.iter().map(|&j| lit(j, &g.qr.rows[i][j])).collect::<Vec<_>>().join(", ");
                    out.push(format!("INSERT INTO {} ({}) VALUES ({})", q, cols, vals));
                }
            }
            RowState::Deleted => {
                if k == Kind::Mongo {
                    out.push(format!("{}.deleteOne({})", q, where_of(&g.qr.rows[i])));
                } else {
                    out.push(format!("DELETE FROM {} WHERE {}", q, where_of(&g.qr.rows[i])));
                }
            }
            RowState::Modified(orig) => {
                let changed: Vec<usize> = (0..g.qr.columns.len()).filter(|&j| orig[j] != g.qr.rows[i][j] && map[j].is_some()).collect();
                if changed.is_empty() {
                    continue;
                }
                if k == Kind::Mongo {
                    let sets: Vec<String> = changed.iter().filter(|&&j| g.qr.rows[i][j].is_some()).map(|&j| format!("{}: {}", serde_json::to_string(&g.qr.columns[j]).unwrap_or_default(), lit(j, &g.qr.rows[i][j]))).collect();
                    let unsets: Vec<String> = changed.iter().filter(|&&j| g.qr.rows[i][j].is_none()).map(|&j| format!("{}: \"\"", serde_json::to_string(&g.qr.columns[j]).unwrap_or_default())).collect();
                    let mut upd = Vec::new();
                    if !sets.is_empty() {
                        upd.push(format!("\"$set\": {{{}}}", sets.join(", ")));
                    }
                    if !unsets.is_empty() {
                        upd.push(format!("\"$unset\": {{{}}}", unsets.join(", ")));
                    }
                    out.push(format!("{}.updateOne({}, {{{}}})", q, where_of(orig), upd.join(", ")));
                } else {
                    let sets = changed.iter().map(|&j| format!("{} = {}", k.quote(&g.qr.columns[j]), lit(j, &g.qr.rows[i][j]))).collect::<Vec<_>>().join(", ");
                    out.push(format!("UPDATE {} SET {} WHERE {}", q, sets, where_of(orig)));
                }
            }
        }
    }
    out
}

/// Runs the statements; stops at the first error.
pub fn apply(sh: &mut Shared, conn: &str, script: &[String]) -> Result<(), String> {
    let sess = sh.session(conn).ok_or("Conexão fechada")?;
    let mut err = None;
    {
        let mut s = sess.lock().unwrap_or_else(|e| e.into_inner());
        for st in script {
            if let Err(e) = s.run(st, 0, 0) {
                err = Some(format!("{}\n→ {}", e, st));
                break;
            }
        }
    }
    for st in script {
        sh.history(conn, st, 0, "1 afetada".into(), err.is_none());
    }
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Decides whether the result of `sql` can be edited. Returns the source table or the reason why not.
pub fn detect_source(sh: &mut Shared, conn: &str, sql: &str, columns: &[String]) -> Result<EditSource, String> {
    let kind = sh.kind(conn);
    if kind == Kind::Mongo {
        return Err("Consultas MongoDB são editáveis ao abrir a coleção (duplo clique no navegador)".into());
    }
    let clean = sqltext::strip_comments(sql).to_lowercase();
    let w = sqltext::first_word(&clean);
    if w != "select" {
        return Err("Só consultas SELECT podem ser editadas".into());
    }
    for bad in [" join ", " group by ", " distinct ", " union ", " having ", " intersect ", " except ", " over(", " over (", "(select"] {
        if clean.contains(bad) {
            return Err(format!("Consulta com {} não pode ser editada — abra a tabela para editar", bad.trim().trim_end_matches('(').to_uppercase()));
        }
    }
    let refs = sqltext::referenced_tables(sql);
    let from_count = refs.len();
    if from_count != 1 {
        return Err("A edição precisa de uma única tabela na consulta".into());
    }
    let table = refs[0].0.clone();
    let schema = sh.default_schema(conn);
    let tables = sh.ensure_tables(conn, &schema);
    let Some((real, is_view)) = tables.iter().find(|(n, _)| n.eq_ignore_ascii_case(&table)).cloned() else {
        return Err(format!("Tabela “{}” não encontrada no esquema {}", table, schema));
    };
    if is_view {
        return Err("Visões não podem ser editadas".into());
    }
    let meta = sh.ensure_tmeta(conn, &schema, &real);
    for c in columns {
        if meta_index(&meta.columns, c).is_none() {
            return Err(format!("A coluna “{}” é uma expressão ou apelido — selecione colunas da tabela para editar", c));
        }
    }
    if !meta.columns.iter().any(|c| c.pk) {
        // still editable: rows are identified by all selected columns
    }
    let qualified = sh.qualified(conn, &schema, &real);
    Ok(EditSource { conn: conn.into(), schema, table: real, meta, qualified, kind })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ConnConfig, Kind};
    use crate::store::Store;

    fn setup(name: &str) -> (Shared, String) {
        let p = std::env::temp_dir().join(format!("dbr-edit-{}.sqlite", name));
        crate::sample::create(&p).unwrap();
        let mut cfg = ConnConfig::new(Kind::Sqlite);
        cfg.database = p.to_string_lossy().to_string();
        let id = cfg.id.clone();
        let mut sh = Shared::new(Store::memory(), false, egui::Context::default());
        sh.store.connections.push(cfg);
        sh.connect(&id).unwrap();
        (sh, id)
    }

    fn load(sh: &mut Shared, id: &str, sql: &str) -> GridState {
        let qr = sh.session(id).unwrap().lock().unwrap().run(sql, 200, 0).unwrap();
        let mut g = GridState::new(qr, "r");
        match detect_source(sh, id, sql, &g.qr.columns.clone()) {
            Ok(src) => {
                g.editable = true;
                g.edit = Some(src);
            }
            Err(e) => g.readonly_reason = Some(e),
        }
        g
    }

    #[test]
    fn edit_query_result_end_to_end() {
        let (mut sh, id) = setup("e2e");
        {
            let s = sh.session(&id).unwrap();
            let mut s = s.lock().unwrap();
            s.run("DELETE FROM order_items", 0, 0).unwrap();
            s.run("DELETE FROM orders", 0, 0).unwrap();
        }
        let mut g = load(&mut sh, &id, "SELECT * FROM customers WHERE customer_id <= 3 ORDER BY customer_id");
        assert!(g.editable, "{:?}", g.readonly_reason);
        g.set_cell(0, 1, Some("O'Neil".into()));
        g.set_cell(1, 4, None);
        g.toggle_delete(&[2]);
        let mut row = vec![None; g.qr.columns.len()];
        row[0] = Some("900".into());
        row[1] = Some("Nova".into());
        row[2] = Some("Pessoa".into());
        g.add_row(row);
        let script = change_script(&g, g.edit.as_ref().unwrap());
        assert_eq!(script.len(), 4, "{:?}", script);
        assert!(script[0].starts_with("UPDATE \"customers\" SET \"first_name\" = 'O''Neil' WHERE \"customer_id\" = 1"), "{}", script[0]);
        assert!(script.iter().any(|s| s.starts_with("DELETE FROM \"customers\" WHERE \"customer_id\" = 3")));
        assert!(script.iter().any(|s| s.contains("INSERT INTO \"customers\" (\"customer_id\", \"first_name\", \"last_name\") VALUES (900, 'Nova', 'Pessoa')")), "{:?}", script);
        // a row that is still referenced keeps the database's own error visible
        apply(&mut sh, &id, &script).unwrap();
        let s = sh.session(&id).unwrap();
        let mut s = s.lock().unwrap();
        assert_eq!(s.run("SELECT first_name FROM customers WHERE customer_id = 1", 0, 0).unwrap().rows[0][0].as_deref(), Some("O'Neil"));
        assert_eq!(s.run("SELECT country FROM customers WHERE customer_id = 2", 0, 0).unwrap().rows[0][0], None);
        assert!(s.run("SELECT 1 FROM customers WHERE customer_id = 3", 0, 0).unwrap().rows.is_empty());
        assert_eq!(s.run("SELECT last_name FROM customers WHERE customer_id = 900", 0, 0).unwrap().rows[0][0].as_deref(), Some("Pessoa"));
    }

    #[test]
    fn grid_edits_stay_pending_in_manual_and_smart_modes() {
        for (name, mode) in [("manual_pending", crate::db::TxMode::Manual), ("smart_pending", crate::db::TxMode::Smart)] {
            grid_edit_pending(name, mode);
        }
    }

    fn grid_edit_pending(name: &str, mode: crate::db::TxMode) {
        let (mut sh, id) = setup(name);
        sh.session(&id).unwrap().lock().unwrap().set_mode(mode).unwrap();
        let mut g = load(&mut sh, &id, "SELECT * FROM customers WHERE customer_id <= 3 ORDER BY customer_id");
        g.set_cell(0, 1, Some("Pendente".into()));
        let script = change_script(&g, g.edit.as_ref().unwrap());
        apply(&mut sh, &id, &script).unwrap();
        let s = sh.session(&id).unwrap();
        let mut s = s.lock().unwrap();
        assert!(s.tx_open, "the edit must open a transaction");
        assert_eq!(s.tx_count, 1);
        s.rollback().unwrap();
        assert_ne!(s.run("SELECT first_name FROM customers WHERE customer_id = 1", 0, 0).unwrap().rows[0][0].as_deref(), Some("Pendente"));
    }

    #[test]
    fn joins_and_expressions_are_read_only() {
        let (mut sh, id) = setup("ro");
        let g = load(&mut sh, &id, "SELECT c.first_name, o.order_id FROM customers c JOIN orders o ON o.customer_id = c.customer_id");
        assert!(!g.editable);
        assert!(g.readonly_reason.unwrap().contains("JOIN"));
        let g = load(&mut sh, &id, "SELECT first_name || ' ' || last_name AS n FROM customers");
        assert!(!g.editable);
        let g = load(&mut sh, &id, "SELECT * FROM customer_totals");
        assert!(!g.editable, "views are read-only");
        let g = load(&mut sh, &id, "SELECT first_name, last_name FROM customers");
        assert!(g.editable);
    }

    #[test]
    fn table_without_all_pk_columns_uses_every_column_to_find_the_row() {
        let (mut sh, id) = setup("nopk");
        let mut g = load(&mut sh, &id, "SELECT first_name, last_name FROM customers WHERE customer_id = 5");
        g.set_cell(0, 0, Some("X".into()));
        let script = change_script(&g, g.edit.as_ref().unwrap());
        assert!(script[0].contains("WHERE \"first_name\" = ") && script[0].contains("AND \"last_name\" = "), "{}", script[0]);
    }

    #[test]
    fn mongo_and_sqlserver_scripts() {
        use crate::db::{ColInfo, TableMeta};
        let meta = TableMeta { columns: vec![ColInfo { name: "_id".into(), typ: "objectId".into(), pk: true, ..Default::default() }, ColInfo { name: "age".into(), typ: "int32".into(), nullable: true, ..Default::default() }], ..Default::default() };
        let qr = crate::db::QueryResult { columns: vec!["_id".into(), "age".into()], rows: vec![vec![Some("65f1a2b3c4d5e6f708192a3b".into()), Some("30".into())]], ..Default::default() };
        let mut g = GridState::new(qr, "m");
        g.set_cell(0, 1, Some("31".into()));
        let src = EditSource { conn: "c".into(), schema: "db1".into(), table: "users".into(), meta: meta.clone(), qualified: "db.getSiblingDB(\"db1\").getCollection(\"users\")".into(), kind: Kind::Mongo };
        let s = change_script(&g, &src);
        assert_eq!(s[0], "db.getSiblingDB(\"db1\").getCollection(\"users\").updateOne({\"_id\": {\"$oid\":\"65f1a2b3c4d5e6f708192a3b\"}}, {\"$set\": {\"age\": 31}})");
        let src = EditSource { kind: Kind::SqlServer, qualified: "[dbo].[users]".into(), ..src };
        let s = change_script(&g, &src);
        assert_eq!(s[0], "UPDATE [dbo].[users] SET [age] = 31 WHERE [_id] = '65f1a2b3c4d5e6f708192a3b'");
    }
}
