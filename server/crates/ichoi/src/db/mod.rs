//! Database infrastructure: the SQLite connection pool, migrations, and boot transforms.
//!
//! Migrations are pure schema DDL, run at startup. Idempotent data backfills are
//! *transforms* (§12) — run separately every boot, best-effort and non-fatal.

pub mod models;
pub mod schema;
pub mod store;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager, CustomizeConnection};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub type SqlitePool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConn = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations");

/// Applies the PRAGMAs every Ichoi connection needs.
#[derive(Debug)]
struct SqliteCustomizer;

impl CustomizeConnection<SqliteConnection, r2d2::Error> for SqliteCustomizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), r2d2::Error> {
        // Per-connection PRAGMAs only. `journal_mode = WAL` is a *persistent database*
        // property set once at pool build (see below); setting it per connection takes a
        // write lock and contends with concurrent writers ("database is locked").
        conn.batch_execute("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
            .map_err(r2d2::Error::QueryError)
    }
}

/// Build the runtime connection pool for a database file path (or `:memory:`).
pub fn establish_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .connection_customizer(Box::new(SqliteCustomizer))
        .build(manager)?;
    // Set WAL once for the database file (persistent). Ignored for `:memory:`.
    if let ::std::result::Result::Ok(mut conn) = pool.get() {
        let _ = conn.batch_execute("PRAGMA journal_mode = WAL;");
    }
    Ok(pool)
}

/// Run all pending schema migrations. Called at `serve` startup and by tests.
pub fn run_migrations(conn: &mut SqliteConnection) -> anyhow::Result<()> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("running migrations: {e}"))?;
    Ok(())
}

/// Idempotent, best-effort data backfills run every boot. Kept separate from migrations
/// (§12). Currently ensures the core node row exists.
pub fn run_transforms(conn: &mut SqliteConnection, hostname: &str) -> anyhow::Result<()> {
    store::ensure_core_node(conn, hostname)?;
    if store::get_setting(conn, "server_output_enabled")?.is_none() {
        store::set_setting(conn, "server_output_enabled", "false")?;
    }
    Ok(())
}

/// A pool whose single connection holds an open test transaction that is rolled back when
/// the pool drops — the DataUtils pattern (§12). Uses a private in-memory database.
pub fn test_pool() -> SqlitePool {
    // A private, connection-scoped in-memory database (max_size = 1 keeps it to the one
    // connection that holds the transaction).
    let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
    let pool = r2d2::Pool::builder()
        .max_size(1)
        .connection_customizer(Box::new(SqliteCustomizer))
        .build(manager)
        .expect("build test pool");
    {
        let mut conn = pool.get().expect("get test conn");
        run_migrations(&mut conn).expect("test migrations");
        conn.begin_test_transaction().expect("begin test txn");
    }
    pool
}
