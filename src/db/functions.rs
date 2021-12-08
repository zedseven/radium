// Uses
use diesel::sql_types::Integer;

// Functions
no_arg_sql_function!(
	last_insert_rowid,
	Integer,
	"Represents the SQLite `last_insert_rowid` function, which is used to get the ROWID of the \
	 last-inserted record."
);
