use crate::config::DatabaseConfig;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::path::Path;
use std::time::Duration;

/// Initialize the PostgreSQL connection pool and run migrations.
pub async fn init_pool(config: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    let url = config.url.as_deref().expect("database URL is required");

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(config.idle_timeout_secs))
        .connect(url)
        .await?;

    // Run pending migrations from the migrations/ directory
    let migrations_dir = Path::new("migrations");
    if migrations_dir.exists() {
        let migrator = sqlx::migrate::Migrator::new(migrations_dir).await
            .expect("failed to load migrations");
        migrator.run(&pool).await
            .expect("failed to run database migrations");
        tracing::info!("[db] Migrations applied successfully");
    } else {
        tracing::warn!("[db] No migrations directory found, skipping migrations");
    }

    tracing::info!(
        max_connections = config.max_connections,
        "[db] PostgreSQL pool initialized"
    );

    Ok(pool)
}

/// Health check: verify the database is reachable.
pub async fn health_check(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1")
        .fetch_one(pool)
        .await
        .is_ok()
}
