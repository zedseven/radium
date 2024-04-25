// Uses
use diesel::expression::functions::sql_function;

// Functions
sql_function! {
	/// Represents the SQLite `last_insert_rowid` function, which is used to get the ROWID of the last-inserted record.
	fn last_insert_rowid() -> Integer;
}
