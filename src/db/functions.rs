// Uses
use diesel::expression::functions::define_sql_function;

// Functions
define_sql_function! {
	/// Represents the SQLite `last_insert_rowid` function, which is used to get the `ROWID` of the last-inserted record.
	fn last_insert_rowid() -> Integer;
}
