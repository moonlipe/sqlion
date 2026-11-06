//! SQL Server (TDS, pure Rust) and MongoDB drivers.
use crate::db::{blob_text, ColInfo, ConnConfig, FkInfo, IdxInfo, QueryResult, TableMeta};
use anyhow::{anyhow, Result};
use mongodb::bson::{Bson, Document};
use serde_json::{json, Value};
use std::time::Duration;
use tiberius::{AuthMethod, Client, ColumnData, Config, FromSql};
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

// ================================================================ SQL Server
pub struct Ms {
    rt: tokio::runtime::Runtime,
    client: Client<Compat<tokio::net::TcpStream>>,
}

impl Ms {
    pub fn connect(cfg: &ConnConfig, pass: &str) -> Result<Ms> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let mut c = Config::new();
        c.host(&cfg.host);
        c.port(cfg.port);
        c.authentication(AuthMethod::sql_server(&cfg.user, pass));
        if !cfg.database.is_empty() {
            c.database(&cfg.database);
        }
        c.trust_cert();
        c.application_name("Sqlion");
        let secs = crate::constants::CONNECT_TIMEOUT_SECS;
        let client = rt.block_on(async {
            let tcp = tokio::time::timeout(Duration::from_secs(secs), tokio::net::TcpStream::connect(c.get_addr()))
                .await
                .map_err(|_| anyhow!("Tempo esgotado ao conectar em {}:{}", cfg.host, cfg.port))??;
            tcp.set_nodelay(true)?;
            let cl = Client::connect(c, tcp.compat_write()).await?;
            Ok::<_, anyhow::Error>(cl)
        })?;
        Ok(Ms { rt, client })
    }

    pub fn exec(&mut self, sql: &str) -> Result<QueryResult> {
        let Ms { rt, client } = self;
        let rows_expected = returns_rows(sql);
        // transaction control must run as a plain batch: through sp_executesql (`execute`) SQL Server raises
        // error 266 ("Transaction count after EXECUTE indicates a mismatching number of BEGIN and COMMIT")
        let tx_control = matches!(crate::sqltext::first_word(sql).to_uppercase().as_str(), "BEGIN" | "COMMIT" | "ROLLBACK" | "START" | "END" | "SAVE");
        if tx_control {
            return rt.block_on(async {
                client.simple_query(sql).await?.into_results().await?;
                Ok(QueryResult::default())
            });
        }
        rt.block_on(async {
            let mut out = QueryResult::default();
            if rows_expected {
                let mut stream = client.simple_query(sql).await?;
                let cols = stream.columns().await?.map(|c| c.iter().map(|c| c.name().to_string()).collect::<Vec<_>>()).unwrap_or_default();
                let rows = stream.into_first_result().await?;
                out.columns = cols;
                for row in rows {
                    out.rows.push(row.into_iter().map(ms_val).collect());
                }
                if out.columns.is_empty() {
                    out.affected = Some(0);
                }
            } else {
                let r = client.execute(sql, &[]).await?;
                out.affected = Some(r.total());
            }
            Ok(out)
        })
    }
}

fn returns_rows(sql: &str) -> bool {
    let w = crate::sqltext::first_word(sql).to_uppercase();
    matches!(w.as_str(), "SELECT" | "WITH" | "EXEC" | "EXECUTE" | "SP_HELP" | "SP_WHO" | "VALUES" | "DBCC" | "SET" | "USE" | "DECLARE") && !sql.to_uppercase().contains("INSERT INTO") || w.starts_with("SP_")
}

