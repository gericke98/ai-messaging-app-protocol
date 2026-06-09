use crate::config::DatabaseConfig;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// Migrations embedded at compile time so the binary carries its own schema
/// regardless of the working directory it is launched from.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

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

    // Run pending migrations from the compile-time embedded set.
    MIGRATOR.run(&pool).await?;
    tracing::info!("[db] Migrations applied successfully");

    tracing::info!(
        max_connections = config.max_connections,
        "[db] PostgreSQL pool initialized"
    );

    Ok(pool)
}

/// Health check: verify the database is reachable.
pub async fn health_check(pool: &PgPool) -> bool {
    sqlx::query("SELECT 1").fetch_one(pool).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_embedded_in_the_binary() {
        // The binary must carry its own schema — resolving `migrations/` from CWD
        // silently skips migrations when launched from anywhere but server/.
        assert!(MIGRATOR.iter().count() >= 7);
        assert!(MIGRATOR
            .iter()
            .any(|m| m.description.contains("compound_memory")
                || m.description.contains("compound memory")));
    }
}
