CREATE TABLE 'saved_rolls' (
	'guild_id' BIGINT NOT NULL,
	'user_id' BIGINT NOT NULL,
	'name' VARCHAR NOT NULL,
	'command' VARCHAR NOT NULL,
	PRIMARY KEY ('guild_id', 'user_id', 'name')
) WITHOUT ROWID;