fn ms_val(cd: ColumnData<'static>) -> Option<String> {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
    match &cd {
        ColumnData::U8(v) => v.map(|x| x.to_string()),
        ColumnData::I16(v) => v.map(|x| x.to_string()),
        ColumnData::I32(v) => v.map(|x| x.to_string()),
        ColumnData::I64(v) => v.map(|x| x.to_string()),
        ColumnData::F32(v) => v.map(|x| x.to_string()),
        ColumnData::F64(v) => v.map(|x| x.to_string()),
        ColumnData::Bit(v) => v.map(|x| if x { "1".to_string() } else { "0".to_string() }),
        ColumnData::String(v) => v.as_ref().map(|s| s.to_string()),
        ColumnData::Guid(v) => v.map(|g| g.to_string().to_uppercase()),
        ColumnData::Binary(v) => v.as_ref().map(|b| blob_text(b)),
        ColumnData::Numeric(v) => v.map(|n| n.to_string()),
        ColumnData::Xml(v) => v.as_ref().map(|x| x.to_string()),
        ColumnData::DateTime(_) | ColumnData::SmallDateTime(_) | ColumnData::DateTime2(_) => {
            NaiveDateTime::from_sql(&cd).ok().flatten().map(|d| d.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
        }
        ColumnData::Date(_) => NaiveDate::from_sql(&cd).ok().flatten().map(|d| d.format("%Y-%m-%d").to_string()),
        ColumnData::Time(_) => NaiveTime::from_sql(&cd).ok().flatten().map(|d| d.format("%H:%M:%S%.3f").to_string()),
        ColumnData::DateTimeOffset(_) => DateTime::<FixedOffset>::from_sql(&cd).ok().flatten().map(|d| d.format("%Y-%m-%d %H:%M:%S%.3f %:z").to_string()),
    }
}

pub fn ms_schemas(ms: &mut Ms) -> Result<Vec<String>> {
    let r = ms.exec("SELECT name FROM sys.schemas WHERE name NOT IN ('guest','INFORMATION_SCHEMA','sys') AND name NOT LIKE 'db[_]%' ORDER BY name")?;
    Ok(r.rows.into_iter().filter_map(|r| r[0].clone()).collect())
}

pub fn ms_tables(ms: &mut Ms, schema: &str) -> Result<Vec<(String, bool)>> {
    let q = format!("SELECT TABLE_NAME, TABLE_TYPE FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = {} ORDER BY TABLE_NAME", crate::db::lit(schema));
    Ok(ms.exec(&q)?.rows.into_iter().map(|r| (r[0].clone().unwrap_or_default(), r[1].as_deref().unwrap_or("").contains("VIEW"))).collect())
}

pub fn ms_meta(ms: &mut Ms, schema: &str, table: &str) -> Result<TableMeta> {
    let mut m = TableMeta::default();
    let q = format!(
        "SELECT c.COLUMN_NAME, c.DATA_TYPE + CASE WHEN c.CHARACTER_MAXIMUM_LENGTH IS NOT NULL THEN '(' + CASE WHEN c.CHARACTER_MAXIMUM_LENGTH = -1 THEN 'max' ELSE CAST(c.CHARACTER_MAXIMUM_LENGTH AS varchar(10)) END + ')' ELSE '' END, c.IS_NULLABLE, c.COLUMN_DEFAULT, \
         (SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE k ON tc.CONSTRAINT_NAME = k.CONSTRAINT_NAME AND tc.TABLE_SCHEMA = k.TABLE_SCHEMA WHERE tc.CONSTRAINT_TYPE = 'PRIMARY KEY' AND tc.TABLE_SCHEMA = c.TABLE_SCHEMA AND tc.TABLE_NAME = c.TABLE_NAME AND k.COLUMN_NAME = c.COLUMN_NAME), \
         COLUMNPROPERTY(OBJECT_ID(c.TABLE_SCHEMA + '.' + c.TABLE_NAME), c.COLUMN_NAME, 'IsIdentity') \
         FROM INFORMATION_SCHEMA.COLUMNS c WHERE c.TABLE_SCHEMA = {} AND c.TABLE_NAME = {} ORDER BY c.ORDINAL_POSITION",
        crate::db::lit(schema),
        crate::db::lit(table)
    );
    for row in ms.exec(&q)?.rows {
        m.columns.push(ColInfo {
            name: row[0].clone().unwrap_or_default(),
            typ: row[1].clone().unwrap_or_default(),
            nullable: row[2].as_deref() == Some("YES"),
            default: row[3].clone(),
            pk: row[4].as_deref().map(|s| s != "0").unwrap_or(false),
            extra: if row[5].as_deref() == Some("1") { "identity".into() } else { String::new() },
        });
    }
    let q = format!(
        "SELECT i.name, i.is_unique, STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
         JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE i.object_id = OBJECT_ID({}) AND i.name IS NOT NULL AND ic.is_included_column = 0 GROUP BY i.name, i.is_unique",
        crate::db::lit(&format!("[{}].[{}]", schema, table))
    );
    if let Ok(r) = ms.exec(&q) {
        for row in r.rows {
            m.indexes.push(IdxInfo {
                name: row[0].clone().unwrap_or_default(),
                unique: matches!(row[1].as_deref(), Some("1")),
                columns: row[2].clone().unwrap_or_default().split(',').map(|s| s.to_string()).collect(),
                typ: "".into(),
            });
        }
    }
    let q = format!(
        "SELECT fk.name, pc.name, rs.name, rt.name, rc.name FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
         JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id JOIN sys.tables rt ON rt.object_id = fkc.referenced_object_id \
         JOIN sys.schemas rs ON rs.schema_id = rt.schema_id JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
         WHERE fk.parent_object_id = OBJECT_ID({}) ORDER BY fk.name, fkc.constraint_column_id",
        crate::db::lit(&format!("[{}].[{}]", schema, table))
    );
    if let Ok(r) = ms.exec(&q) {
        for row in r.rows {
            let name = row[0].clone().unwrap_or_default();
            if let Some(f) = m.fks.iter_mut().find(|f| f.name == name) {
                f.columns.push(row[1].clone().unwrap_or_default());
                f.ref_columns.push(row[4].clone().unwrap_or_default());
            } else {
                m.fks.push(FkInfo { name, columns: vec![row[1].clone().unwrap_or_default()], ref_schema: row[2].clone().unwrap_or_default(), ref_table: row[3].clone().unwrap_or_default(), ref_columns: vec![row[4].clone().unwrap_or_default()] });
            }
        }
    }
    m.ddl = crate::db::build_ddl(crate::db::Kind::SqlServer, schema, table, &m);
    Ok(m)
}

// ================================================================ MongoDB
pub struct Mongo {
    pub client: mongodb::sync::Client,
    pub db: String,
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) })
        .collect()
}

