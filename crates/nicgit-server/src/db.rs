// SPDX-License-Identifier: AGPL-3.0-or-later
use sqlx::{postgres::PgPoolOptions, PgPool};

pub async fn connect(url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().max_connections(16).connect(url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
