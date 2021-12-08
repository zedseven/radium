// Uses
use super::schema::*;

// Models
#[derive(Identifiable, Queryable, Insertable)]
#[table_name = "saved_rolls"]
#[primary_key(guild_id, user_id, name)]
pub struct SavedRoll {
	pub guild_id: i64,
	pub user_id: i64,
	pub name: String,
	pub command: String,
}
