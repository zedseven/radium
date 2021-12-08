// Modules
mod roll;

// Uses
use anyhow::Context;
use diesel::{replace_into, QueryDsl, RunQueryDsl, TextExpressionMethods};
use poise::{command, serenity::model::misc::Mentionable};

use self::roll::{evaluate_roll_rpn, parse_roll_command, Dice};
use crate::{
	db::{models::SavedRoll, schema::*},
	util::{escape_str, is_application_context, reply, reply_embed, reply_plain},
	Error,
	PoiseContext,
};

// Constants
const ANNOTATION_CHAR: char = '!';
const MAX_FIELD_VALUE: usize = 1024;

// Commands

/// Roll as many dice as you want, and do whatever math you need to do with
/// their roll results.
///
/// Dice rolls are specified as `<count>d<size>`, eg. `2d8`. If the count is 1,
/// you can leave it off. (eg. `d20`)
///
/// Dice rolls also support (dis)advantage. Simply put a `b` (for best) or `w`
/// (for worst) on the end of the roll, eg. `3d10b2`. Again, if you only want
/// the best 1, you can leave it off. (eg. `2d20w` for disadvantage)
///
/// You can do whatever math you want with the dice values, or even do pure math
/// with no dice involved. (eg. `/roll (2d20b + 1d8) ^ 2 / 3`)
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	aliases("eval", "evaluate", "calc", "calculate", "r")
)]
pub async fn roll(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "The dice to roll. Follow the command with `!` to annotate what the roll is \
	                 for."]
	command: String,
) -> Result<(), Error> {
	// Parse the raw command string into clean, meaningful slices
	let annotation_index = command.find(ANNOTATION_CHAR);
	let command_slice = match annotation_index {
		Some(index) => command[0..index].trim(),
		None => command.trim(),
	};
	let annotation_slice = annotation_index.map(|index| command[(index + 1)..].trim());

	// Execute the command
	execute_roll(ctx, command_slice, annotation_slice).await?;

	Ok(())
}

/// Batch roll the same command multiple times.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "batchroll",
	aliases("br")
)]
pub async fn batch_roll(
	ctx: PoiseContext<'_>,
	#[description = "The number of times to execute the command."] count: u32,
	#[rest]
	#[description = "The dice to roll. Follow the command with `!` to annotate what the roll is \
	                 for."]
	command: String,
) -> Result<(), Error> {
	if count < 2 {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	let slash_command = is_application_context(&ctx);

	let annotation_index = command.find(ANNOTATION_CHAR);
	let command_slice = match annotation_index {
		Some(index) => command[0..index].trim(),
		None => command.trim(),
	};

	if let Ok(rpn) = parse_roll_command(command_slice) {
		// Execute the rolls
		let mut roll_results = Vec::new();
		for _ in 0..count {
			if let Some((result, _)) = evaluate_roll_rpn(&rpn) {
				roll_results.push(result);
			} else {
				reply(ctx, "Invalid command.").await?;
				return Ok(());
			}
		}

		// Annotation parsing
		let annotation = escape_str(if let Some(index) = annotation_index {
			command[(index + 1)..].trim()
		} else {
			""
		});

		// Prepare the results list
		let number_width = count.log10() as usize + 1;
		let mut result_display = String::new();
		for (i, result) in roll_results.iter().enumerate() {
			result_display.push_str(format!("{:>1$}: ", i + 1, number_width).as_str());
			result_display.push_str(
				format!("{:.2}", result)
					.trim_end_matches('0')
					.trim_end_matches('.'),
			);
			if i < count as usize - 1 {
				result_display.push('\n');
			}
		}

		// Escape the command string
		let command_slice_escaped = escape_str(command_slice);

		reply_embed(ctx, |e| {
			if !slash_command {
				e.field("For:", ctx.author().mention(), true);
			}
			e.field("Count:", format!("`{}`", count), true);
			if !annotation.is_empty() {
				e.field("Reason:", format!("`{}`", annotation), true);
			}
			e.field("Command:", format!("`{}`", command_slice_escaped), false)
				.field("Results:", format!("```{}```", result_display), false)
		})
		.await?;
	} else {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	Ok(())
}

/// Save a roll command for frequent use.
///
/// The command should be typed out exactly as you would when using the roll
/// command. (without the "-roll")
///
/// You cannot save an annotation with the roll command.
///
/// The command name is case-insensitive.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "saveroll",
	aliases("sr")
)]
pub async fn save_roll(
	ctx: PoiseContext<'_>,
	#[description = "The name to save the command as."] name: String,
	#[rest]
	#[description = "The roll command to save. Type it out exactly how you would if you were \
	                 using the roll command."]
	command: String,
) -> Result<(), Error> {
	// Get the associated guild ID or exit
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id.0 as i64
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};
	let user_id = ctx.author().id.0 as i64;

	// Clean up the input
	let command = command.trim();

	// Verify that the command is valid
	if command.contains(ANNOTATION_CHAR) {
		reply(ctx, "You cannot include annotations on saved commands.").await?;
		return Ok(());
	}
	if parse_roll_command(command).is_err() {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	// Create the new records and insert
	{
		let conn = ctx.data().db_pool.get().unwrap();

		// Insert the roll command
		let saved_roll = SavedRoll {
			guild_id,
			user_id,
			name: name.clone(),
			command: command.to_owned(),
		};
		replace_into(saved_rolls::table)
			.values(&saved_roll)
			.execute(&conn)
			.with_context(|| "failed to save the roll command to the database")?;
	}

	// Finish up
	reply(ctx, format!("Saved the roll command `{}`.", name)).await?;

	Ok(())
}

