// Modules
mod roll;

// Uses
use std::borrow::Cow;

use anyhow::Context;
use diesel::{
	delete,
	replace_into,
	ExpressionMethods,
	QueryDsl,
	RunQueryDsl,
	TextExpressionMethods,
};
use poise::{command, serenity::model::misc::Mentionable};

use self::roll::{evaluate_roll_rpn, parse_roll_command, Dice};
use crate::{
	db::{models::SavedRoll, schema::*},
	util::{escape_str, is_application_context, none_on_empty, reply, reply_embed, reply_plain},
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
	execute_roll(ctx, command_slice, annotation_slice, false).await?;

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
		let number_width = count.ilog10() as usize + 1;
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
	#[description = "The name to save the command as."] mut identifier: String,
	#[rest]
	#[description = "The roll command to save. Type it out exactly how you would if you were \
	                 using the roll command."]
	command: String,
) -> Result<(), Error> {
	// Get the associated IDs or exit
	let Some((ctx_guild_id, ctx_user_id)) = get_ctx_ids(ctx) else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	// Clean up the input
	identifier = identifier.to_lowercase();
	let command = command.trim();

	// Verify that the command is valid
	if command.contains(ANNOTATION_CHAR) {
		reply(
			ctx,
			"You cannot include annotations on saved commands. The identifier will be used as an \
			 annotation, and you can annotate when you run the saved rolls as well.",
		)
		.await?;
		return Ok(());
	}
	if command.is_empty() || parse_roll_command(command).is_err() {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	// Create the new records and insert
	{
		let conn = ctx.data().db_pool.get().unwrap();

		// Insert the roll command
		let saved_roll = SavedRoll {
			guild_id: ctx_guild_id,
			user_id: ctx_user_id,
			name: Cow::from(identifier.as_str()),
			command: Cow::from(command),
		};
		replace_into(saved_rolls::table)
			.values(&saved_roll)
			.execute(&conn)
			.with_context(|| "failed to save the roll command to the database")?;
	}

	// Finish up
	reply(ctx, format!("Saved the roll command `{}`.", identifier)).await?;

	Ok(())
}

/// Delete a saved roll command.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "deleteroll"
)]
pub async fn delete_roll(
	ctx: PoiseContext<'_>,
	#[description = "The name of the saved roll command to delete."] mut identifier: String,
) -> Result<(), Error> {
	// Get the associated IDs or exit
	let Some((ctx_guild_id, ctx_user_id)) = get_ctx_ids(ctx) else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	// Prepare the identifier
	identifier = identifier.to_lowercase();

	// Delete the row
	let deleted_rows = {
		use self::saved_rolls::dsl::*;

		let conn = ctx.data().db_pool.get().unwrap();

		delete(saved_rolls)
			.filter(guild_id.eq(ctx_guild_id))
			.filter(user_id.eq(ctx_user_id))
			.filter(name.eq(&identifier))
			.execute(&conn)
	};

	// Respond with the result
	if let Ok(count) = deleted_rows {
		if count == 1 {
			reply(ctx, format!("The saved roll `{}` was deleted.", identifier)).await?;
		} else {
			reply(
				ctx,
				format!(
					"A saved roll could not be found with the name `{}`.",
					identifier
				),
			)
			.await?;
		}
	} else {
		reply(
			ctx,
			format!("A problem was encountered with deleting `{}`.", identifier),
		)
		.await?;
	}
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
	additional: Option<String>,
) -> Result<(), Error> {
	// Get the associated IDs or exit
	let Some((ctx_guild_id, ctx_user_id)) = get_ctx_ids(ctx) else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	// Clean and prepare the identifier
	let identifier_query = format!("{}%", identifier.trim().to_lowercase());

	// Fetch the command to execute from the database
	let (mut roll_reason, mut roll_command) = {
		use self::saved_rolls::dsl::*;

		let conn = ctx.data().db_pool.get().unwrap();

		let search_result = saved_rolls
			.filter(guild_id.eq(ctx_guild_id))
			.filter(user_id.eq(ctx_user_id))
			.filter(name.like(&identifier_query))
			.select((name, command))
			.limit(1)
			.get_result::<(String, String)>(&conn);

		if search_result.is_err() {
			reply(
				ctx,
				format!(
					"A saved roll could not be found for the query `{}`.",
					identifier
				),
			)
			.await?;
			return Ok(());
		}

		search_result.unwrap()
	};

	// Parse the raw command string into clean, meaningful slices
	let annotation_index = additional.as_ref().and_then(|a| a.find(ANNOTATION_CHAR));
	let additional_command_slice = additional
		.as_ref()
		.map(|full_string| {
			annotation_index
				.map_or_else(|| full_string.trim(), |index| full_string[0..index].trim())
		})
		.and_then(none_on_empty);
	let additional_annotation_slice = additional
		.as_ref()
		.and_then(|full_string| annotation_index.map(|index| full_string[(index + 1)..].trim()))
		.and_then(none_on_empty);

	// Combine the saved roll with the additional information provided, if any
	if let Some(added_command) = additional_command_slice {
		roll_command.insert(0, '(');
		roll_command.push_str(") ");
		roll_command.push_str(added_command);
	}
	if let Some(added_annotation) = additional_annotation_slice {
		roll_reason.push_str("; ");
		roll_reason.push_str(added_annotation);
	}

	// Execute the command
	execute_roll(ctx, roll_command.as_str(), Some(roll_reason.as_str()), true).await?;

	Ok(())
}

