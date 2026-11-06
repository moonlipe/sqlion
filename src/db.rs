//! Database access layer: SQLite, PostgreSQL and MySQL/MariaDB without any JVM.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Colour and icon/label of a connection badge.
#[derive(Clone, Debug, PartialEq)]
pub struct BadgeStyle {
    pub label: String,
    pub icon: Option<String>,
    pub color: (u8, u8, u8),
}

/// Transaction mode of a connection.
/// Auto: every statement commits. Manual: a transaction opens on the first statement of any kind.
/// Smart: only statements that change data or structure open a transaction; reads run directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum TxMode {
    #[default]
    Auto,
    Manual,
    Smart,
}

impl TxMode {
    pub const ALL: [TxMode; 3] = [TxMode::Auto, TxMode::Manual, TxMode::Smart];
    pub fn label(&self) -> &'static str {
        match self {
            TxMode::Auto => "Auto",
            TxMode::Manual => "Manual",
            TxMode::Smart => "Smart",
        }
    }
    pub fn from_auto_commit(on: bool) -> TxMode {
        if on {
            TxMode::Auto
        } else {
            TxMode::Manual
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Kind {
    Sqlite,
    Postgres,
    Mysql,
    SqlServer,
    Mongo,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Sqlite => "SQLite",
            Kind::Postgres => "PostgreSQL",
            Kind::Mysql => "MySQL / MariaDB",
            Kind::SqlServer => "SQL Server",
            Kind::Mongo => "MongoDB",
        }
    }
    pub fn default_port(&self) -> u16 {
        match self {
            Kind::Sqlite => 0,
            Kind::Postgres => 5432, // ext.postgresql/plugin.xml:377
            Kind::Mysql => 3306,    // ext.mysql/plugin.xml:172
            Kind::SqlServer => 1433,
            Kind::Mongo => 27017,
        }
    }
    pub fn quote(&self, ident: &str) -> String {
        match self {
            Kind::Mysql => format!("`{}`", ident.replace('`', "``")),
            Kind::SqlServer => format!("[{}]", ident.replace(']', "]]")),
            Kind::Mongo => ident.to_string(),
            _ => format!("\"{}\"", ident.replace('"', "\"\"")),
        }
    }
    pub fn badge(&self) -> (&'static str, (u8, u8, u8)) {
        match self {
            Kind::Sqlite => ("Sq", (60, 140, 200)),
            Kind::Postgres => ("Pg", (51, 103, 145)),
            Kind::Mysql => ("My", (0, 117, 143)),
            Kind::SqlServer => ("Ms", (190, 60, 60)),
            Kind::Mongo => ("Mo", (66, 150, 80)),
        }
    }
}

/// Ready-made connection templates: the engine that speaks the protocol, default port and user.
pub struct Preset {
    pub name: &'static str,
    pub kind: Kind,
    pub port: u16,
    pub user: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "SQLite", kind: Kind::Sqlite, port: 0, user: "" },
    Preset { name: "PostgreSQL", kind: Kind::Postgres, port: 5432, user: "postgres" },
    Preset { name: "MySQL", kind: Kind::Mysql, port: 3306, user: "root" },
    Preset { name: "MariaDB", kind: Kind::Mysql, port: 3306, user: "root" },
    Preset { name: "SQL Server", kind: Kind::SqlServer, port: 1433, user: "sa" },
    Preset { name: "MongoDB", kind: Kind::Mongo, port: 27017, user: "" },
    Preset { name: "CockroachDB", kind: Kind::Postgres, port: 26257, user: "root" },
    Preset { name: "YugabyteDB", kind: Kind::Postgres, port: 5433, user: "yugabyte" },
    Preset { name: "TimescaleDB", kind: Kind::Postgres, port: 5432, user: "postgres" },
    Preset { name: "Amazon Redshift", kind: Kind::Postgres, port: 5439, user: "awsuser" },
    Preset { name: "Greenplum", kind: Kind::Postgres, port: 5432, user: "gpadmin" },
    Preset { name: "TiDB", kind: Kind::Mysql, port: 4000, user: "root" },
    Preset { name: "OceanBase (MySQL)", kind: Kind::Mysql, port: 2881, user: "root" },
    Preset { name: "StarRocks", kind: Kind::Mysql, port: 9030, user: "root" },
    Preset { name: "Apache Doris", kind: Kind::Mysql, port: 9030, user: "root" },
    Preset { name: "SingleStore", kind: Kind::Mysql, port: 3306, user: "root" },
];

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConnConfig {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String, // obfuscated, see store::obfuscate
    /// legacy flag, kept so that old files load; `tx_mode` wins when present
    pub auto_commit: bool,
    /// id of the folder project this connection belongs to; empty = the default project
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub tx_mode: Option<TxMode>,
    /// custom badge colour (r, g, b); None = the colour of the database type
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// custom badge: the name of a built-in icon, or a label of up to 3 characters; None = the type's label
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub conn_type: crate::constants::ConnType,
    #[serde(default)]
    pub flavor: String,
}

impl ConnConfig {
    pub fn new(kind: Kind) -> Self {
        ConnConfig {
            id: crate::store::new_id(),
            name: String::new(),
            kind,
            host: "localhost".into(),
            port: kind.default_port(),
            database: String::new(),
            user: match kind {
                Kind::Postgres => "postgres".into(),
                Kind::Mysql => "root".into(),
                Kind::SqlServer => "sa".into(),
                _ => String::new(),
            },
            password: String::new(),
            auto_commit: true,
            project: String::new(),
            tx_mode: None,
            color: None,
            icon: None,
            folder: String::new(),
            read_only: false,
            conn_type: crate::constants::ConnType::Dev,
            flavor: String::new(),
        }
    }
    /// How the connection is drawn: the user's choice, else the default of its database type.
    pub fn badge_style(&self) -> BadgeStyle {
        let (label, color) = self.kind.badge();
        let custom = self.icon.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let (label, icon) = match custom {
            Some(c) if crate::icons::icon_by_name(c).is_some() => (label.to_string(), Some(c.to_string())),
            Some(c) => (c.chars().take(3).collect(), None),
            None => (label.to_string(), None),
        };
        BadgeStyle { label, icon, color: self.color.map(|c| (c[0], c[1], c[2])).unwrap_or(color) }
    }

    /// back to the defaults of the database type
    pub fn reset_appearance(&mut self) {
        self.color = None;
        self.icon = None;
    }

