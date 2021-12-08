// Modules
pub mod functions;
pub mod models;
pub mod schema;

// Uses
use anyhow::{Context, Result};
use diesel::{
	r2d2::{ConnectionManager, Pool},
	SqliteConnection,
};

// Embed database migrations
embed_migrations!("migrations");

/// Establish a connection to the database.
pub fn init(database_url: String) -> Result<Pool<ConnectionManager<SqliteConnection>>> {
	// Initialize the connection pool
	let pool = Pool::builder()
		.max_size(16)
		.build(ConnectionManager::new(database_url))
		.with_context(|| "failed to initialize the connection pool")?;

	// Run embedded migrations to set up the database if necessary
	embedded_migrations::run(&pool.get().unwrap())
		.with_context(|| "failed to run embedded migrations")?;

	// Return the initialized connection pool
	Ok(pool)
}