impl Mongo {
    pub fn connect(cfg: &ConnConfig, pass: &str) -> Result<Mongo> {
        let uri = if cfg.host.starts_with("mongodb://") || cfg.host.starts_with("mongodb+srv://") {
            cfg.host.clone()
        } else {
            let auth = if cfg.user.is_empty() { String::new() } else { format!("{}:{}@", pct(&cfg.user), pct(pass)) };
            let secs = crate::constants::CONNECT_TIMEOUT_SECS;
            format!("mongodb://{}{}:{}/?serverSelectionTimeoutMS={}&connectTimeoutMS={}", auth, cfg.host, cfg.port, secs * 1000, secs * 1000)
        };
        let client = mongodb::sync::Client::with_uri_str(&uri)?;
        // force server selection so that errors show up now
        client.database("admin").run_command(mongodb::bson::doc! {"ping": 1}).run()?;
        let db = if cfg.database.is_empty() { "test".to_string() } else { cfg.database.clone() };
        Ok(Mongo { client, db })
    }

    pub fn run(&mut self, sql: &str, limit: usize, offset: usize) -> Result<QueryResult> {
        let t = sql.trim().trim_end_matches(';').trim();
        let low = t.to_lowercase();
        if low == "show dbs" || low == "show databases" {
            let names = self.client.list_database_names().run()?;
            return Ok(table(vec!["name".into()], names.into_iter().map(|n| vec![Some(n)]).collect()));
        }
        if low == "show collections" || low == "show tables" {
            let names = self.client.database(&self.db).list_collection_names().run()?;
            return Ok(table(vec!["name".into()], names.into_iter().map(|n| vec![Some(n)]).collect()));
        }
        if let Some(rest) = low.strip_prefix("use ") {
            self.db = rest.trim().to_string();
            let mut r = QueryResult::default();
            r.affected = Some(0);
            return Ok(r);
        }
        if !t.starts_with("db") {
            return Err(anyhow!("Comando MongoDB inválido. Use por exemplo: db.minhaColecao.find({{}}) — ou show collections"));
        }
        let calls = parse_calls(t)?;
        self.exec(calls, limit, offset)
    }