    /// effective transaction mode (old files only have the `auto_commit` flag)
    pub fn mode(&self) -> TxMode {
        self.tx_mode.unwrap_or_else(|| TxMode::from_auto_commit(self.auto_commit))
    }
    pub fn set_mode(&mut self, m: TxMode) {
        self.tx_mode = Some(m);
        self.auto_commit = m == TxMode::Auto;
    }
    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.clone();
        }
        match self.kind {
            Kind::Sqlite => std::path::Path::new(&self.database)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "SQLite".into()),
            _ => {
                if self.database.is_empty() {
                    self.host.clone()
                } else {
                    self.database.clone()
                }
            }
        }
    }
    pub fn url(&self) -> String {
        match self.kind {
            Kind::Sqlite => format!("sqlite:{}", self.database),
            Kind::Postgres => format!("postgresql://{}:{}/{}", self.host, self.port, self.database),
            Kind::Mysql => format!("mysql://{}:{}/{}", self.host, self.port, self.database),
            Kind::SqlServer => format!("sqlserver://{}:{}/{}", self.host, self.port, self.database),
            Kind::Mongo => format!("mongodb://{}:{}/{}", self.host, self.port, self.database),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub affected: Option<u64>,
    pub elapsed_ms: u128,
    pub has_more: bool,
    pub sql: String,
    pub offset: usize,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct ColInfo {
    pub name: String,
    pub typ: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub pk: bool,
    pub extra: String,
}

impl ColInfo {
    pub fn is_numeric(&self) -> bool {
        let t = self.typ.to_lowercase();
        ["int", "real", "float", "double", "numeric", "decimal", "serial", "number", "money"]
            .iter()
            .any(|k| t.contains(k))
            && !t.contains("interval")
            && !t.contains("point")
    }
}

#[derive(Clone, Debug, Default)]
pub struct FkInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct IdxInfo {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
    pub typ: String,
}

#[derive(Clone, Debug, Default)]
pub struct TableMeta {
    pub columns: Vec<ColInfo>,
    pub indexes: Vec<IdxInfo>,
    pub fks: Vec<FkInfo>,
    pub ddl: String,
}

pub enum Conn {
    Sqlite(rusqlite::Connection),
    Pg(postgres::Client),
    My(mysql::Conn),
    Ms(Box<crate::drivers::Ms>),
    Mongo(Box<crate::drivers::Mongo>),
}

/// Transaction state readable without locking the session (the UI polls it while a job holds the lock).
#[derive(Default)]
pub struct TxSnap {
    mode: std::sync::atomic::AtomicU8,
    open: std::sync::atomic::AtomicBool,
    count: std::sync::atomic::AtomicUsize,
    rows: std::sync::atomic::AtomicU64,
}

impl TxSnap {
    pub fn get(&self) -> (TxMode, bool, usize, u64) {
        use std::sync::atomic::Ordering::Relaxed;
        let m = match self.mode.load(Relaxed) {
            1 => TxMode::Manual,
            2 => TxMode::Smart,
            _ => TxMode::Auto,
        };
        (m, self.open.load(Relaxed), self.count.load(Relaxed), self.rows.load(Relaxed))
    }
}

pub struct Session {
    pub snap: std::sync::Arc<TxSnap>,
    pub cfg: ConnConfig,
    pub conn: Conn,
    pub mode: TxMode,
    pub tx_open: bool,
    /// statements that changed data in the open transaction
    pub tx_count: usize,
    /// rows those statements affected
    pub tx_rows: u64,
    /// the server only accepts the old (pre-4.1) password format, enabled automatically on a retry
    pub legacy_auth: bool,
}

pub fn deobf(p: &str) -> String {
    crate::store::deobfuscate(p)
}

impl Session {
    pub fn open(cfg: &ConnConfig) -> Result<Session> {
        let mut legacy_auth = false;
        let conn = match cfg.kind {
            Kind::Sqlite => {
                if cfg.database.trim().is_empty() {
                    return Err(anyhow!("Informe o arquivo do banco SQLite"));
                }
                Conn::Sqlite(rusqlite::Connection::open(&cfg.database)?)
            }
            Kind::Postgres => {
                let mut c = postgres::Config::new();
                let (host, port) = split_host_port(&cfg.host, cfg.port);
                c.host(&host).port(port).user(&cfg.user);
                if !cfg.password.is_empty() {
                    c.password(deobf(&cfg.password));
                }
                if !cfg.database.is_empty() {
                    c.dbname(&cfg.database);
                }
                c.connect_timeout(std::time::Duration::from_secs(crate::constants::CONNECT_TIMEOUT_SECS));
                c.application_name("Sqlion");
                Conn::Pg(c.connect(postgres::NoTls)?)
            }
            Kind::Mysql => {
                let (conn, legacy) = connect_mysql(cfg)?;
                legacy_auth = legacy;
                Conn::My(conn)
            }
            Kind::SqlServer => Conn::Ms(Box::new(crate::drivers::Ms::connect(cfg, &deobf(&cfg.password))?)),
            Kind::Mongo => Conn::Mongo(Box::new(crate::drivers::Mongo::connect(cfg, &deobf(&cfg.password))?)),
        };
        let mut s = Session { cfg: cfg.clone(), conn, snap: Default::default(), mode: cfg.mode(), tx_open: false, tx_count: 0, tx_rows: 0, legacy_auth };
        if let Conn::My(_) = s.conn {
            // old servers (MySQL < 5.5.3, MariaDB 5.1) do not know utf8mb4
            if s.raw("SET NAMES utf8mb4").is_err() {
                let _ = s.raw("SET NAMES utf8");
            }
        }
        s.sync_snap();
        Ok(s)
    }

    pub fn kind(&self) -> Kind {
        self.cfg.kind
    }

    /// Publish the transaction state for `TxSnap::get`.
    pub fn sync_snap(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        self.snap.mode.store(match self.mode { TxMode::Auto => 0, TxMode::Manual => 1, TxMode::Smart => 2 }, Relaxed);
        self.snap.open.store(self.tx_open, Relaxed);
        self.snap.count.store(self.tx_count, Relaxed);
        self.snap.rows.store(self.tx_rows, Relaxed);
    }

    pub fn auto_commit(&self) -> bool {
        self.mode == TxMode::Auto
    }

    pub fn set_mode(&mut self, m: TxMode) -> Result<()> {
        if m == TxMode::Auto && self.tx_open {
            self.commit()?;
        }
        self.mode = m;
        self.sync_snap();
        Ok(())
    }

    pub fn set_auto_commit(&mut self, on: bool) -> Result<()> {
        self.set_mode(TxMode::from_auto_commit(on))
    }

    pub fn commit(&mut self) -> Result<()> {
        if self.tx_open {
            self.raw("COMMIT")?;
            self.tx_open = false;
            self.tx_count = 0;
            self.tx_rows = 0;
        }
        self.sync_snap();
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if self.tx_open {
            self.raw("ROLLBACK")?;
            self.tx_open = false;
            self.tx_count = 0;
            self.tx_rows = 0;
        }
        self.sync_snap();
        Ok(())
    }

    fn raw(&mut self, sql: &str) -> Result<()> {
        match &mut self.conn {
            Conn::Sqlite(c) => c.execute_batch(sql)?,
            Conn::Pg(c) => {
                c.simple_query(sql)?;
            }
            Conn::My(c) => {
                use mysql::prelude::Queryable;
                c.query_drop(sql)?;
            }
            Conn::Ms(m) => {
                m.exec(sql)?;
            }
            Conn::Mongo(_) => {}
        }
        Ok(())
    }

    fn begin_if_needed(&mut self, sql: &str) -> Result<()> {
        if self.kind() == Kind::Mongo {
            return Ok(());
        }
        if self.mode != TxMode::Auto && !self.tx_open && !is_tx_control(sql) && (self.mode == TxMode::Manual || !is_read_only_sql(sql)) {
            let b = match self.kind() {
                Kind::Mysql => "START TRANSACTION",
                Kind::SqlServer => "BEGIN TRANSACTION",
                _ => "BEGIN",
            };
            self.raw(b)?;
            self.tx_open = true;
        }
        Ok(())
    }

    /// Run one statement and return a result set (or the affected-row count).
    /// `limit` > 0 applies paging to plain SELECT statements (200 rows by default,
    /// ModelPreferences.java:356).
    pub fn run(&mut self, sql: &str, limit: usize, offset: usize) -> Result<QueryResult> {
        let r = self.run_inner(sql, limit, offset).map_err(clean_err);
        self.sync_snap();
        r
    }

    fn run_inner(&mut self, sql: &str, limit: usize, offset: usize) -> Result<QueryResult> {
        let sql_t = sql.trim().trim_end_matches(';').trim();
        if sql_t.is_empty() {
            return Err(anyhow!("Instrução vazia"));
        }
        if self.cfg.read_only && !is_read_only_sql(sql_t) {
            return Err(anyhow!("Conexão somente leitura"));
        }
        self.begin_if_needed(sql_t)?;
        let start = Instant::now();
        if let Conn::Mongo(m) = &mut self.conn {
            let mut r = m.run(sql_t, limit, offset)?;
            r.elapsed_ms = start.elapsed().as_millis();
            r.sql = sql_t.to_string();
            r.offset = offset;
            return Ok(r);
        }
        let paged = limit > 0 && is_pageable(sql_t);
        let mut res = if paged {
            let wrapped = if self.kind() == Kind::SqlServer {
                // T-SQL pages with OFFSET .. FETCH, which needs an ORDER BY
                let order = if sql_t.to_uppercase().contains("ORDER BY") { "" } else { " ORDER BY (SELECT NULL)" };
                format!("{}{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", sql_t, order, offset, limit + 1)
            } else {
                format!("SELECT * FROM ({}) AS _dbq LIMIT {} OFFSET {}", sql_t, limit + 1, offset)
            };
            match self.exec(&wrapped) {
                Ok(r) => r,
                Err(_) => {
                    let mut r = self.exec(sql_t)?;
                    let total = r.rows.len();
                    r.rows = r.rows.into_iter().skip(offset).take(limit + 1).collect();
                    let _ = total;
                    r
                }
            }
        } else {
            self.exec(sql_t)?
        };
        if paged && res.rows.len() > limit {
            res.rows.truncate(limit);
            res.has_more = true;
        }
        match tx_control_kind(sql_t) {
            Some(true) => self.tx_open = true,
            Some(false) => {
                self.tx_open = false;
                self.tx_count = 0;
            self.tx_rows = 0;
            }
            None => {
                if !is_read_only_sql(sql_t) && self.tx_open {
                    self.tx_count += 1;
                    self.tx_rows += res.affected.unwrap_or(0);
                }
            }
        }
        res.elapsed_ms = start.elapsed().as_millis();
        res.sql = sql_t.to_string();
        res.offset = offset;
        Ok(res)
    }

    fn exec(&mut self, sql: &str) -> Result<QueryResult> {
        let mut out = QueryResult::default();
        match &mut self.conn {
            Conn::Sqlite(c) => {
                let mut stmt = c.prepare(sql)?;
                let n = stmt.column_count();
                if n == 0 {
                    drop(stmt);
                    let a = c.execute(sql, [])?;
                    out.affected = Some(a as u64);
                } else {
                    out.columns = stmt.column_names().iter().map(|s| s.to_string()).collect();
                    let mut rows = stmt.query([])?;
                    while let Some(r) = rows.next()? {
                        let mut v = Vec::with_capacity(n);
                        for i in 0..n {
                            v.push(sqlite_val(r.get_ref(i)?));
                        }
                        out.rows.push(v);
                    }
                }
            }
            Conn::Pg(c) => {
                use postgres::SimpleQueryMessage as M;
                let msgs = c.simple_query(sql)?;
                for m in msgs {
                    match m {
                        M::Row(r) => {
                            if out.columns.is_empty() {
                                out.columns = r.columns().iter().map(|c| c.name().to_string()).collect();
                            }
                            out.rows.push((0..r.len()).map(|i| r.get(i).map(|s| s.to_string())).collect());
                        }
                        M::CommandComplete(n) => {
                            if out.columns.is_empty() {
                                out.affected = Some(n);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Conn::My(c) => {
                use mysql::prelude::Queryable;
                let mut res = c.query_iter(sql)?;
                let first = res.iter();
                if let Some(set) = first {
                    out.columns = set.columns().as_ref().iter().map(|c| c.name_str().to_string()).collect();
                    let n = out.columns.len();
                    if n > 0 {
                        for row in set {
                            let row = row?;
                            out.rows.push((0..n).map(|i| mysql_val(&row[i])).collect());
                        }
                    } else {
                        out.affected = Some(set.affected_rows());
                    }
                }
            }
            Conn::Ms(m) => out = m.exec(sql)?,
            Conn::Mongo(_) => {}
        }
        Ok(out)
    }

    // ---------------------------------------------------------------- metadata
    pub fn schemas(&mut self) -> Result<Vec<String>> {
        if let Conn::Ms(m) = &mut self.conn {
            return crate::drivers::ms_schemas(m);
        }
        let cfg_db = self.cfg.database.clone();
        if let Conn::Mongo(m) = &mut self.conn {
            return Ok(m.schemas(&cfg_db));
        }
        Ok(match self.kind() {
            Kind::SqlServer | Kind::Mongo => vec![],
            Kind::Sqlite => vec!["main".into()],
            Kind::Postgres => self
                .run("SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('pg_catalog','information_schema') AND schema_name NOT LIKE 'pg_toast%' AND schema_name NOT LIKE 'pg_temp%' ORDER BY 1", 0, 0)?
                .rows
                .into_iter()
                .filter_map(|r| r[0].clone())
                .collect(),
            Kind::Mysql => self
                .run("SHOW DATABASES", 0, 0)?
                .rows
                .into_iter()
                .filter_map(|r| r[0].clone())
                .collect(),
        })
    }

    /// (name, is_view)
    pub fn tables(&mut self, schema: &str) -> Result<Vec<(String, bool)>> {
        if let Conn::Ms(m) = &mut self.conn {
            return crate::drivers::ms_tables(m, schema);
        }
        if let Conn::Mongo(m) = &mut self.conn {
            return m.collections(schema);
        }
        let q = match self.kind() {
            Kind::SqlServer | Kind::Mongo => String::new(),
            Kind::Sqlite => "SELECT name, type FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name".to_string(),
            Kind::Postgres => format!("SELECT table_name, table_type FROM information_schema.tables WHERE table_schema = {} AND table_type <> 'FOREIGN' ORDER BY 1", lit(schema)),
            Kind::Mysql => format!("SELECT table_name, table_type FROM information_schema.tables WHERE table_schema = {} AND table_type <> 'SEQUENCE' ORDER BY 1", lit(schema)),
        };
        Ok(self
            .run(&q, 0, 0)?
            .rows
            .into_iter()
            .map(|r| {
                let n = r[0].clone().unwrap_or_default();
                let t = r.get(1).cloned().flatten().unwrap_or_default().to_lowercase();
                (n, t.contains("view"))
            })
            .collect())
    }

    pub fn qualified(&self, schema: &str, table: &str) -> String {
        let k = self.kind();
        match k {
            Kind::Sqlite => k.quote(table),
            _ => format!("{}.{}", k.quote(schema), k.quote(table)),
        }
    }

    pub fn table_meta(&mut self, schema: &str, table: &str) -> Result<TableMeta> {
        if let Conn::Ms(ms) = &mut self.conn {
            return crate::drivers::ms_meta(ms, schema, table);
        }
        if let Conn::Mongo(mg) = &mut self.conn {
            return mg.meta(schema, table);
        }
        let mut m = TableMeta::default();
        match self.kind() {
            Kind::SqlServer | Kind::Mongo => {}
            Kind::Sqlite => {
                let r = self.run(&format!("PRAGMA table_info({})", Kind::Sqlite.quote(table)), 0, 0)?;
                for row in r.rows {
                    m.columns.push(ColInfo {
                        name: row[1].clone().unwrap_or_default(),
                        typ: row[2].clone().unwrap_or_default(),
                        nullable: row[3].as_deref() != Some("1"),
                        default: row[4].clone(),
                        pk: row[5].as_deref().map(|s| s != "0").unwrap_or(false),
                        extra: String::new(),
                    });
                }
                let r = self.run(&format!("PRAGMA index_list({})", Kind::Sqlite.quote(table)), 0, 0)?;
                for row in r.rows {
                    let name = row[1].clone().unwrap_or_default();
                    let cols = self.run(&format!("PRAGMA index_info({})", Kind::Sqlite.quote(&name)), 0, 0)?;
                    m.indexes.push(IdxInfo {
                        name,
                        unique: row[2].as_deref() == Some("1"),
                        columns: cols.rows.iter().filter_map(|c| c[2].clone()).collect(),
                        typ: "BTREE".into(),
                    });
                }
                let r = self.run(&format!("PRAGMA foreign_key_list({})", Kind::Sqlite.quote(table)), 0, 0)?;
                for row in r.rows {
                    let id = row[0].clone().unwrap_or_default();
                    let col = row[3].clone().unwrap_or_default();
                    let refc = row[4].clone().unwrap_or_default();
                    if let Some(f) = m.fks.iter_mut().find(|f| f.name == format!("fk_{}_{}", table, id)) {
                        f.columns.push(col);
                        f.ref_columns.push(refc);
                    } else {
                        m.fks.push(FkInfo {
                            name: format!("fk_{}_{}", table, id),
                            columns: vec![col],
                            ref_schema: schema.into(),
                            ref_table: row[2].clone().unwrap_or_default(),
                            ref_columns: vec![refc],
                        });
                    }
                }
                let r = self.run(&format!("SELECT sql FROM sqlite_master WHERE name = {}", lit(table)), 0, 0)?;
                m.ddl = r.rows.first().and_then(|r| r[0].clone()).map(|s| format!("{};", s)).unwrap_or_default();
            }
            Kind::Postgres => {
                let q = format!(
                    "SELECT c.column_name, CASE WHEN c.data_type = 'USER-DEFINED' THEN c.udt_name WHEN c.character_maximum_length IS NOT NULL THEN c.data_type || '(' || c.character_maximum_length || ')' ELSE c.data_type END, c.is_nullable, c.column_default, \
                     (SELECT count(*) FROM information_schema.table_constraints tc JOIN information_schema.key_column_usage k ON tc.constraint_name = k.constraint_name AND tc.table_schema = k.table_schema WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = c.table_schema AND tc.table_name = c.table_name AND k.column_name = c.column_name) \
                     FROM information_schema.columns c WHERE c.table_schema = {} AND c.table_name = {} ORDER BY c.ordinal_position",
                    lit(schema),
                    lit(table)
                );
                for row in self.run(&q, 0, 0)?.rows {
                    m.columns.push(ColInfo {
                        name: row[0].clone().unwrap_or_default(),
                        typ: row[1].clone().unwrap_or_default(),
                        nullable: row[2].as_deref() == Some("YES"),
                        default: row[3].clone(),
                        pk: row[4].as_deref().map(|s| s != "0").unwrap_or(false),
                        extra: String::new(),
                    });
                }
                let q = format!("SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = {} AND tablename = {} ORDER BY 1", lit(schema), lit(table));
                for row in self.run(&q, 0, 0)?.rows {
                    let def = row[1].clone().unwrap_or_default();
                    let cols = def
                        .rfind('(')
                        .map(|i| def[i + 1..].trim_end_matches(')').split(',').map(|s| s.trim().trim_matches('"').to_string()).collect())
                        .unwrap_or_default();
                    m.indexes.push(IdxInfo { name: row[0].clone().unwrap_or_default(), unique: def.contains("UNIQUE"), columns: cols, typ: "btree".into() });
                }
                let q = format!(
                    "SELECT con.conname, pg_get_constraintdef(con.oid) FROM pg_constraint con JOIN pg_class cl ON cl.oid = con.conrelid JOIN pg_namespace n ON n.oid = cl.relnamespace WHERE con.contype = 'f' AND n.nspname = {} AND cl.relname = {}",
                    lit(schema),
                    lit(table)
                );
                for row in self.run(&q, 0, 0)?.rows {
                    if let Some(f) = parse_pg_fk(&row[0].clone().unwrap_or_default(), &row[1].clone().unwrap_or_default(), schema) {
                        m.fks.push(f);
                    }
                }
                m.ddl = build_ddl(self.kind(), schema, table, &m);
            }
            Kind::Mysql => {
                let q = format!(
                    "SELECT column_name, column_type, is_nullable, column_default, column_key, extra FROM information_schema.columns WHERE table_schema = {} AND table_name = {} ORDER BY ordinal_position",
                    lit(schema),
                    lit(table)
                );
                for row in self.run(&q, 0, 0)?.rows {
                    m.columns.push(ColInfo {
                        name: row[0].clone().unwrap_or_default(),
                        typ: row[1].clone().unwrap_or_default(),
                        nullable: row[2].as_deref() == Some("YES"),
                        default: row[3].clone(),
                        pk: row[4].as_deref() == Some("PRI"),
                        extra: row[5].clone().unwrap_or_default(),
                    });
                }
                let q = format!("SHOW INDEX FROM {}", self.qualified(schema, table));
                if let Ok(r) = self.run(&q, 0, 0) {
                    for row in r.rows {
                        let name = row[2].clone().unwrap_or_default();
                        let col = row[4].clone().unwrap_or_default();
                        if let Some(i) = m.indexes.iter_mut().find(|i| i.name == name) {
                            i.columns.push(col);
                        } else {
                            m.indexes.push(IdxInfo { name, unique: row[1].as_deref() == Some("0"), columns: vec![col], typ: row.get(10).cloned().flatten().unwrap_or_default() });
                        }
                    }
                }
                let q = format!(
                    "SELECT constraint_name, column_name, referenced_table_schema, referenced_table_name, referenced_column_name FROM information_schema.key_column_usage WHERE table_schema = {} AND table_name = {} AND referenced_table_name IS NOT NULL ORDER BY constraint_name, ordinal_position",
                    lit(schema),
                    lit(table)
                );
                for row in self.run(&q, 0, 0)?.rows {
                    let name = row[0].clone().unwrap_or_default();
                    if let Some(f) = m.fks.iter_mut().find(|f| f.name == name) {
                        f.columns.push(row[1].clone().unwrap_or_default());
                        f.ref_columns.push(row[4].clone().unwrap_or_default());
                    } else {
                        m.fks.push(FkInfo {
                            name,
                            columns: vec![row[1].clone().unwrap_or_default()],
                            ref_schema: row[2].clone().unwrap_or_default(),
                            ref_table: row[3].clone().unwrap_or_default(),
                            ref_columns: vec![row[4].clone().unwrap_or_default()],
                        });
                    }
                }
                let q = format!("SHOW CREATE TABLE {}", self.qualified(schema, table));
                if let Ok(r) = self.run(&q, 0, 0) {
                    m.ddl = r.rows.first().and_then(|r| r.get(1).cloned().flatten()).map(|s| format!("{};", s)).unwrap_or_default();
                }
            }
        }
        Ok(m)
    }

    pub fn version(&mut self) -> String {
        let q = match self.kind() {
            Kind::Sqlite => "SELECT 'SQLite ' || sqlite_version()",
            Kind::SqlServer => "SELECT @@VERSION",
            Kind::Mongo => return match &self.conn {
                Conn::Mongo(m) => m
                    .client
                    .database("admin")
                    .run_command(mongodb::bson::doc! {"buildInfo": 1})
                    .run()
                    .ok()
                    .and_then(|d| d.get_str("version").ok().map(|v| format!("MongoDB {}", v)))
                    .unwrap_or_else(|| "MongoDB".into()),
                _ => String::new(),
            },
            _ => "SELECT version()",
        };
        let v = self.run(q, 0, 0).ok().and_then(|r| r.rows.first().and_then(|r| r[0].clone())).unwrap_or_default();
        v.lines().next().unwrap_or("").trim().to_string()
    }
}

pub fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// SQL literal for a value of a column.
pub fn value_literal(v: &Option<String>, numeric: bool) -> String {
    match v {
        None => "NULL".into(),
        Some(s) => {
            if numeric && s.trim().parse::<f64>().is_ok() {
                s.trim().to_string()
            } else {
                lit(s)
            }
        }
    }
}

/// `Some(true)` for BEGIN / START TRANSACTION, `Some(false)` for COMMIT / ROLLBACK / END.
fn tx_control_kind(sql: &str) -> Option<bool> {
    match crate::sqltext::first_word(sql).to_uppercase().as_str() {
        "BEGIN" | "START" => Some(true),
        "COMMIT" | "ROLLBACK" | "END" => Some(false),
        _ => None,
    }
}

fn is_tx_control(sql: &str) -> bool {
    tx_control_kind(sql).is_some()
}

pub fn is_read_only_sql(sql: &str) -> bool {
    let w = crate::sqltext::first_word(sql).to_uppercase();
    if w == "DB" {
        return !crate::drivers::mongo_is_write(sql);
    }
    matches!(w.as_str(), "USE" | "SELECT" | "WITH" | "SHOW" | "EXPLAIN" | "DESCRIBE" | "DESC" | "PRAGMA" | "VALUES" | "BEGIN" | "COMMIT" | "ROLLBACK" | "START" | "SET")
        && !(w == "WITH" && sql.to_uppercase().contains("INSERT INTO"))
}

fn is_pageable(sql: &str) -> bool {
    let w = crate::sqltext::first_word(sql).to_uppercase();
    if !(w == "SELECT" || w == "WITH") {
        return false;
    }
    let up = sql.to_uppercase();
    !(up.contains(" LIMIT ") || up.contains(" INTO ") || up.contains(" FOR UPDATE") || up.contains("PRAGMA"))
}

fn sqlite_val(v: rusqlite::types::ValueRef) -> Option<String> {
    use rusqlite::types::ValueRef::*;
    match v {
        Null => None,
        Integer(i) => Some(i.to_string()),
        Real(f) => Some(fmt_f64(f)),
        Text(t) => Some(String::from_utf8_lossy(t).to_string()),
        Blob(b) => Some(blob_text(b)),
    }
}

fn fmt_f64(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{:.1}", f)
    } else {
        f.to_string()
    }
}

pub fn blob_text(b: &[u8]) -> String {
    // BINARY_FORMATS[0] = hex, string max len 32 (ModelPreferences.java:370)
    let mut s = String::from("0x");
    for x in b.iter().take(crate::constants::BINARY_STRING_MAX_LEN) {
        s.push_str(&format!("{:02X}", x));
    }
    if b.len() > crate::constants::BINARY_STRING_MAX_LEN {
        s.push('…');
    }
    s
}

fn mysql_val(v: &mysql::Value) -> Option<String> {
    use mysql::Value::*;
    match v {
        NULL => None,
        Bytes(b) => Some(match String::from_utf8(b.clone()) {
            Ok(s) => s,
            Err(_) => blob_text(b),
        }),
        Int(i) => Some(i.to_string()),
        UInt(u) => Some(u.to_string()),
        Float(f) => Some(f.to_string()),
        Double(d) => Some(d.to_string()),
        Date(y, mo, d, h, mi, s, us) => {
            if *h == 0 && *mi == 0 && *s == 0 && *us == 0 {
                Some(format!("{:04}-{:02}-{:02}", y, mo, d))
            } else {
                Some(format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}", y, mo, d, h, mi, s, us / 1000))
            }
        }
        Time(neg, d, h, mi, s, _) => Some(format!("{}{:02}:{:02}:{:02}", if *neg { "-" } else { "" }, d * 24 + *h as u32, mi, s)),
    }
}

fn parse_pg_fk(name: &str, def: &str, schema: &str) -> Option<FkInfo> {
    // FOREIGN KEY (a, b) REFERENCES schema.table(x, y) ...
    let up = def.to_uppercase();
    let a = up.find("FOREIGN KEY (")? + 13;
    let b = def[a..].find(')')? + a;
    let cols: Vec<String> = def[a..b].split(',').map(|s| s.trim().trim_matches('"').to_string()).collect();
    let r = up.find("REFERENCES ")? + 11;
    let p = def[r..].find('(')? + r;
    let tname = def[r..p].trim().to_string();
    let q = def[p..].find(')')? + p;
    let rcols: Vec<String> = def[p + 1..q].split(',').map(|s| s.trim().trim_matches('"').to_string()).collect();
    let (rs, rt) = match tname.rsplit_once('.') {
        Some((s, t)) => (s.trim_matches('"').to_string(), t.trim_matches('"').to_string()),
        None => (schema.to_string(), tname.trim_matches('"').to_string()),
    };
    Some(FkInfo { name: name.into(), columns: cols, ref_schema: rs, ref_table: rt, ref_columns: rcols })
}

pub fn build_ddl(kind: Kind, schema: &str, table: &str, m: &TableMeta) -> String {
    let mut s = String::new();
    let q = |x: &str| kind.quote(x);
    let full = if kind == Kind::Sqlite { q(table) } else { format!("{}.{}", q(schema), q(table)) };
    s.push_str(&format!("CREATE TABLE {} (\n", full));
    let mut parts: Vec<String> = Vec::new();
    for c in &m.columns {
        let mut l = format!("\t{} {}", q(&c.name), c.typ);
        if let Some(d) = &c.default {
            l.push_str(&format!(" DEFAULT {}", d));
        }
        if !c.nullable {
            l.push_str(" NOT NULL");
        }
        parts.push(l);
    }
    let pks: Vec<String> = m.columns.iter().filter(|c| c.pk).map(|c| q(&c.name)).collect();
    if !pks.is_empty() {
        parts.push(format!("\tCONSTRAINT {}_pkey PRIMARY KEY ({})", table, pks.join(", ")));
    }
    for f in &m.fks {
        parts.push(format!(
            "\tCONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
            q(&f.name),
            f.columns.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "),
            if kind == Kind::Sqlite { q(&f.ref_table) } else { format!("{}.{}", q(&f.ref_schema), q(&f.ref_table)) },
            f.ref_columns.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")
        ));
    }
    s.push_str(&parts.join(",\n"));
    s.push_str("\n);\n");
    for i in &m.indexes {
        if i.name.ends_with("_pkey") || i.name.starts_with("sqlite_autoindex") {
            continue;
        }
        s.push_str(&format!(
            "\nCREATE {}INDEX {} ON {} ({});",
            if i.unique { "UNIQUE " } else { "" },
            q(&i.name),
            full,
            i.columns.iter().map(|c| q(c)).collect::<Vec<_>>().join(", ")
        ));
    }
    s
}

/// "localhost:3337" typed in the server field -> ("localhost", 3337). IPv6 literals are left alone.
pub fn split_host_port(host: &str, default_port: u16) -> (String, u16) {
    let h = host.trim();
    if h.matches(':').count() == 1 {
        if let Some((a, b)) = h.split_once(':') {
            if let Ok(p) = b.trim().parse::<u16>() {
                return (a.trim().to_string(), p);
            }
        }
    }
    (h.to_string(), default_port)
}

/// MySQL / MariaDB, including old servers:
/// - `localhost` is tried on IPv4 as well (servers bound to 127.0.0.1 refuse ::1)
/// - the pre-4.1 password format (`mysql_old_password`) is refused by the driver by default; it is enabled
///   automatically when the server asks for it. Returns (connection, used_legacy_auth).
fn connect_mysql(cfg: &ConnConfig) -> Result<(mysql::Conn, bool)> {
    let (host, port) = split_host_port(&cfg.host, cfg.port);
    let hosts: Vec<String> = if host.eq_ignore_ascii_case("localhost") { vec!["localhost".into(), "127.0.0.1".into()] } else { vec![host.clone()] };
    let mut last: Option<anyhow::Error> = None;
    for h in hosts {
        let mut legacy = false;
        loop {
            let mut b = mysql::OptsBuilder::new()
                .ip_or_hostname(Some(h.clone()))
                .tcp_port(port)
                .user(Some(cfg.user.clone()))
                .pass(Some(deobf(&cfg.password)))
                .secure_auth(!legacy)
                // the driver asks old servers for `@@socket` to switch to a unix socket on localhost; they answer
                // "Unknown system variable 'socket'". Plain TCP works everywhere.
                .prefer_socket(false)
                .enable_cleartext_plugin(true)
                .tcp_connect_timeout(Some(std::time::Duration::from_secs(crate::constants::CONNECT_TIMEOUT_SECS)));
            if !cfg.database.is_empty() {
                b = b.db_name(Some(cfg.database.clone()));
            }
            match mysql::Conn::new(b) {
                Ok(c) => return Ok((c, legacy)),
                Err(e) => {
                    let old_pw = matches!(&e, mysql::Error::DriverError(mysql::error::DriverError::OldMysqlPasswordDisabled));
                    if !legacy && old_pw {
                        legacy = true; // retry once with mysql_old_password allowed
                        continue;
                    }
                    last = Some(anyhow::Error::from(e));
                    break;
                }
            }
        }
        // a wrong user/password/database is the same on 127.0.0.1: do not try the next address
        if let Some(e) = &last {
            if e.to_string().contains("ERROR 1045") || e.to_string().contains("ERROR 1044") || e.to_string().contains("ERROR 1049") {
                break;
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("Não foi possível conectar")))
}

pub fn clean_err(e: anyhow::Error) -> anyhow::Error {
    let t = e.to_string();
    if t.contains("ERROR 1045") {
        return anyhow!("Acesso negado: usuário ou senha incorretos (ou o usuário não pode entrar a partir desta máquina).\n{}", t);
    }
    if t.contains("ERROR 1049") {
        return anyhow!("O banco de dados informado não existe no servidor.\n{}", t);
    }
    if t.to_lowercase().contains("connection refused") {
        return anyhow!("Conexão recusada: confirme servidor e porta, e se o banco está rodando.\n{}", t);
    }
    if t.to_lowercase().contains("unknownauthplugin") || t.contains("Unknown authentication plugin") {
        return anyhow!("O servidor usa um método de autenticação que este app ainda não suporta.\n{}", t);
    }
    let text = e.to_string();
    if text.contains("Server selection timeout") {
        return anyhow!("Não foi possível conectar ao MongoDB: servidor inacessível, porta errada ou credenciais inválidas");
    }
    if let Some(pe) = e.downcast_ref::<postgres::Error>() {
        if let Some(db) = pe.as_db_error() {
            let mut m = format!("{}: {}", db.severity(), db.message());
            if let Some(h) = db.hint() {
                m.push_str(&format!("\nDica: {}", h));
            }
            if let Some(p) = db.position() {
                let _ = p;
            }
            return anyhow!(m);
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str) -> Session {
        let p = std::env::temp_dir().join(format!("dbr-test-{}.sqlite", name));
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        Session::open(&c).unwrap()
    }

    #[test]
    fn badge_style_defaults_and_customisation() {
        let mut c = ConnConfig::new(Kind::Postgres);
        let s = c.badge_style();
        assert_eq!((s.label.as_str(), s.icon.clone(), s.color), ("Pg", None, (51, 103, 145)));
        c.color = Some([200, 10, 20]);
        c.icon = Some("database".into());
        let s = c.badge_style();
        assert_eq!((s.icon.as_deref(), s.color), (Some("database"), (200, 10, 20)));
        c.icon = Some("PRODUCAO".into());
        assert_eq!(c.badge_style().label, "PRO", "labels are cut to 3 characters");
        assert_eq!(c.badge_style().icon, None);
        c.reset_appearance();
        assert_eq!(c.badge_style(), ConnConfig::new(Kind::Postgres).badge_style());
        // old files have neither field
        let mut v = serde_json::to_value(&c).unwrap();
        v.as_object_mut().unwrap().remove("color");
        v.as_object_mut().unwrap().remove("icon");
        assert!(serde_json::from_value::<ConnConfig>(v).is_ok());
    }

    #[test]
    fn contrast_picks_readable_text() {
        use egui::Color32;
        assert_eq!(crate::icons::contrast(Color32::from_rgb(255, 235, 59)), Color32::from_rgb(25, 28, 34), "dark text on yellow");
        assert_eq!(crate::icons::contrast(Color32::from_rgb(20, 40, 90)), Color32::WHITE, "white text on navy");
        assert_eq!(crate::icons::contrast(Color32::from_rgb(190, 60, 60)), Color32::WHITE);
    }

    #[test]
    fn paging_and_meta() {
        let mut s = open("paging_and_meta");
        let r = s.run("SELECT * FROM orders", 100, 0).unwrap();
        assert_eq!(r.rows.len(), 100);
        assert!(r.has_more);
        let r2 = s.run("SELECT * FROM orders", 100, 100).unwrap();
        assert_eq!(r2.rows.len(), 50);
        assert!(!r2.has_more);
        let m = s.table_meta("main", "orders").unwrap();
        assert_eq!(m.columns.len(), 4);
        assert!(m.columns[0].pk);
        assert_eq!(m.fks.len(), 1);
        assert_eq!(m.fks[0].ref_table, "customers");
        assert!(m.indexes.iter().any(|i| i.name == "idx_orders_customer"));
        assert!(m.ddl.contains("CREATE TABLE"));
        let t = s.tables("main").unwrap();
        assert!(t.iter().any(|(n, v)| n == "customer_totals" && *v));
        assert!(!s.schemas().unwrap().is_empty());
    }

    #[test]
    fn manual_commit_rollback() {
        let mut s = open("manual_commit_rollback");
        s.set_auto_commit(false).unwrap();
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        assert!(s.tx_open);
        assert_eq!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
        s.rollback().unwrap();
        assert_ne!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
    }

    #[test]
    fn old_connection_files_migrate_to_a_mode() {
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.auto_commit = false;
        let mut v = serde_json::to_value(&c).unwrap();
        v.as_object_mut().unwrap().remove("tx_mode");
        let old: ConnConfig = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(old.mode(), TxMode::Manual);
        v["auto_commit"] = serde_json::json!(true);
        let old: ConnConfig = serde_json::from_value(v).unwrap();
        assert_eq!(old.mode(), TxMode::Auto);
        let mut n = ConnConfig::new(Kind::Sqlite);
        n.set_mode(TxMode::Smart);
        let back: ConnConfig = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        assert_eq!(back.mode(), TxMode::Smart);
    }

    #[test]
    fn auto_mode_commits_each_statement() {
        let mut s = open("tx_auto");
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        assert!(!s.tx_open);
        assert_eq!(s.tx_count, 0);
    }

    #[test]
    fn manual_mode_opens_a_transaction_even_for_select() {
        let mut s = open("tx_manual_select");
        s.set_mode(TxMode::Manual).unwrap();
        s.run("SELECT COUNT(*) FROM orders", 0, 0).unwrap();
        assert!(s.tx_open);
        assert_eq!(s.tx_count, 0, "a read is not a pending change");
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        assert_eq!(s.tx_count, 1);
        s.commit().unwrap();
        assert!(!s.tx_open);
        assert_eq!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
    }

    #[test]
    fn smart_mode_reads_directly_and_writes_stay_pending() {
        let mut s = open("tx_smart");
        s.set_mode(TxMode::Smart).unwrap();
        s.run("SELECT * FROM orders", 0, 0).unwrap();
        s.run("PRAGMA table_info(orders)", 0, 0).unwrap();
        assert!(!s.tx_open, "reads run without a transaction");
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        assert!(s.tx_open);
        assert_eq!(s.tx_count, 1);
        // a read during pending changes sees them and does not commit them
        assert_eq!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
        assert!(s.tx_open);
        s.rollback().unwrap();
        assert_ne!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
    }

    #[test]
    fn switching_to_auto_commits_pending_changes() {
        let mut s = open("tx_switch");
        s.set_mode(TxMode::Smart).unwrap();
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        s.set_mode(TxMode::Auto).unwrap();
        assert!(!s.tx_open);
        assert_eq!(s.run("SELECT COUNT(*) FROM order_items", 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
    }

    #[test]
    fn explicit_transaction_statements_are_tracked() {
        let mut s = open("tx_explicit");
        s.set_mode(TxMode::Manual).unwrap();
        s.run("DELETE FROM order_items", 0, 0).unwrap();
        assert!(s.tx_open);
        s.run("COMMIT", 0, 0).unwrap();
        assert!(!s.tx_open);
        assert_eq!(s.tx_count, 0);
    }

    #[test]
    fn pending_rows_are_summed_in_manual_mode() {
        let mut s = open("tx_rows");
        s.set_mode(TxMode::Manual).unwrap();
        s.run("UPDATE products SET in_stock = 1 WHERE product_id <= 3", 0, 0).unwrap();
        s.run("UPDATE products SET in_stock = 0 WHERE product_id <= 2", 0, 0).unwrap();
        assert_eq!((s.tx_count, s.tx_rows), (2, 5));
        s.rollback().unwrap();
        assert_eq!((s.tx_count, s.tx_rows), (0, 0));
    }

    #[test]
    fn errors_are_readable() {
        let mut s = open("errors_are_readable");
        let e = s.run("SELECT * FROM nope", 0, 0).unwrap_err().to_string();
        assert!(e.contains("no such table"), "{}", e);
    }

    #[test]
    fn dml_affected() {
        let mut s = open("dml_affected");
        let r = s.run("UPDATE products SET in_stock = 1 WHERE product_id <= 3", 0, 0).unwrap();
        assert_eq!(r.affected, Some(3));
    }
}

#[cfg(test)]
mod pg_tests {
    use super::*;

    /// Runs only when DBR_PG_PORT is set (a local PostgreSQL server).
    #[test]
    fn postgres_smoke() {
        let Ok(port) = std::env::var("DBR_PG_PORT") else { return };
        let mut c = ConnConfig::new(Kind::Postgres);
        c.port = port.parse().unwrap();
        c.database = "postgres".into();
        c.host = "127.0.0.1".into();
        let mut s = Session::open(&c).expect("connect");
        s.run("DROP TABLE IF EXISTS dbr_a CASCADE", 0, 0).unwrap();
        s.run("DROP TABLE IF EXISTS dbr_b CASCADE", 0, 0).unwrap();
        s.run("CREATE TABLE dbr_a (id serial PRIMARY KEY, name varchar(20) NOT NULL, born date)", 0, 0).unwrap();
        s.run("CREATE TABLE dbr_b (id int PRIMARY KEY, a_id int REFERENCES dbr_a(id))", 0, 0).unwrap();
        s.run("CREATE INDEX dbr_b_idx ON dbr_b (a_id)", 0, 0).unwrap();
        let r = s.run("INSERT INTO dbr_a (name, born) SELECT 'n' || g, DATE '2020-01-01' + g FROM generate_series(1, 250) g", 0, 0).unwrap();
        assert_eq!(r.affected, Some(250));
        let r = s.run("SELECT * FROM dbr_a ORDER BY id", 200, 0).unwrap();
        assert_eq!(r.rows.len(), 200);
        assert!(r.has_more);
        assert_eq!(r.columns, vec!["id", "name", "born"]);
        let e = s.run("SELECT * FROM nope", 0, 0).unwrap_err().to_string();
        assert!(e.contains("nope"), "{}", e);
        assert!(s.schemas().unwrap().contains(&"public".to_string()));
        assert!(s.tables("public").unwrap().iter().any(|t| t.0 == "dbr_a"));
        let m = s.table_meta("public", "dbr_b").unwrap();
        assert_eq!(m.columns.len(), 2);
        assert_eq!(m.fks.len(), 1, "{:?}", m.fks);
        assert_eq!(m.fks[0].ref_table, "dbr_a");
        assert!(m.indexes.iter().any(|i| i.name == "dbr_b_idx" && i.columns == vec!["a_id"]));
        let ma = s.table_meta("public", "dbr_a").unwrap();
        assert!(ma.columns[0].pk && !ma.columns[1].nullable);
        println!("{}", build_ddl(Kind::Postgres, "public", "dbr_b", &m));
        s.set_auto_commit(false).unwrap();
        s.run("DELETE FROM dbr_a", 0, 0).unwrap();
        s.rollback().unwrap();
        assert_eq!(s.run("SELECT count(*) FROM dbr_a", 0, 0).unwrap().rows[0][0].as_deref(), Some("250"));
        s.set_auto_commit(true).unwrap();
        s.run("DROP TABLE dbr_b", 0, 0).unwrap();
        s.run("DROP TABLE dbr_a", 0, 0).unwrap();
    }
}

#[cfg(test)]
mod offline_driver_tests {
    use super::*;

    #[test]
    fn sql_server_unreachable_gives_a_readable_error() {
        let mut c = ConnConfig::new(Kind::SqlServer);
        c.host = "127.0.0.1".into();
        c.port = 1; // nothing listens here
        let e = Session::open(&c).err().expect("must fail").to_string();
        assert!(!e.is_empty(), "{}", e);
        println!("sql server error: {}", e);
    }

    #[test]
    #[ignore] // waits for the server-selection timeout
    fn mongo_unreachable_gives_a_readable_error() {
        let mut c = ConnConfig::new(Kind::Mongo);
        c.host = "127.0.0.1".into();
        c.port = 1;
        let e = Session::open(&c).err().expect("must fail").to_string();
        println!("mongo error: {}", e);
        assert!(!e.is_empty());
    }
}

// ================================================================ schema objects (navigator folders)
// Folders follow the navigator definitions of the DBeaver plugins:
// ext.postgresql/plugin.xml:141-333, ext.mysql/plugin.xml:26-151, ext.mssql/plugin.xml:50-164.
#[derive(Clone, Debug)]
pub struct ObjInfo {
    pub name: String,
    pub detail: String,
}

pub struct FolderDef {
    pub key: &'static str,
    pub label: &'static str,
}

const fn f(key: &'static str, label: &'static str) -> FolderDef {
    FolderDef { key, label }
}

/// Folders shown inside each schema, after Tables and Views.
pub fn schema_folders(kind: Kind) -> Vec<FolderDef> {
    match kind {
        Kind::Sqlite => vec![f("index", "Índices"), f("trigger", "Gatilhos")],
        Kind::Postgres => vec![
            f("matview", "Views materializadas"),
            f("foreign", "Tabelas externas"),
            f("index", "Índices"),
            f("function", "Funções"),
            f("procedure", "Procedimentos"),
            f("aggregate", "Funções agregadas"),
            f("sequence", "Sequências"),
            f("trigger", "Gatilhos"),
            f("type", "Tipos de dados"),
        ],
        Kind::Mysql => vec![f("index", "Índices"), f("procedure", "Procedimentos"), f("function", "Funções"), f("trigger", "Gatilhos"), f("event", "Eventos"), f("sequence", "Sequências")],
        Kind::SqlServer => vec![
            f("index", "Índices"),
            f("procedure", "Procedimentos"),
            f("function", "Funções"),
            f("sequence", "Sequências"),
            f("synonym", "Sinônimos"),
            f("trigger", "Gatilhos"),
            f("type", "Tipos de dados"),
        ],
        Kind::Mongo => vec![],
    }
}

/// Folders at the level of the whole database / server.
pub fn db_folders(kind: Kind) -> Vec<FolderDef> {
    match kind {
        Kind::Postgres => vec![f("extension", "Extensões"), f("tablespace", "Tablespaces"), f("role", "Roles")],
        Kind::Mysql => vec![f("user", "Usuários"), f("engine", "Engines"), f("charset", "Charsets")],
        Kind::SqlServer => vec![f("dbtrigger", "Gatilhos do banco"), f("login", "Logins")],
        _ => vec![],
    }
}

fn pairs(r: QueryResult) -> Vec<ObjInfo> {
    r.rows.into_iter().map(|r| ObjInfo { name: r[0].clone().unwrap_or_default(), detail: r.get(1).cloned().flatten().unwrap_or_default() }).collect()
}

impl Session {
    /// Lists the objects of one folder. `schema` is empty for the database-level folders.
    pub fn objects(&mut self, key: &str, schema: &str) -> Result<Vec<ObjInfo>> {
        let s = lit(schema);
        let k = self.kind();
        let q: String = match (k, key) {
            (Kind::Sqlite, "index") => "SELECT name, tbl_name FROM sqlite_master WHERE type = 'index' AND name NOT LIKE 'sqlite_%' ORDER BY name".into(),
            (Kind::Sqlite, "trigger") => "SELECT name, tbl_name FROM sqlite_master WHERE type = 'trigger' ORDER BY name".into(),

            (Kind::Postgres, "matview") => format!("SELECT matviewname, '' FROM pg_matviews WHERE schemaname = {} ORDER BY 1", s),
            (Kind::Postgres, "foreign") => format!("SELECT c.relname, '' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind = 'f' AND n.nspname = {} ORDER BY 1", s),
            (Kind::Postgres, "index") => format!("SELECT indexname, tablename FROM pg_indexes WHERE schemaname = {} ORDER BY 1", s),
            (Kind::Postgres, "function") => format!("SELECT p.proname, pg_get_function_identity_arguments(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = {} AND p.prokind = 'f' ORDER BY 1, 2", s),
            (Kind::Postgres, "procedure") => format!("SELECT p.proname, pg_get_function_identity_arguments(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = {} AND p.prokind = 'p' ORDER BY 1, 2", s),
            (Kind::Postgres, "aggregate") => format!("SELECT p.proname, pg_get_function_identity_arguments(p.oid) FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = {} AND p.prokind = 'a' ORDER BY 1, 2", s),
            (Kind::Postgres, "sequence") => format!("SELECT sequencename, '' FROM pg_sequences WHERE schemaname = {} ORDER BY 1", s),
            (Kind::Postgres, "trigger") => format!("SELECT t.tgname, c.relname FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = {} AND NOT t.tgisinternal ORDER BY 1", s),
            (Kind::Postgres, "type") => format!(
                "SELECT t.typname, CASE t.typtype WHEN 'e' THEN 'enum' WHEN 'd' THEN 'domain' ELSE 'composto' END FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
                 WHERE n.nspname = {} AND (t.typtype IN ('e','d') OR (t.typtype = 'c' AND EXISTS (SELECT 1 FROM pg_class c WHERE c.oid = t.typrelid AND c.relkind = 'c'))) ORDER BY 1",
                s
            ),
            (Kind::Postgres, "extension") => "SELECT extname, extversion FROM pg_extension ORDER BY 1".into(),
            (Kind::Postgres, "tablespace") => "SELECT spcname, '' FROM pg_tablespace ORDER BY 1".into(),
            (Kind::Postgres, "role") => "SELECT rolname, CASE WHEN rolsuper THEN 'superusuário' WHEN rolcanlogin THEN 'login' ELSE '' END FROM pg_roles WHERE rolname !~ '^pg_' ORDER BY 1".into(),

            (Kind::Mysql, "index") => format!("SELECT DISTINCT index_name, table_name FROM information_schema.statistics WHERE table_schema = {} ORDER BY 1, 2", s),
            (Kind::Mysql, "procedure") => format!("SELECT routine_name, '' FROM information_schema.routines WHERE routine_schema = {} AND routine_type = 'PROCEDURE' ORDER BY 1", s),
            (Kind::Mysql, "function") => format!("SELECT routine_name, '' FROM information_schema.routines WHERE routine_schema = {} AND routine_type = 'FUNCTION' ORDER BY 1", s),
            (Kind::Mysql, "trigger") => format!("SELECT trigger_name, event_object_table FROM information_schema.triggers WHERE trigger_schema = {} ORDER BY 1", s),
            (Kind::Mysql, "event") => format!("SELECT event_name, status FROM information_schema.events WHERE event_schema = {} ORDER BY 1", s),
            (Kind::Mysql, "sequence") => format!("SELECT table_name, '' FROM information_schema.tables WHERE table_schema = {} AND table_type = 'SEQUENCE' ORDER BY 1", s),
            (Kind::Mysql, "user") => "SELECT DISTINCT CONCAT(user, '@', host), '' FROM mysql.user ORDER BY 1".into(),
            (Kind::Mysql, "engine") => "SHOW ENGINES".into(),
            (Kind::Mysql, "charset") => "SHOW CHARACTER SET".into(),

            (Kind::SqlServer, "index") => format!(
                "SELECT i.name, o.name FROM sys.indexes i JOIN sys.objects o ON o.object_id = i.object_id WHERE o.type = 'U' AND i.name IS NOT NULL AND o.schema_id = SCHEMA_ID({}) ORDER BY 1, 2",
                s
            ),
            (Kind::SqlServer, "procedure") => format!("SELECT name, '' FROM sys.procedures WHERE schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "function") => format!("SELECT name, type_desc FROM sys.objects WHERE type IN ('FN','IF','TF','FS','FT') AND schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "sequence") => format!("SELECT name, '' FROM sys.sequences WHERE schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "synonym") => format!("SELECT name, base_object_name FROM sys.synonyms WHERE schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "trigger") => format!("SELECT t.name, o.name FROM sys.triggers t JOIN sys.objects o ON o.object_id = t.parent_id WHERE t.parent_class = 1 AND o.schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "type") => format!("SELECT name, '' FROM sys.types WHERE is_user_defined = 1 AND schema_id = SCHEMA_ID({}) ORDER BY 1", s),
            (Kind::SqlServer, "dbtrigger") => "SELECT name, '' FROM sys.triggers WHERE parent_class = 0 ORDER BY 1".into(),
            (Kind::SqlServer, "login") => "SELECT name, type_desc FROM sys.server_principals WHERE type IN ('S','U','G') AND name NOT LIKE '##%' ORDER BY 1".into(),
            _ => return Ok(vec![]),
        };
        Ok(pairs(self.run(&q, 0, 0)?))
    }

    /// Source code / definition of an object, as text to open in the SQL editor.
    pub fn object_source(&mut self, key: &str, schema: &str, name: &str, detail: &str) -> Result<String> {
        let k = self.kind();
        let (s, n, d) = (lit(schema), lit(name), lit(detail));
        let first = |r: QueryResult| -> Option<String> { r.rows.into_iter().next().and_then(|r| r.into_iter().flatten().find(|v| !v.is_empty())) };
        // SHOW CREATE ... returns several columns (name, sql_mode, definition...): take the one holding the CREATE statement
        let create = |r: QueryResult| -> Option<String> { r.rows.into_iter().next().and_then(|r| r.into_iter().flatten().find(|v| v.to_uppercase().contains("CREATE"))) };
        let text: Option<String> = match (k, key) {
            (Kind::Sqlite, _) => first(self.run(&format!("SELECT sql FROM sqlite_master WHERE name = {}", n), 0, 0)?).map(|t| format!("{};", t)),

            (Kind::Postgres, "view") => first(self.run(&format!("SELECT 'CREATE OR REPLACE VIEW ' || quote_ident({s}) || '.' || quote_ident({n}) || ' AS' || E'\\n' || pg_get_viewdef((quote_ident({s}) || '.' || quote_ident({n}))::regclass, true)"), 0, 0)?),
            (Kind::Postgres, "matview") => first(self.run(&format!("SELECT 'CREATE MATERIALIZED VIEW ' || quote_ident(schemaname) || '.' || quote_ident(matviewname) || ' AS' || E'\\n' || definition FROM pg_matviews WHERE schemaname = {s} AND matviewname = {n}"), 0, 0)?),
            (Kind::Postgres, "index") => first(self.run(&format!("SELECT indexdef || ';' FROM pg_indexes WHERE schemaname = {s} AND indexname = {n}"), 0, 0)?),
            (Kind::Postgres, "function") | (Kind::Postgres, "procedure") => first(self.run(
                &format!("SELECT pg_get_functiondef(p.oid) FROM pg_proc p JOIN pg_namespace ns ON ns.oid = p.pronamespace WHERE ns.nspname = {s} AND p.proname = {n} AND pg_get_function_identity_arguments(p.oid) = {d}"),
                0,
                0,
            )?),
            (Kind::Postgres, "aggregate") => Some(format!("-- função agregada {}({})", name, detail)),
            (Kind::Postgres, "sequence") => first(self.run(
                &format!(
                    "SELECT 'CREATE SEQUENCE ' || quote_ident(schemaname) || '.' || quote_ident(sequencename) || E'\\n\\tINCREMENT BY ' || increment_by || E'\\n\\tMINVALUE ' || min_value || E'\\n\\tMAXVALUE ' || max_value || E'\\n\\tSTART WITH ' || start_value || E'\\n\\t' || CASE WHEN cycle THEN 'CYCLE' ELSE 'NO CYCLE' END || ';' FROM pg_sequences WHERE schemaname = {s} AND sequencename = {n}"
                ),
                0,
                0,
            )?),
            (Kind::Postgres, "trigger") => first(self.run(
                &format!("SELECT pg_get_triggerdef(t.oid, true) || ';' FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid JOIN pg_namespace ns ON ns.oid = c.relnamespace WHERE ns.nspname = {s} AND t.tgname = {n} AND c.relname = {d}"),
                0,
                0,
            )?),
            (Kind::Postgres, "type") => first(self.run(
                &format!(
                    "SELECT 'CREATE TYPE ' || quote_ident(ns.nspname) || '.' || quote_ident(t.typname) || ' AS ENUM (' || string_agg(quote_literal(e.enumlabel), ', ' ORDER BY e.enumsortorder) || ');' FROM pg_type t JOIN pg_namespace ns ON ns.oid = t.typnamespace JOIN pg_enum e ON e.enumtypid = t.oid WHERE ns.nspname = {s} AND t.typname = {n} GROUP BY ns.nspname, t.typname"
                ),
                0,
                0,
            )?)
            .or_else(|| Some(format!("-- tipo {} ({})", name, detail))),
            (Kind::Postgres, "extension") => Some(format!("CREATE EXTENSION IF NOT EXISTS {};  -- versão {}", k.quote(name), detail)),
            (Kind::Postgres, "role") => Some(format!("CREATE ROLE {};  -- {}", k.quote(name), detail)),
            (Kind::Postgres, "tablespace") => Some(format!("-- tablespace {}", name)),

            (Kind::Mysql, "view") | (Kind::Mysql, "table") => create(self.run(&format!("SHOW CREATE TABLE {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "procedure") => create(self.run(&format!("SHOW CREATE PROCEDURE {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "function") => create(self.run(&format!("SHOW CREATE FUNCTION {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "trigger") => create(self.run(&format!("SHOW CREATE TRIGGER {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "event") => create(self.run(&format!("SHOW CREATE EVENT {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "sequence") => create(self.run(&format!("SHOW CREATE SEQUENCE {}.{}", k.quote(schema), k.quote(name)), 0, 0)?),
            (Kind::Mysql, "index") => {
                let r = self.run(
                    &format!("SELECT GROUP_CONCAT(column_name ORDER BY seq_in_index SEPARATOR ', '), MIN(non_unique) FROM information_schema.statistics WHERE table_schema = {s} AND table_name = {d} AND index_name = {n}"),
                    0,
                    0,
                )?;
                r.rows.first().map(|row| format!("CREATE {}INDEX {} ON {} ({});", if row[1].as_deref() == Some("0") { "UNIQUE " } else { "" }, k.quote(name), k.quote(detail), row[0].clone().unwrap_or_default()))
            }
            (Kind::Mysql, "user") => Some(format!("SHOW GRANTS FOR '{}';", name.replacen('@', "'@'", 1))),
            (Kind::Mysql, _) => Some(format!("-- {}", name)),

            (Kind::SqlServer, "view") | (Kind::SqlServer, "procedure") | (Kind::SqlServer, "function") | (Kind::SqlServer, "trigger") => {
                let obj = format!("[{}].[{}]", schema, name);
                first(self.run(&format!("SELECT OBJECT_DEFINITION(OBJECT_ID({}))", lit(&obj)), 0, 0)?)
            }
            (Kind::SqlServer, "dbtrigger") => first(self.run(&format!("SELECT OBJECT_DEFINITION(object_id) FROM sys.triggers WHERE name = {n} AND parent_class = 0"), 0, 0)?),
            (Kind::SqlServer, "sequence") => first(self.run(
                &format!("SELECT 'CREATE SEQUENCE [' + {s} + '].[' + name + '] START WITH ' + CAST(start_value AS varchar(40)) + ' INCREMENT BY ' + CAST(increment AS varchar(40)) + ';' FROM sys.sequences WHERE name = {n} AND schema_id = SCHEMA_ID({s})"),
                0,
                0,
            )?),
            (Kind::SqlServer, "synonym") => Some(format!("CREATE SYNONYM [{}].[{}] FOR {};", schema, name, detail)),
            (Kind::SqlServer, "index") => {
                let r = self.run(
                    &format!(
                        "SELECT i.is_unique, STRING_AGG(c.name, ', ') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE i.object_id = OBJECT_ID({}) AND i.name = {n} AND ic.is_included_column = 0 GROUP BY i.is_unique",
                        lit(&format!("[{}].[{}]", schema, detail))
                    ),
                    0,
                    0,
                )?;
                r.rows.first().map(|row| format!("CREATE {}INDEX [{}] ON [{}].[{}] ({});", if matches!(row[0].as_deref(), Some("1")) { "UNIQUE " } else { "" }, name, schema, detail, row[1].clone().unwrap_or_default()))
            }
            (Kind::SqlServer, _) => Some(format!("-- {} {}", key, name)),
            _ => None,
        };
        Ok(text.unwrap_or_else(|| format!("-- sem definição disponível para {}", name)))
    }
}


#[cfg(test)]
mod object_tests {
    use super::*;

    #[test]
    fn sqlite_objects_and_sources() {
        let p = std::env::temp_dir().join("dbr-test-objects.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        let mut s = Session::open(&c).unwrap();
        s.run("CREATE TRIGGER trg_stock AFTER INSERT ON order_items BEGIN UPDATE products SET in_stock = in_stock - NEW.quantity WHERE product_id = NEW.product_id; END", 0, 0).unwrap();
        let idx = s.objects("index", "main").unwrap();
        assert!(idx.iter().any(|o| o.name == "idx_orders_customer" && o.detail == "orders"), "{:?}", idx);
        let trg = s.objects("trigger", "main").unwrap();
        assert_eq!(trg.len(), 2, "the sample trigger and the new one");
        assert!(trg.iter().all(|t| t.detail == "order_items"));
        let src = s.object_source("trigger", "main", "trg_stock", "order_items").unwrap();
        assert!(src.contains("CREATE TRIGGER trg_stock") && src.ends_with(';'), "{}", src);
        let v = s.object_source("view", "main", "customer_totals", "").unwrap();
        assert!(v.contains("CREATE VIEW customer_totals"));
        assert!(schema_folders(Kind::Sqlite).iter().any(|f| f.key == "trigger"));
        assert!(schema_folders(Kind::Postgres).iter().any(|f| f.key == "procedure"));
        assert!(schema_folders(Kind::Mysql).iter().any(|f| f.key == "event"));
        assert!(schema_folders(Kind::SqlServer).iter().any(|f| f.key == "synonym"));
    }

    /// Runs only when DBR_PG_PORT is set (a local PostgreSQL server).
    #[test]
    fn postgres_objects_and_sources() {
        let Ok(port) = std::env::var("DBR_PG_PORT") else { return };
        let mut c = ConnConfig::new(Kind::Postgres);
        c.port = port.parse().unwrap();
        c.database = "postgres".into();
        c.host = "127.0.0.1".into();
        let mut s = Session::open(&c).unwrap();
        for q in [
            "DROP SCHEMA IF EXISTS dbr_obj CASCADE",
            "CREATE SCHEMA dbr_obj",
            "CREATE TABLE dbr_obj.t (id serial PRIMARY KEY, v int)",
            "CREATE VIEW dbr_obj.vw AS SELECT id FROM dbr_obj.t",
            "CREATE MATERIALIZED VIEW dbr_obj.mv AS SELECT id FROM dbr_obj.t",
            "CREATE SEQUENCE dbr_obj.seq1 START 10 INCREMENT 5",
            "CREATE FUNCTION dbr_obj.soma(a int, b int) RETURNS int LANGUAGE sql AS 'SELECT a + b'",
            "CREATE PROCEDURE dbr_obj.limpa() LANGUAGE sql AS 'DELETE FROM dbr_obj.t'",
            "CREATE FUNCTION dbr_obj.trg() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'",
            "CREATE TRIGGER t_ins BEFORE INSERT ON dbr_obj.t FOR EACH ROW EXECUTE FUNCTION dbr_obj.trg()",
            "CREATE TYPE dbr_obj.cor AS ENUM ('azul', 'verde')",
        ] {
            s.run(q, 0, 0).unwrap_or_else(|e| panic!("{}: {}", q, e));
        }
        let names = |s: &mut Session, k: &str| s.objects(k, "dbr_obj").unwrap().into_iter().map(|o| format!("{}({})", o.name, o.detail)).collect::<Vec<_>>();
        assert_eq!(names(&mut s, "function").iter().filter(|n| n.starts_with("soma")).count(), 1);
        assert!(names(&mut s, "function").contains(&"soma(a integer, b integer)".to_string()), "{:?}", names(&mut s, "function"));
        assert_eq!(names(&mut s, "procedure"), vec!["limpa()"]);
        assert_eq!(names(&mut s, "sequence"), vec!["seq1()", "t_id_seq()"], "serial columns own a sequence too");
        assert_eq!(names(&mut s, "matview"), vec!["mv()"]);
        assert_eq!(names(&mut s, "trigger"), vec!["t_ins(t)"]);
        assert_eq!(names(&mut s, "type"), vec!["cor(enum)"]);
        assert!(names(&mut s, "index").iter().any(|n| n.starts_with("t_pkey")));
        assert!(s.tables("dbr_obj").unwrap().iter().all(|(n, _)| n != "mv"), "materialized views are not tables");
        let f = s.object_source("function", "dbr_obj", "soma", "a integer, b integer").unwrap();
        assert!(f.contains("CREATE OR REPLACE FUNCTION dbr_obj.soma"), "{}", f);
        assert!(s.object_source("procedure", "dbr_obj", "limpa", "").unwrap().contains("CREATE OR REPLACE PROCEDURE"));
        assert!(s.object_source("sequence", "dbr_obj", "seq1", "").unwrap().contains("INCREMENT BY 5"));
        assert!(s.object_source("trigger", "dbr_obj", "t_ins", "t").unwrap().contains("CREATE TRIGGER t_ins"));
        assert!(s.object_source("type", "dbr_obj", "cor", "enum").unwrap().contains("ENUM ('azul', 'verde')"));
        assert!(s.object_source("view", "dbr_obj", "vw", "").unwrap().contains("CREATE OR REPLACE VIEW dbr_obj.vw"));
        assert!(s.object_source("matview", "dbr_obj", "mv", "").unwrap().contains("CREATE MATERIALIZED VIEW dbr_obj.mv"));
        // editing a function and running the edited text changes the database
        assert_eq!(s.run("SELECT dbr_obj.soma(3, 4)", 0, 0).unwrap().rows[0][0].as_deref(), Some("7"));
        let edited = f.replace("a + b", "a * b");
        let text = crate::app::editable_source(Kind::Postgres, "function", "dbr_obj", "soma", "a integer, b integer", &edited);
        for (x, y) in crate::sqltext::split_statements(&text, ";") {
            s.run(&text[x..y], 0, 0).unwrap_or_else(|e| panic!("{}: {}", &text[x..y], e));
        }
        assert_eq!(s.run("SELECT dbr_obj.soma(3, 4)", 0, 0).unwrap().rows[0][0].as_deref(), Some("12"), "the edited function is now stored in the database");
        assert!(!s.objects("role", "").unwrap().is_empty());
        assert!(s.objects("extension", "").unwrap().iter().any(|o| o.name == "plpgsql"));
        s.run("DROP SCHEMA dbr_obj CASCADE", 0, 0).unwrap();
    }
}

#[cfg(test)]
mod format_roundtrip {
    use super::*;

    #[test]
    fn formatted_sql_returns_the_same_rows() {
        let p = std::env::temp_dir().join("dbr-test-format.sqlite");
        crate::sample::create(&p).unwrap();
        let mut c = ConnConfig::new(Kind::Sqlite);
        c.database = p.to_string_lossy().to_string();
        let mut s = Session::open(&c).unwrap();
        for q in [
            "select c.customer_id, c.first_name, count(o.order_id) as n from customers c left join orders o on o.customer_id = c.customer_id where c.country <> 'Chile' and c.customer_id between 3 and 30 group by c.customer_id order by n desc, c.customer_id limit 7",
            "select p.name, case when p.unit_price > 50 then 'caro' else 'barato' end as faixa from products p where p.category in ('Livros', 'Jogos') order by p.name",
            "select first_name || ' ' || last_name as nome from customers where email like '%1@%' union select name from products where product_id < 3 order by 1",
            "select * from (select customer_id, count(*) c from orders group by customer_id having count(*) > 3) t order by c desc, customer_id",
        ] {
            let plain = s.run(q, 0, 0).unwrap_or_else(|e| panic!("{}: {}", q, e));
            for upper in [true, false] {
                let formatted = crate::sqltext::format_sql(q, upper);
                let again = s.run(&formatted, 0, 0).unwrap_or_else(|e| panic!("formatted SQL fails: {}\n{}", e, formatted));
                assert_eq!(plain.rows, again.rows, "{}", formatted);
                assert_eq!(plain.columns, again.columns);
                // formatting twice changes nothing
                assert_eq!(crate::sqltext::format_sql(&formatted, upper), formatted, "not stable:\n{}", formatted);
            }
        }
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;

    #[test]
    fn host_and_port_in_one_field() {
        assert_eq!(split_host_port("localhost:3337", 3306), ("localhost".to_string(), 3337));
        assert_eq!(split_host_port(" db.exemplo.com ", 3306), ("db.exemplo.com".to_string(), 3306));
        assert_eq!(split_host_port("10.0.0.5:abc", 3306), ("10.0.0.5:abc".to_string(), 3306), "not a port: left alone");
        assert_eq!(split_host_port("::1", 5432), ("::1".to_string(), 5432), "IPv6 is left alone");
    }

    #[test]
    fn unreachable_mysql_gives_a_readable_error() {
        let mut c = ConnConfig::new(Kind::Mysql);
        c.host = "localhost:1".into(); // nothing listens on port 1
        let e = Session::open(&c).err().expect("must fail");
        let e = clean_err(e).to_string();
        assert!(e.contains("Conexão recusada") || e.to_lowercase().contains("refused"), "{}", e);
    }
}

#[cfg(test)]
mod mysql_tests {
    use super::*;

    /// Runs only when DBR_MYSQL_PORT is set (a local MySQL/MariaDB server).
    /// DBR_MYSQL_USER / DBR_MYSQL_PASS / DBR_MYSQL_DB select the account; DBR_MYSQL_LEGACY=1 expects the pre-4.1 password path.
    #[test]
    fn mysql_smoke() {
        let Ok(port) = std::env::var("DBR_MYSQL_PORT") else { return };
        let mut c = ConnConfig::new(Kind::Mysql);
        c.host = format!("localhost:{}", port); // host and port typed in one field
        c.user = std::env::var("DBR_MYSQL_USER").unwrap_or_else(|_| "root".into());
        c.password = crate::store::obfuscate(&std::env::var("DBR_MYSQL_PASS").unwrap_or_default());
        c.database = std::env::var("DBR_MYSQL_DB").unwrap_or_default();
        let mut s = Session::open(&c).unwrap_or_else(|e| panic!("connect: {}", clean_err(e)));
        if std::env::var("DBR_MYSQL_LEGACY").is_ok() {
            assert!(s.legacy_auth, "the server only accepts the old password format");
        }
        println!("version: {} (legacy auth: {})", s.version(), s.legacy_auth);
        if std::env::var("DBR_MYSQL_MAKE_LEGACY").is_ok() {
            // an account whose password is stored in the pre-4.1 format
            s.run("SET SESSION old_passwords = 1", 0, 0).unwrap();
            s.run("GRANT ALL ON *.* TO 'velho'@'localhost' IDENTIFIED BY 'senhavelha'", 0, 0).unwrap();
            s.run("FLUSH PRIVILEGES", 0, 0).unwrap();
            println!("accounts: {:?}", s.run("SELECT user, host, LENGTH(password) FROM mysql.user", 0, 0).unwrap().rows);
            println!("old_passwords={:?} secure_auth={:?}", s.run("SELECT @@old_passwords", 0, 0).unwrap().rows, s.run("SELECT @@secure_auth", 0, 0).unwrap().rows);
            let mut c2 = c.clone();
            c2.user = "velho".into();
            c2.password = crate::store::obfuscate("senhavelha");
            let s2 = Session::open(&c2).unwrap_or_else(|e| panic!("legacy connect: {}", clean_err(e)));
            assert!(s2.legacy_auth, "the old-password path was used");
            let mut c3 = c2.clone();
            c3.password = crate::store::obfuscate("errada");
            let e = clean_err(Session::open(&c3).err().expect("wrong password must fail")).to_string();
            println!("wrong password: {}", e);
        }
        let db = if c.database.is_empty() { "dbr_test".to_string() } else { c.database.clone() };
        if c.database.is_empty() {
            s.run(&format!("CREATE DATABASE IF NOT EXISTS {}", db), 0, 0).unwrap();
        }
        let q = |t: &str| format!("{}.{}", Kind::Mysql.quote(&db), Kind::Mysql.quote(t));
        for st in [
            format!("DROP TABLE IF EXISTS {}", q("dbr_b")),
            format!("DROP TABLE IF EXISTS {}", q("dbr_a")),
            format!("CREATE TABLE {} (id INT AUTO_INCREMENT PRIMARY KEY, nome VARCHAR(30) NOT NULL, nasc DATE, ativo TINYINT(1) DEFAULT 1, obs TEXT) ENGINE=InnoDB", q("dbr_a")),
            format!("CREATE TABLE {} (id INT PRIMARY KEY, a_id INT, FOREIGN KEY (a_id) REFERENCES {}(id)) ENGINE=InnoDB", q("dbr_b"), q("dbr_a")),
            format!("CREATE INDEX dbr_idx ON {} (nome)", q("dbr_a")),
        ] {
            s.run(&st, 0, 0).unwrap_or_else(|e| panic!("{}: {}", st, e));
        }
        s.run(&format!("INSERT INTO {} (nome, nasc) VALUES ('Ana', '2020-01-02'), ('Bia', NULL), ('João', '1999-12-31')", q("dbr_a")), 0, 0).unwrap();
        for i in 0..250 {
            s.run(&format!("INSERT INTO {} (nome) VALUES ('n{}')", q("dbr_a"), i), 0, 0).unwrap();
        }
        let r = s.run(&format!("SELECT * FROM {} ORDER BY id", q("dbr_a")), 200, 0).unwrap();
        assert_eq!(r.rows.len(), 200);
        assert!(r.has_more);
        assert_eq!(r.rows[0][1].as_deref(), Some("Ana"));
        assert_eq!(r.rows[0][2].as_deref(), Some("2020-01-02"));
        assert_eq!(r.rows[1][2], None);
        assert_eq!(r.rows[2][1].as_deref(), Some("João"), "accents survive the round trip");
        let e = s.run(&format!("SELECT * FROM {}", q("nao_existe_xyz")), 0, 0).unwrap_err().to_string();
        assert!(e.contains("nao_existe_xyz"), "{}", e);
        assert!(s.schemas().unwrap().contains(&db));
        assert!(s.tables(&db).unwrap().iter().any(|t| t.0 == "dbr_a"));
        let m = s.table_meta(&db, "dbr_b").unwrap();
        assert_eq!(m.fks.len(), 1, "{:?}", m.fks);
        assert_eq!(m.fks[0].ref_table, "dbr_a");
        let ma = s.table_meta(&db, "dbr_a").unwrap();
        assert!(ma.columns[0].pk);
        assert!(ma.indexes.iter().any(|i| i.name == "dbr_idx"));
        assert!(ma.ddl.contains("CREATE TABLE"));
        // objects (procedures, functions, triggers) when the server supports them
        for st in [
            format!("CREATE FUNCTION {}(a INT, b INT) RETURNS INT DETERMINISTIC RETURN a + b", q("dbr_soma")),
            format!("CREATE PROCEDURE {}() BEGIN SELECT COUNT(*) FROM {}; END", q("dbr_conta"), q("dbr_a")),
            format!("CREATE TRIGGER {} BEFORE INSERT ON {} FOR EACH ROW SET NEW.obs = 'novo'", q("dbr_trg"), q("dbr_a")),
        ] {
            let _ = s.run(&format!("DROP {} IF EXISTS {}", st.split_whitespace().nth(1).unwrap(), st.split('`').nth(3).map(|n| q(n)).unwrap_or_default()), 0, 0);
            s.run(&st, 0, 0).unwrap_or_else(|e| panic!("{}: {}", st, e));
        }
        assert!(s.objects("function", &db).unwrap().iter().any(|o| o.name == "dbr_soma"));
        assert!(s.objects("procedure", &db).unwrap().iter().any(|o| o.name == "dbr_conta"));
        let trg = s.objects("trigger", &db).unwrap();
        assert!(trg.iter().any(|o| o.name == "dbr_trg" && o.detail == "dbr_a"), "{:?}", trg);
        assert!(s.object_source("function", &db, "dbr_soma", "").unwrap().to_uppercase().contains("CREATE"));
        assert!(s.object_source("procedure", &db, "dbr_conta", "").unwrap().contains("dbr_conta"));
        assert!(s.object_source("trigger", &db, "dbr_trg", "dbr_a").unwrap().contains("dbr_trg"));
        assert!(s.objects("index", &db).unwrap().iter().any(|o| o.name == "dbr_idx"));
        assert!(s.object_source("index", &db, "dbr_idx", "dbr_a").unwrap().contains("CREATE INDEX"));
        // manual commit / rollback
        s.set_auto_commit(false).unwrap();
        s.run(&format!("DELETE FROM {}", q("dbr_b")), 0, 0).unwrap();
        s.run(&format!("INSERT INTO {} (id, a_id) VALUES (1, 1)", q("dbr_b")), 0, 0).unwrap();
        s.rollback().unwrap();
        assert_eq!(s.run(&format!("SELECT COUNT(*) FROM {}", q("dbr_b")), 0, 0).unwrap().rows[0][0].as_deref(), Some("0"));
        s.set_auto_commit(true).unwrap();
        for st in [format!("DROP TRIGGER {}", q("dbr_trg")), format!("DROP PROCEDURE {}", q("dbr_conta")), format!("DROP FUNCTION {}", q("dbr_soma")), format!("DROP TABLE {}", q("dbr_b")), format!("DROP TABLE {}", q("dbr_a"))] {
            let _ = s.run(&st, 0, 0);
        }
    }
}

#[cfg(test)]
mod mysql_probe {
    use super::*;

    /// Read-only connection check (never writes): DBR_PROBE_PORT, DBR_PROBE_USER, DBR_PROBE_PASS, DBR_PROBE_DB.
    #[test]
    fn mysql_readonly_probe() {
        let Ok(port) = std::env::var("DBR_PROBE_PORT") else { return };
        let mut c = ConnConfig::new(Kind::Mysql);
        c.host = format!("localhost:{}", port);
        c.user = std::env::var("DBR_PROBE_USER").unwrap_or_default();
        c.password = crate::store::obfuscate(&std::env::var("DBR_PROBE_PASS").unwrap_or_default());
        c.database = std::env::var("DBR_PROBE_DB").unwrap_or_default();
        match Session::open(&c) {
            Ok(mut s) => {
                println!("CONNECTED: {} | legacy_auth={}", s.version(), s.legacy_auth);
                let dbs = s.schemas().unwrap_or_default();
                println!("databases: {:?}", dbs);
                if !c.database.is_empty() {
                    let t = s.tables(&c.database).unwrap_or_default();
                    println!("tables in {}: {} (first: {:?})", c.database, t.len(), t.iter().take(5).collect::<Vec<_>>());
                    if let Some((t0, _)) = s.tables(&c.database).unwrap_or_default().first().cloned() {
                        let m = s.table_meta(&c.database, &t0).map(|m| (m.columns.len(), m.indexes.len(), m.fks.len(), m.ddl.lines().next().unwrap_or("").to_string()));
                        println!("meta of {}: {:?}", t0, m);
                        let q = format!("SELECT * FROM {}.{}", Kind::Mysql.quote(&c.database), Kind::Mysql.quote(&t0));
                        match s.run(&q, 3, 0) {
                            Ok(r) => println!("select: columns={:?} rows={} has_more={}", r.columns, r.rows.len(), r.has_more),
                            Err(e) => println!("select failed: {}", clean_err(e)),
                        }
                    }
                    for k in ["procedure", "function", "trigger", "event"] {
                        println!("{}: {}", k, s.objects(k, &c.database).map(|v| v.len().to_string()).unwrap_or_else(|e| format!("erro {}", e)));
                    }
                }
            }
            Err(e) => println!("FAILED: {}", clean_err(e)),
        }
    }
}
