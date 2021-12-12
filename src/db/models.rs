// Uses
use std::borrow::Cow;

use super::schema::*;

// Models
#[derive(Identifiable, Queryable, Insertable)]
#[table_name = "saved_rolls"]
#[primary_key(guild_id, user_id, name)]
pub struct SavedRoll<'a> {
	pub guild_id: i64,
	pub user_id: i64,
	pub name: Cow<'a, str>,
	pub command: Cow<'a, str>,
}