    fn exec(&mut self, calls: Vec<Call>, limit: usize, offset: usize) -> Result<QueryResult> {
        let mut db = self.db.clone();
        let mut coll: Option<String> = None;
        let mut i = 0;
        // db-level pieces
        while i < calls.len() {
            let c = &calls[i];
            match (c.name.as_str(), &c.args) {
                ("getSiblingDB", Some(a)) => {
                    db = arg_str(a)?;
                    i += 1;
                }
                ("getCollection", Some(a)) => {
                    coll = Some(arg_str(a)?);
                    i += 1;
                    break;
                }
                ("getCollectionNames", _) => {
                    let names = self.client.database(&db).list_collection_names().run()?;
                    return Ok(table(vec!["name".into()], names.into_iter().map(|n| vec![Some(n)]).collect()));
                }
                ("runCommand", Some(a)) => {
                    let d = to_doc(&parse_relaxed(a.trim())?)?;
                    let r = self.client.database(&db).run_command(d).run()?;
                    return Ok(docs_result(vec![r]));
                }
                (name, None) => {
                    coll = Some(name.to_string());
                    i += 1;
                    break;
                }
                (name, Some(_)) => return Err(anyhow!("Método db.{}() não suportado", name)),
            }
        }
        let coll = coll.ok_or_else(|| anyhow!("Informe a coleção: db.minhaColecao.find({{}})"))?;
        let method = calls.get(i).ok_or_else(|| anyhow!("Informe a operação, por exemplo .find({{}})"))?;
        let chain = &calls[i + 1..];
        let c: mongodb::sync::Collection<Document> = self.client.database(&db).collection(&coll);
        let args = split_args(method.args.as_deref().unwrap_or(""));
        let arg_doc = |n: usize| -> Result<Document> {
            match args.get(n) {
                Some(a) if !a.trim().is_empty() => to_doc(&parse_relaxed(a)?),
                _ => Ok(Document::new()),
            }
        };
        match method.name.as_str() {
            "find" | "findOne" => {
                let mut sort: Option<Document> = None;
                let mut skip = offset as u64;
                let mut lim = limit;
                for m in chain {
                    let a = m.args.as_deref().unwrap_or("").trim();
                    match m.name.as_str() {
                        "sort" => sort = Some(to_doc(&parse_relaxed(a)?)?),
                        "skip" => skip += a.parse::<u64>().unwrap_or(0),
                        "limit" => {
                            let l = a.parse::<usize>().unwrap_or(0);
                            if l > 0 {
                                lim = if lim == 0 { l } else { lim.min(l) };
                            }
                        }
                        "count" | "itcount" => {
                            let n = c.count_documents(arg_doc(0)?).run()?;
                            return Ok(table(vec!["count".into()], vec![vec![Some(n.to_string())]]));
                        }
                        _ => {}
                    }
                }
                let filter = arg_doc(0)?;
                let mut f = c.find(filter);
                if args.len() > 1 && !args[1].trim().is_empty() {
                    f = f.projection(arg_doc(1)?);
                }
                if let Some(s) = sort {
                    f = f.sort(s);
                }
                if skip > 0 {
                    f = f.skip(skip);
                }
                let want = if method.name == "findOne" { 1 } else { lim };
                if want > 0 {
                    f = f.limit(if method.name == "findOne" { 1 } else { want as i64 + 1 });
                }
                let mut docs = Vec::new();
                for d in f.run()? {
                    docs.push(d?);
                }
                let mut r = docs_result(docs);
                if want > 0 && method.name != "findOne" && r.rows.len() > want {
                    r.rows.truncate(want);
                    r.has_more = true;
                }
                Ok(r)
            }
            "aggregate" => {
                let p = parse_relaxed(args.first().map(|s| s.as_str()).unwrap_or("[]"))?;
                let pipe: Vec<Document> = p.as_array().ok_or_else(|| anyhow!("aggregate espera uma lista de estágios"))?.iter().map(to_doc).collect::<Result<_>>()?;
                let mut docs = Vec::new();
                for d in c.aggregate(pipe).run()? {
                    docs.push(d?);
                }
                let mut r = docs_result(docs);
                if limit > 0 {
                    let tail: Vec<_> = r.rows.drain(..).skip(offset).collect();
                    r.rows = tail;
                    if r.rows.len() > limit {
                        r.rows.truncate(limit);
                        r.has_more = true;
                    }
                }
                Ok(r)
            }
            "countDocuments" | "count" => {
                let n = c.count_documents(arg_doc(0)?).run()?;
                Ok(table(vec!["count".into()], vec![vec![Some(n.to_string())]]))
            }
            "estimatedDocumentCount" => {
                let n = c.estimated_document_count().run()?;
                Ok(table(vec!["count".into()], vec![vec![Some(n.to_string())]]))
            }
            "distinct" => {
                let field = arg_str(args.first().map(|s| s.as_str()).unwrap_or(""))?;
                let v = c.distinct(&field, arg_doc(1)?).run()?;
                Ok(table(vec![field], v.into_iter().map(|b| vec![bson_text(&b)]).collect()))
            }
            "insertOne" => {
                c.insert_one(arg_doc(0)?).run()?;
                Ok(affected(1))
            }
            "insertMany" => {
                let p = parse_relaxed(args.first().map(|s| s.as_str()).unwrap_or("[]"))?;
                let docs: Vec<Document> = p.as_array().ok_or_else(|| anyhow!("insertMany espera uma lista de documentos"))?.iter().map(to_doc).collect::<Result<_>>()?;
                let n = docs.len();
                if n > 0 {
                    c.insert_many(docs).run()?;
                }
                Ok(affected(n as u64))
            }
            "updateOne" | "updateMany" => {
                let (f, u) = (arg_doc(0)?, arg_doc(1)?);
                let r = if method.name == "updateOne" { c.update_one(f, u).run()? } else { c.update_many(f, u).run()? };
                Ok(affected(r.modified_count))
            }
            "replaceOne" => {
                let r = c.replace_one(arg_doc(0)?, arg_doc(1)?).run()?;
                Ok(affected(r.modified_count))
            }
            "deleteOne" => Ok(affected(c.delete_one(arg_doc(0)?).run()?.deleted_count)),
            "deleteMany" => Ok(affected(c.delete_many(arg_doc(0)?).run()?.deleted_count)),
            "drop" => {
                c.drop().run()?;
                Ok(affected(0))
            }
            "createIndex" => {
                let model = mongodb::IndexModel::builder().keys(arg_doc(0)?).build();
                c.create_index(model).run()?;
                Ok(affected(0))
            }
            "getIndexes" => {
                let mut rows = Vec::new();
                for ix in c.list_indexes().run()? {
                    let ix = ix?;
                    let name = ix.options.as_ref().and_then(|o| o.name.clone()).unwrap_or_default();
                    rows.push(vec![Some(name), Some(serde_json::to_string(&Bson::Document(ix.keys.clone()).into_relaxed_extjson()).unwrap_or_default())]);
                }
                Ok(table(vec!["name".into(), "keys".into()], rows))
            }
            other => Err(anyhow!("Operação .{}() não suportada. Use find, findOne, aggregate, countDocuments, distinct, insertOne/Many, updateOne/Many, replaceOne, deleteOne/Many, createIndex, getIndexes, drop", other)),
        }
    }

