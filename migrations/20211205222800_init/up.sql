CREATE TABLE 'saved_rolls' (
    'id' INTEGER PRIMARY KEY,
	'guild_id' BIGINT NOT NULL,
	'user_id' BIGINT NOT NULL,
	'name' VARCHAR NOT NULL,
	'command' VARCHAR NOT NULL,
	UNIQUE ('guild_id', 'user_id', 'name')
);

CREATE TABLE 'saved_roll_aliases' (
    'id' INTEGER PRIMARY KEY,
    'saved_roll_id' INTEGER NOT NULL,
    'alias' VARCHAR NOT NULL,
    FOREIGN KEY ('saved_roll_id') REFERENCES saved_rolls('id')
);
