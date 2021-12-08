// Uses
use super::schema::*;

// Models
#[derive(Identifiable, Queryable, Insertable)]
#[table_name = "saved_rolls"]
pub struct SavedRoll {
	pub id: Option<i32>,
	pub guild_id: i64,
	pub user_id: i64,
	pub name: String,
	pub command: String,
}

#[derive(Identifiable, Associations, Queryable, Insertable)]
#[belongs_to(SavedRoll)]
#[table_name = "saved_roll_aliases"]
pub struct SavedRollAlias {
	pub id: Option<i32>,
	pub saved_roll_id: i32,
	pub alias: String,
}