    pub fn schemas(&mut self, cfg_db: &str) -> Vec<String> {
        match self.client.list_database_names().run() {
            Ok(v) if !v.is_empty() => v,
            _ => vec![if cfg_db.is_empty() { "test".to_string() } else { cfg_db.to_string() }],
        }
    }

    pub fn collections(&mut self, db: &str) -> Result<Vec<(String, bool)>> {
        let mut out = Vec::new();
        for s in self.client.database(db).list_collections().run()? {
            let s = s?;
            out.push((s.name.clone(), matches!(s.collection_type, mongodb::results::CollectionType::View)));
        }
        out.sort();
        Ok(out)
    }

    pub fn meta(&mut self, db: &str, coll: &str) -> Result<TableMeta> {
        let c: mongodb::sync::Collection<Document> = self.client.database(db).collection(coll);
        let mut m = TableMeta::default();
        let mut seen: Vec<(String, String)> = Vec::new();
        for d in c.find(Document::new()).limit(100).run()? {
            let d = d?;
            for (k, v) in d.iter() {
                if let Some(e) = seen.iter_mut().find(|(n, _)| n == k) {
                    if e.1 == "null" {
                        e.1 = bson_type(v).to_string();
                    }
                } else {
                    seen.push((k.clone(), bson_type(v).to_string()));
                }
            }
        }
        seen.sort_by_key(|(k, _)| if k == "_id" { 0 } else { 1 });
        for (k, t) in seen {
            m.columns.push(ColInfo { pk: k == "_id", nullable: k != "_id", name: k, typ: t, default: None, extra: String::new() });
        }
        if let Ok(cur) = c.list_indexes().run() {
            for ix in cur.flatten() {
                let o = ix.options.clone().unwrap_or_default();
                m.indexes.push(IdxInfo { name: o.name.unwrap_or_default(), unique: o.unique.unwrap_or(false), columns: ix.keys.keys().cloned().collect(), typ: "".into() });
            }
        }
        m.ddl = format!(
            "// Coleção sem esquema fixo — campos inferidos das primeiras 100 linhas\ndb.getSiblingDB(\"{}\").getCollection(\"{}\").find({{}})\n\n// Campos:\n{}",
            db,
            coll,
            m.columns.iter().map(|c| format!("//   {}: {}", c.name, c.typ)).collect::<Vec<_>>().join("\n")
        );
        Ok(m)
    }
}

