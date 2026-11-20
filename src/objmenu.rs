//! Actions of a database object (table, view, procedure...), shared by the navigator and the SQL editor menus.
//! Everything is done through requests, so any part of the UI that has the `Shared` state can draw them.
use crate::db::{build_ddl, Kind};
use crate::shared::{ObjKind, ObjRef, Req, Shared};

/// Text of the "Gerar SQL" entries: 0 SELECT, 1 INSERT, 2 UPDATE, 3 DELETE, other DDL.
pub fn generate_sql(sh: &mut Shared, id: &str, schema: &str, name: &str, kind: u8) -> String {
    let m = sh.ensure_tmeta(id, schema, name);
    let k = sh.kind(id);
    let q = sh.qualified(id, schema, name);
    let cols: Vec<String> = m.columns.iter().map(|c| k.quote(&c.name)).collect();
    let pk: Vec<String> = m.columns.iter().filter(|c| c.pk).map(|c| format!("{} = ?", k.quote(&c.name))).collect();
    let wh = if pk.is_empty() { "<condição>".to_string() } else { pk.join(" AND ") };
    match kind {
        0 => format!("SELECT {}\nFROM {};", cols.join(",\n       "), q),
        1 => format!("INSERT INTO {} ({})\nVALUES ({});", q, cols.join(", "), vec!["?"; cols.len()].join(", ")),
        2 => format!("UPDATE {}\nSET {}\nWHERE {};", q, cols.iter().map(|c| format!("{} = ?", c)).collect::<Vec<_>>().join(",\n    "), wh),
        3 => format!("DELETE FROM {}\nWHERE {};", q, wh),
        _ => {
            if m.ddl.is_empty() {
                build_ddl(k, schema, name, &m)
            } else {
                m.ddl
            }
        }
    }
}

/// CALL / EXEC template of a procedure or function.
pub fn call_template(kind: Kind, key: &str, schema: &str, name: &str) -> String {
    match (kind, key) {
        (Kind::SqlServer, "procedure") => format!("EXEC [{}].[{}];\n", schema, name),
        (Kind::SqlServer, _) => format!("SELECT [{}].[{}]();\n", schema, name),
        (Kind::Mysql, "procedure") => format!("CALL {}.{}();\n", kind.quote(schema), kind.quote(name)),
        (Kind::Mysql, _) => format!("SELECT {}.{}();\n", kind.quote(schema), kind.quote(name)),
        (_, "procedure") => format!("CALL {}.{}();\n", kind.quote(schema), kind.quote(name)),
        _ => format!("SELECT {}.{}();\n", kind.quote(schema), kind.quote(name)),
    }
}

fn source_req(o: &ObjRef) -> Req {
    match &o.kind {
        ObjKind::Code { key, detail } => Req::OpenSource { conn: o.conn.clone(), schema: o.schema.clone(), key: key.clone(), name: o.name.clone(), detail: detail.clone() },
        _ => Req::OpenSource { conn: o.conn.clone(), schema: o.schema.clone(), key: "view".into(), name: o.name.clone(), detail: String::new() },
    }
}

/// The context menu entries of a database object.
pub fn object_menu(ui: &mut egui::Ui, sh: &mut Shared, o: &ObjRef) {
    let (id, schema, name) = (o.conn.as_str(), o.schema.as_str(), o.name.as_str());
    let ObjKind::Code { key, .. } = &o.kind else {
        let view = o.kind == ObjKind::View;
        if ui.button("Abrir tabela (dados)").clicked() {
            sh.req(Req::OpenTable { conn: id.into(), schema: schema.into(), table: name.into(), view });
            ui.close();
        }
        if ui.button("Novo script SQL (SELECT)").clicked() {
            let q = sh.qualified(id, schema, name);
            sh.req(Req::OpenSql { conn: Some(id.into()), text: format!("SELECT * FROM {};\n", q), title: None });
            ui.close();
        }
        if ui.button("Diagrama ER").clicked() {
            sh.req(Req::OpenEr { conn: id.into(), schema: schema.into(), focus: Some(name.into()) });
            ui.close();
        }
        ui.menu_button("Gerar SQL", |ui| {
            for (label, kind) in [("SELECT", 0u8), ("INSERT", 1), ("UPDATE", 2), ("DELETE", 3), ("DDL", 4)] {
                if ui.button(label).clicked() {
                    let text = generate_sql(sh, id, schema, name, kind);
                    sh.req(Req::GenSql { title: format!("Gerar SQL — {}", label), text });
                    ui.close();
                }
            }
        });
        if view && ui.button("Ver código-fonte").clicked() {
            sh.req(source_req(o));
            ui.close();
        }
        ui.separator();
        if ui.button("Exportar dados…").clicked() {
            let q = sh.qualified(id, schema, name);
            sh.req(Req::Export { conn: id.into(), sql: format!("SELECT * FROM {}", q), name: name.into(), rows: None });
            ui.close();
        }
        if !view && ui.button("Importar dados CSV…").clicked() {
            sh.req(Req::Import { conn: id.into(), schema: schema.into(), table: name.into() });
            ui.close();
        }
        ui.separator();
        if ui.button("Copiar nome").clicked() {
            ui.ctx().copy_text(name.to_string());
            ui.close();
        }
        if ui.button("Atualizar").clicked() {
            sh.refresh(id);
            ui.close();
        }
        if !view && ui.button("Excluir tabela (abre DROP no editor)").clicked() {
            let q = sh.qualified(id, schema, name);
            sh.req(Req::OpenSql { conn: Some(id.into()), text: format!("DROP TABLE {};\n", q), title: None });
            ui.close();
        }
        return;
    };
    if ui.button("Ver código-fonte").clicked() {
        sh.req(source_req(o));
        ui.close();
    }
    if matches!(key.as_str(), "function" | "procedure") && ui.button("Gerar chamada (CALL/EXEC)").clicked() {
        let text = call_template(sh.kind(id), key, schema, name);
        sh.req(Req::OpenSql { conn: Some(id.into()), text, title: None });
        ui.close();
    }
    if ui.button("Copiar nome").clicked() {
        ui.ctx().copy_text(name.to_string());
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(kind: ObjKind) -> ObjRef {
        ObjRef { conn: "c".into(), schema: "s".into(), name: "n".into(), kind }
    }

    #[test]
    fn views_and_code_objects_open_their_source() {
        assert!(matches!(source_req(&obj(ObjKind::View)), Req::OpenSource { ref key, .. } if key == "view"));
        let p = obj(ObjKind::Code { key: "procedure".into(), detail: "a int".into() });
        assert!(matches!(source_req(&p), Req::OpenSource { ref key, ref detail, .. } if key == "procedure" && detail == "a int"));
        assert_eq!(call_template(Kind::SqlServer, "procedure", "dbo", "p"), "EXEC [dbo].[p];\n");
    }
}