/// Show a list of all your saved rolls.
#[command(
	prefix_command,
	slash_command,
	category = "Chance",
	rename = "savedrolls"
)]
pub async fn saved_rolls(ctx: PoiseContext<'_>) -> Result<(), Error> {
	// Get the associated IDs or exit
	let Some((ctx_guild_id, ctx_user_id)) = get_ctx_ids(ctx) else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	// Fetch the command to execute from the database
	let saved_commands = {
		use self::saved_rolls::dsl::*;

		let conn = ctx.data().db_pool.get().unwrap();

		saved_rolls
			.filter(guild_id.eq(ctx_guild_id))
			.filter(user_id.eq(ctx_user_id))
			.order_by(name)
			.select((name, command))
			.load::<(String, String)>(&conn)
			.with_context(|| "failed to retrieve a list of the saved roll commands")?
	};

	if saved_commands.is_empty() {
		reply(
			ctx,
			format!(
				"No saved rolls could be found for {}.",
				ctx.author().id.mention()
			),
		)
		.await?;
		return Ok(());
	}

	// Prepare the formatted list
	let mut output = format!("For {}:", ctx.author().id.mention());
	for (name, command) in &saved_commands {
		output.push_str(format!("\n**{}:** `{}`", name, command).as_str());
	}

	// Send the reply
	reply_embed(ctx, |e| e.title("Saved Rolls").description(output)).await?;

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
	always_show_command_in_output: bool,
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
				let mut pushed = false;
				if !slash_command {
					display.push_str(ctx.author().mention().to_string().as_str());
					pushed = true;
				}
				if let Some(annotation) = annotation_escaped {
					if pushed {
						display.push(' ');
					}
					display.push('`');
					display.push_str(annotation.as_str());
					display.push('`');
					pushed = true;
				}
				if always_show_command_in_output || slash_command {
					if pushed {
						display.push_str(" - ");
					}
					display.push('`');
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

/// Retrieves the guild ID and user ID from the message context.
fn get_ctx_ids(ctx: PoiseContext) -> Option<(i64, i64)> {
	Some((
		if let Some(guild_id) = ctx.guild_id() {
			guild_id.0 as i64
		} else {
			return None;
		},
		ctx.author().id.0 as i64,
	))
}