fn affected(n: u64) -> QueryResult {
    QueryResult { affected: Some(n), ..Default::default() }
}

fn table(columns: Vec<String>, rows: Vec<Vec<Option<String>>>) -> QueryResult {
    QueryResult { columns, rows, ..Default::default() }
}

pub fn bson_type(b: &Bson) -> &'static str {
    match b {
        Bson::String(_) => "string",
        Bson::Int32(_) => "int32",
        Bson::Int64(_) => "int64",
        Bson::Double(_) => "double",
        Bson::Boolean(_) => "bool",
        Bson::DateTime(_) => "date",
        Bson::ObjectId(_) => "objectId",
        Bson::Document(_) => "object",
        Bson::Array(_) => "array",
        Bson::Decimal128(_) => "decimal",
        Bson::Null => "null",
        _ => "other",
    }
}

pub fn bson_text(b: &Bson) -> Option<String> {
    match b {
        Bson::Null | Bson::Undefined => None,
        Bson::String(s) => Some(s.clone()),
        Bson::ObjectId(o) => Some(o.to_hex()),
        Bson::Int32(i) => Some(i.to_string()),
        Bson::Int64(i) => Some(i.to_string()),
        Bson::Double(f) => Some(if f.fract() == 0.0 && f.abs() < 1e15 { format!("{:.1}", f) } else { f.to_string() }),
        Bson::Boolean(v) => Some(v.to_string()),
        Bson::DateTime(d) => Some(d.try_to_rfc3339_string().unwrap_or_else(|_| d.to_string())),
        Bson::Decimal128(d) => Some(d.to_string()),
        other => Some(serde_json::to_string(&other.clone().into_relaxed_extjson()).unwrap_or_default()),
    }
}

fn docs_result(docs: Vec<Document>) -> QueryResult {
    let mut cols: Vec<String> = Vec::new();
    for d in &docs {
        for k in d.keys() {
            if !cols.contains(k) {
                cols.push(k.clone());
            }
        }
    }
    cols.sort_by_key(|k| if k == "_id" { 0 } else { 1 });
    let rows = docs.iter().map(|d| cols.iter().map(|c| d.get(c).and_then(bson_text)).collect()).collect();
    QueryResult { columns: cols, rows, ..Default::default() }
}

fn to_doc(v: &Value) -> Result<Document> {
    match Bson::try_from(v.clone()).map_err(|e| anyhow!("JSON inválido: {}", e))? {
        Bson::Document(d) => Ok(d),
        _ => Err(anyhow!("Esperava um documento entre chaves")),
    }
}

fn arg_str(a: &str) -> Result<String> {
    match parse_relaxed(a.trim())? {
        Value::String(s) => Ok(s),
        _ => Err(anyhow!("Esperava um texto entre aspas")),
    }
}

// ---- shell-style command parsing: db.coll.method(args).chain(args)
pub struct Call {
    pub name: String,
    pub args: Option<String>,
}