/// Run a saved roll command.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "runroll",
	aliases("rr")
)]
pub async fn run_roll(
	ctx: PoiseContext<'_>,
	#[description = "The name of the saved roll command to run."] identifier: String,
	#[rest]
	#[description = "Additional roll command modifiers and reason, if any."]
	additional: String,
) -> Result<(), Error> {
	// Clean and prepare the identifier
	let identifier_query = format!("{}%", identifier.trim().to_lowercase());

	// Fetch the command to execute from the database
	let (mut roll_reason, mut roll_command) = {
		use self::saved_rolls::dsl::*;

		let conn = ctx.data().db_pool.get().unwrap();

		let search_result = saved_rolls
			.filter(name.like(&identifier_query))
			.select((name, command))
			.limit(1)
			.get_result::<(String, String)>(&conn);

		if search_result.is_err() {
			reply(
				ctx,
				format!(
					"A saved roll by the name or alias `{}` does not exist.",
					&identifier_query
				),
			)
			.await?;
			return Ok(());
		}

		search_result.unwrap()
	};

	// Parse the raw command string into clean, meaningful slices
	let annotation_index = additional.find(ANNOTATION_CHAR);
	let additional_command_slice =
		annotation_index.map_or_else(|| additional.trim(), |index| additional[0..index].trim());
	let additional_annotation_slice =
		annotation_index.map_or("", |index| additional[(index + 1)..].trim());

	// Combine the saved roll with the additional information provided, if any
	if !additional_command_slice.is_empty() {
		roll_command.insert(0, '(');
		roll_command.push_str(") ");
		roll_command.push_str(additional_command_slice);
	}
	if !additional_annotation_slice.is_empty() {
		roll_reason.push_str("; ");
		roll_reason.push_str(additional_annotation_slice);
	}

	// Execute the command
	execute_roll(ctx, roll_command.as_str(), Some(roll_reason.as_str())).await?;

	Ok(())
}

/// Put bad dice in dice jail and get new dice.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "dicejail",
	aliases("newdice")
)]
pub async fn dice_jail(ctx: PoiseContext<'_>) -> Result<(), Error> {
	const DICE_SIZE: u32 = 20;
	const DICE_COUNT: u32 = 5;

	let (rolls, _) = Dice {
		size: DICE_SIZE,
		count: DICE_COUNT,
		modifier: None,
	}
	.eval();

	reply_embed(ctx, |e| {
		if !is_application_context(&ctx) {
			e.field("Requested By:", ctx.author().mention(), true);
		}
		e.title("New Dice")
			.description(
				"The previous dice have been\nput in dice jail for now. \u{1f3b2}\u{26d3}\u{fe0f}",
			)
			.field(
				format!("Sample Rolls ({}d{}):", DICE_COUNT, DICE_SIZE),
				display_rolls(&[rolls]),
				false,
			)
	})
	.await?;

	Ok(())
}

