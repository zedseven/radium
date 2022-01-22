// Modules
mod chance;
mod playback;
mod util;

// Uses
use poise::Command;

use self::{chance::*, playback::*, util::*};
use crate::{DataArc, Error};

/// The list of commands supported by the bot.
pub fn commands() -> Vec<Command<DataArc, Error>> {
	vec![
		register(),
		set_status(),
		help(),
		about(),
		ping(),
		join(),
		leave(),
		play(),
		skip(),
		pause(),
		resume(),
		seek(),
		clear(),
		now_playing(),
		queue(),
		roll(),
		batch_roll(),
		save_roll(),
		delete_roll(),
		saved_rolls(),
		run_roll(),
		dice_jail(),
	]
}