fn parse_calls(t: &str) -> Result<Vec<Call>> {
    let b: Vec<char> = t.chars().collect();
    let mut i = 2; // after "db"
    let mut calls = Vec::new();
    while i < b.len() {
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != '.' {
            break;
        }
        i += 1;
        let st = i;
        while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_' || b[i] == '$') {
            i += 1;
        }
        let name: String = b[st..i].iter().collect();
        if name.is_empty() {
            return Err(anyhow!("Nome esperado depois do ponto"));
        }
        if i < b.len() && b[i] == '(' {
            let mut depth = 0;
            let mut quote: Option<char> = None;
            let a0 = i + 1;
            let mut end = None;
            while i < b.len() {
                let ch = b[i];
                if let Some(q) = quote {
                    if ch == '\\' {
                        i += 1;
                    } else if ch == q {
                        quote = None;
                    }
                } else if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else if ch == '(' || ch == '{' || ch == '[' {
                    depth += 1;
                } else if ch == ')' || ch == '}' || ch == ']' {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            let e = end.ok_or_else(|| anyhow!("Parêntese não fechado"))?;
            calls.push(Call { name, args: Some(b[a0..e].iter().collect()) });
        } else {
            calls.push(Call { name, args: None });
        }
    }
    if calls.is_empty() {
        return Err(anyhow!("Comando vazio"));
    }
    Ok(calls)
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut esc = false;
    for ch in s.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                cur.push(ch);
            }
            '{' | '[' | '(' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' | ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

// ---- tolerant JSON (shell flavour): bare keys, single quotes, ObjectId("..."), ISODate("...") ...
pub fn parse_relaxed(s: &str) -> Result<Value> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = Relaxed { c: &chars, i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i < chars.len() {
        return Err(anyhow!("Texto inesperado depois do valor: {}", chars[p.i..].iter().take(20).collect::<String>()));
    }
    Ok(v)
}

struct Relaxed<'a> {
    c: &'a [char],
    i: usize,
}