// Utility Functions

/// Executes a roll command and replies to the requester with the results,
/// formatted.
async fn execute_roll(
	ctx: PoiseContext<'_>,
	command: &str,
	annotation: Option<&str>,
) -> Result<(), Error> {
	let slash_command = is_application_context(&ctx);

	if let Ok(rpn) = parse_roll_command(command) {
		if let Some((result, dice_rolls)) = evaluate_roll_rpn(&rpn) {
			// Display preparation
			let mut rolls_string = display_rolls(&dice_rolls);

			// Annotation parsing
			let annotation_escaped = annotation.map(escape_str);

			// Display
			let dice_rolls_len = dice_rolls.len();
			let display_big_result =
				dice_rolls_len > 1 || (dice_rolls_len == 1 && dice_rolls[0].len() >= 5);

			// Display the result with maximum 2 decimal places of precision, but strip
			// off trailing '0's and '.'s so that normal rolls don't have decimals
			// We don't use the &[char] pattern:
			// If we did, numbers like `600.0` would become `6`
			let result_display = format!("{:.2}", result)
				.trim_end_matches('0')
				.trim_end_matches('.')
				.to_owned();

			let command_slice_escaped = escape_str(command);

			if display_big_result {
				if rolls_string.len() > MAX_FIELD_VALUE {
					rolls_string =
						"*\u{2026}clipped because there were too many values*".to_owned();
				}
				reply_embed(ctx, |e| {
					if !slash_command {
						e.field("For:", ctx.author().mention(), true);
					}
					if let Some(annotation) = annotation_escaped {
						e.field("Reason:", format!("`{}`", annotation), true);
					}
					e.field("Command:", format!("`{}`", command_slice_escaped), false)
						.field("Rolls:", rolls_string, false)
						.field("Result:", format!("`{}`", result_display), false)
				})
				.await?;
			} else {
				let mut display = String::new();
				if !slash_command {
					display.push_str(ctx.author().mention().to_string().as_str());
				}
				if let Some(annotation) = annotation_escaped {
					display.push_str(" `");
					display.push_str(annotation.as_str());
					display.push('`');
				}
				if slash_command {
					display.push_str(" `");
					display.push_str(command_slice_escaped.as_str());
					display.push('`');
				}
				display.push_str(": ");
				display.push_str(rolls_string.as_str());
				if !(dice_rolls_len == 1
					&& dice_rolls[0].len() == 1
					&& f64::from(dice_rolls[0][0]).eq(&result))
				{
					if !rolls_string.is_empty() {
						display.push(' ');
					}
					display.push_str("Result: `");
					display.push_str(result_display.as_str());
					display.push('`');
				}

				reply_plain(ctx, display.trim()).await?;
			}
		} else {
			reply(ctx, "Invalid command.").await?;
			return Ok(());
		}
	} else {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	Ok(())
}

/// Displays a set of rolls.
fn display_rolls(dice_rolls: &[Vec<u32>]) -> String {
	let mut rolls_string = String::new();

	let rolls_count = dice_rolls.len();
	if rolls_count == 0 {
		return rolls_string;
	}
	rolls_string.push('`');
	if rolls_count > 1 {
		rolls_string.push('[');
	}
	for (i, dice_roll) in dice_rolls.iter().enumerate() {
		if i > 0 {
			rolls_string.push(' ');
		}
		let roll_dice_count = dice_roll.len();
		if roll_dice_count > 1 {
			rolls_string.push('[');
		}
		for (j, value) in dice_roll.iter().enumerate() {
			if j > 0 {
				rolls_string.push(' ');
			}
			rolls_string.push_str(value.to_string().as_str());
		}
		if roll_dice_count > 1 {
			rolls_string.push(']');
		}
	}
	if rolls_count > 1 {
		rolls_string.push(']');
	}
	rolls_string.push('`');

	rolls_string
}
