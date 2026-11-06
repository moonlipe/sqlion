//! Creates a small sample SQLite database (customers, products, orders) like DBeaver's sample database.
use rusqlite::Connection;
use std::path::Path;

pub fn create(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(path).ok();
    }
    let c = Connection::open(path)?;
    c.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE customers (
            customer_id INTEGER PRIMARY KEY,
            first_name  TEXT NOT NULL,
            last_name   TEXT NOT NULL,
            email       TEXT UNIQUE,
            country     TEXT,
            created_at  DATE
         );
         CREATE TABLE products (
            product_id  INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            category    TEXT,
            unit_price  REAL NOT NULL,
            in_stock    INTEGER DEFAULT 0
         );
         CREATE TABLE orders (
            order_id    INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL REFERENCES customers(customer_id),
            order_date  DATE NOT NULL,
            status      TEXT DEFAULT 'new'
         );
         CREATE TABLE order_items (
            order_id    INTEGER NOT NULL REFERENCES orders(order_id),
            product_id  INTEGER NOT NULL REFERENCES products(product_id),
            quantity    INTEGER NOT NULL,
            PRIMARY KEY (order_id, product_id)
         );
         CREATE INDEX idx_orders_customer ON orders(customer_id);
         CREATE TRIGGER trg_order_items_stock AFTER INSERT ON order_items
         BEGIN
            UPDATE products SET in_stock = in_stock - NEW.quantity WHERE product_id = NEW.product_id;
         END;
         CREATE VIEW customer_totals AS
            SELECT c.customer_id, c.first_name || ' ' || c.last_name AS customer,
                   COUNT(DISTINCT o.order_id) AS orders, ROUND(SUM(p.unit_price * i.quantity), 2) AS total
            FROM customers c
            JOIN orders o ON o.customer_id = c.customer_id
            JOIN order_items i ON i.order_id = o.order_id
            JOIN products p ON p.product_id = i.product_id
            GROUP BY c.customer_id;",
    )?;
    let firsts = ["Ana", "Bruno", "Carla", "Diego", "Elisa", "Felipe", "Gabi", "Hugo", "Irene", "João", "Karen", "Lucas", "Marta", "Nuno", "Olga", "Pedro"];
    let lasts = ["Silva", "Souza", "Costa", "Lima", "Alves", "Rocha", "Pinto", "Dias", "Melo", "Reis"];
    let countries = ["Brasil", "Portugal", "Chile", "Argentina", "México", "Espanha"];
    let cats = ["Livros", "Jogos", "Casa", "Música", "Papelaria"];
    let mut seed: u64 = 42;
    let mut rnd = |n: i64| -> i64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) % n as u64) as i64
    };
    let tx = c.unchecked_transaction()?;
    for i in 1..=40i64 {
        let f = firsts[rnd(16) as usize];
        let l = lasts[rnd(10) as usize];
        tx.execute(
            "INSERT INTO customers VALUES (?1,?2,?3,?4,?5,?6)",
            rusqlite::params![i, f, l, format!("{}.{}{}@exemplo.com", f.to_lowercase(), l.to_lowercase(), i), countries[rnd(6) as usize], format!("20{:02}-{:02}-{:02}", 18 + rnd(7), 1 + rnd(12), 1 + rnd(28))],
        )?;
    }
    for i in 1..=24i64 {
        tx.execute(
            "INSERT INTO products VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![i, format!("Produto {:02}", i), cats[rnd(5) as usize], (500 + rnd(9500)) as f64 / 100.0, rnd(200)],
        )?;
    }
    for o in 1..=150i64 {
        tx.execute(
            "INSERT INTO orders VALUES (?1,?2,?3,?4)",
            rusqlite::params![o, 1 + rnd(40), format!("2025-{:02}-{:02}", 1 + rnd(12), 1 + rnd(28)), ["new", "paid", "shipped", "done"][rnd(4) as usize]],
        )?;
        let n = 1 + rnd(4);
        let mut used = Vec::new();
        for _ in 0..n {
            let p = 1 + rnd(24);
            if used.contains(&p) {
                continue;
            }
            used.push(p);
            tx.execute("INSERT INTO order_items VALUES (?1,?2,?3)", rusqlite::params![o, p, 1 + rnd(5)])?;
        }
    }
    tx.commit()?;
    Ok(())
}