impl Relaxed<'_> {
    fn ws(&mut self) {
        while self.i < self.c.len() && self.c[self.i].is_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn expect(&mut self, ch: char) -> Result<()> {
        self.ws();
        if self.peek() == Some(ch) {
            self.i += 1;
            Ok(())
        } else {
            Err(anyhow!("Esperava '{}' na posição {}", ch, self.i))
        }
    }
    fn string(&mut self) -> Result<String> {
        let q = self.peek().unwrap();
        self.i += 1;
        let mut s = String::new();
        while let Some(ch) = self.peek() {
            self.i += 1;
            if ch == '\\' {
                let n = self.peek().ok_or_else(|| anyhow!("Texto incompleto"))?;
                self.i += 1;
                s.push(match n {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
            } else if ch == q {
                return Ok(s);
            } else {
                s.push(ch);
            }
        }
        Err(anyhow!("Texto sem aspas de fechamento"))
    }
    fn ident(&mut self) -> String {
        let st = self.i;
        while self.i < self.c.len() && (self.c[self.i].is_alphanumeric() || self.c[self.i] == '_' || self.c[self.i] == '$' || self.c[self.i] == '.') {
            self.i += 1;
        }
        self.c[st..self.i].iter().collect()
    }
    fn value(&mut self) -> Result<Value> {
        self.ws();
        match self.peek() {
            None => Err(anyhow!("Valor esperado")),
            Some('{') => {
                self.i += 1;
                let mut m = serde_json::Map::new();
                loop {
                    self.ws();
                    if self.peek() == Some('}') {
                        self.i += 1;
                        break;
                    }
                    let key = match self.peek() {
                        Some('"') | Some('\'') => self.string()?,
                        _ => self.ident(),
                    };
                    if key.is_empty() {
                        return Err(anyhow!("Nome de campo esperado na posição {}", self.i));
                    }
                    self.expect(':')?;
                    let v = self.value()?;
                    m.insert(key, v);
                    self.ws();
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                Ok(Value::Object(m))
            }
            Some('[') => {
                self.i += 1;
                let mut a = Vec::new();
                loop {
                    self.ws();
                    if self.peek() == Some(']') {
                        self.i += 1;
                        break;
                    }
                    a.push(self.value()?);
                    self.ws();
                    if self.peek() == Some(',') {
                        self.i += 1;
                    }
                }
                Ok(Value::Array(a))
            }
            Some('"') | Some('\'') => Ok(Value::String(self.string()?)),
            Some(ch) if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' => {
                let st = self.i;
                self.i += 1;
                while self.i < self.c.len() && (self.c[self.i].is_ascii_digit() || matches!(self.c[self.i], '.' | 'e' | 'E' | '-' | '+')) {
                    self.i += 1;
                }
                let t: String = self.c[st..self.i].iter().collect();
                if let Ok(n) = t.parse::<i64>() {
                    Ok(json!(n))
                } else {
                    t.parse::<f64>().map(|f| json!(f)).map_err(|_| anyhow!("Número inválido: {}", t))
                }
            }
            Some(_) => {
                let mut id = self.ident();
                if id == "new" {
                    self.ws();
                    id = self.ident();
                }
                self.ws();
                if self.peek() == Some('(') {
                    self.i += 1;
                    let mut args = Vec::new();
                    loop {
                        self.ws();
                        if self.peek() == Some(')') {
                            self.i += 1;
                            break;
                        }
                        args.push(self.value()?);
                        self.ws();
                        if self.peek() == Some(',') {
                            self.i += 1;
                        }
                    }
                    let a0 = args.first().cloned().unwrap_or(Value::Null);
                    return Ok(match id.as_str() {
                        "ObjectId" => json!({"$oid": a0}),
                        "ISODate" | "Date" => match a0 {
                            Value::Null => json!({"$date": chrono::Utc::now().to_rfc3339()}),
                            v => json!({"$date": v}),
                        },
                        "NumberLong" => json!({"$numberLong": value_text(&a0)}),
                        "NumberInt" | "Number" => a0,
                        "NumberDecimal" => json!({"$numberDecimal": value_text(&a0)}),
                        "UUID" => json!({"$uuid": a0}),
                        other => return Err(anyhow!("Função {}() não suportada", other)),
                    });
                }
                match id.as_str() {
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    "null" | "undefined" => Ok(Value::Null),
                    "" => Err(anyhow!("Valor inesperado na posição {}", self.i)),
                    other => Err(anyhow!("Valor desconhecido: {}", other)),
                }
            }
        }
    }
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

pub fn mongo_is_write(sql: &str) -> bool {
    let t = sql.trim();
    if !t.starts_with("db") {
        return false;
    }
    let w = ["insert", "update", "delete", "replace", "drop", "createIndex", "dropIndex", "bulkWrite", "findOneAnd", "rename", "remove", "save"];
    parse_calls(t).map(|c| c.iter().any(|m| w.iter().any(|x| m.name.starts_with(x)))).unwrap_or(false)
}

/// JSON text of a cell as it should be written back to MongoDB.
pub fn mongo_value(v: &Option<String>, is_id: bool) -> String {
    match v {
        None => "null".into(),
        Some(s) => {
            if is_id && s.len() == 24 && s.chars().all(|c| c.is_ascii_hexdigit()) {
                return format!("{{\"$oid\":\"{}\"}}", s);
            }
            match serde_json::from_str::<Value>(s) {
                Ok(j) if !s.trim().is_empty() => j.to_string(),
                _ => serde_json::to_string(s).unwrap_or_else(|_| "null".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relaxed_json() {
        let v = parse_relaxed("{ name: 'Ana', age: {$gt: 18}, tags: [\"a\", 'b'], _id: ObjectId(\"65f1a2b3c4d5e6f708192a3b\") }").unwrap();
        assert_eq!(v["name"], "Ana");
        assert_eq!(v["age"]["$gt"], 18);
        assert_eq!(v["_id"]["$oid"], "65f1a2b3c4d5e6f708192a3b");
        let d = to_doc(&v).unwrap();
        assert!(matches!(d.get("_id"), Some(Bson::ObjectId(_))));
    }

    #[test]
    fn shell_calls() {
        let c = parse_calls("db.getSiblingDB(\"x\").getCollection(\"users\").find({a: 1}, {b: 1}).sort({a: -1}).limit(5)").unwrap();
        let names: Vec<_> = c.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["getSiblingDB", "getCollection", "find", "sort", "limit"]);
        assert_eq!(split_args(c[2].args.as_deref().unwrap()).len(), 2);
        assert!(mongo_is_write("db.users.updateOne({a:1},{$set:{b:2}})"));
        assert!(!mongo_is_write("db.users.find({})"));
    }

    #[test]
    fn mongo_values() {
        assert_eq!(mongo_value(&Some("42".into()), false), "42");
        assert_eq!(mongo_value(&Some("abc".into()), false), "\"abc\"");
        assert_eq!(mongo_value(&None, false), "null");
        assert_eq!(mongo_value(&Some("65f1a2b3c4d5e6f708192a3b".into()), true), "{\"$oid\":\"65f1a2b3c4d5e6f708192a3b\"}");
    }
}
