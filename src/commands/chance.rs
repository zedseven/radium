// Uses
use std::{cmp::Reverse, collections::VecDeque, num::ParseIntError, str::FromStr};

use anyhow::{Context, Error as AnyhowError};
use diesel::{
	insert_or_ignore_into,
	replace_into,
	select,
	Connection,
	QueryDsl,
	RunQueryDsl,
	TextExpressionMethods,
};
use poise::{command, serenity::model::misc::Mentionable};
use rand::{distributions::Uniform, thread_rng, Rng};

use crate::{
	db::{
		functions::last_insert_rowid,
		models::{SavedRoll, SavedRollAlias},
		schema::*,
	},
	util::{escape_str, is_application_context, reply, reply_embed, reply_plain},
	Error,
	PoiseContext,
};

// Constants
const ANNOTATION_CHAR: char = '!';
const OPERATOR_SYMBOLS: [char; 10] = ['^', '*', '×', 'x', '/', '÷', '+', '-', '(', ')'];
const MAX_FIELD_VALUE: usize = 1024;

async fn execute_roll(
	ctx: PoiseContext<'_>,
	command: &str,
	annotation: Option<&str>,
) -> Result<(), Error> {
	let slash_command = is_application_context(&ctx);

	if let Some(rpn) = parse_roll_command(command) {
		if let Some((result, dice_rolls)) = evaluate_roll_command(&rpn) {
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

	if let Some(rpn) = parse_roll_command(command_slice) {
		// Execute the rolls
		let mut roll_results = Vec::new();
		for _ in 0..count {
			if let Some((result, _)) = evaluate_roll_command(&rpn) {
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

#[derive(Debug)]
enum Evaluable {
	Num(f64),
	Dice(Dice),
	Operator(Operator),
}

#[derive(Debug)]
struct Dice {
	size: u32,
	count: u32,
	modifier: Option<DiceModifier>,
}

#[derive(Debug)]
enum DiceModifier {
	Best(u32),  // Keep the best n values
	Worst(u32), // Keep the worst n values
}

impl Dice {
	fn eval(&self) -> (Vec<u32>, u32) {
		let mut rolls = Vec::new();
		let mut rng = thread_rng();
		let range = Uniform::new_inclusive(1, self.size);
		for _ in 0..self.count {
			rolls.push(rng.sample(range));
		}

		let result = match self.modifier {
			Some(DiceModifier::Best(n)) => {
				let mut temp_rolls = rolls.clone();
				temp_rolls.sort_unstable_by_key(|r| Reverse(*r));
				temp_rolls.iter().take(n as usize).sum::<u32>()
			}
			Some(DiceModifier::Worst(n)) => {
				let mut temp_rolls = rolls.clone();
				temp_rolls.sort_unstable();
				temp_rolls.iter().take(n as usize).sum::<u32>()
			}
			None => rolls.iter().sum::<u32>(),
		};

		(rolls, result)
	}
}

enum ParseDiceError {
	Int(ParseIntError),
	Format,
	Value,
}

impl FromStr for Dice {
	type Err = ParseDiceError;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let processed = s.trim().to_lowercase();

		let d_index = match processed.find('d') {
			Some(v) => v,
			None => return Err(ParseDiceError::Format),
		};

		let dice_count = if d_index == 0 {
			1
		} else {
			s[0..d_index].parse::<u32>().map_err(ParseDiceError::Int)?
		};

		let remaining = &s[(d_index + 1)..];
		let b_index = remaining.find('b');
		let w_index = remaining.find('w');

		let mod_index = if b_index.is_some() {
			if w_index.is_some() {
				return Err(ParseDiceError::Format);
			}
			b_index
		} else {
			w_index
		};
		let die_size = match mod_index {
			Some(i) => remaining[0..i]
				.parse::<u32>()
				.map_err(ParseDiceError::Int)?,
			None => remaining.parse::<u32>().map_err(ParseDiceError::Int)?,
		};
		let modifier = match mod_index {
			Some(i) => {
				let n = if i + 1 < remaining.len() {
					remaining[(i + 1)..]
						.parse::<u32>()
						.map_err(ParseDiceError::Int)?
				} else {
					1
				};
				if n > dice_count {
					return Err(ParseDiceError::Value);
				}
				if b_index.is_some() {
					Some(DiceModifier::Best(n))
				} else {
					Some(DiceModifier::Worst(n))
				}
			}
			None => None,
		};

		if dice_count < 1 {
			return Err(ParseDiceError::Value);
		}
		if die_size < 2 {
			return Err(ParseDiceError::Value);
		}

		Ok(Dice {
			size: die_size,
			count: dice_count,
			modifier,
		})
	}
}

#[derive(Debug)]
struct Operator {
	op: OperatorType,
	functional: bool,
	precedence: u8,
	associates_left: bool,
}

#[derive(Eq, PartialEq, Debug)]
enum OperatorType {
	Exponent,
	Multiply,
	Divide,
	Add,
	Subtract,
	ParenthesisLeft,
	ParenthesisRight,
}

/// Parse the roll command into a [Reverse Polish Notation](https://en.wikipedia.org/wiki/Reverse_Polish_notation) expression.
///
/// This is an implementation of the [Shunting-Yard Algorithm](https://en.wikipedia.org/wiki/Shunting-yard_algorithm).
fn parse_roll_command(command: &str) -> Option<Vec<Evaluable>> {
	/// Sub-function for converting token chars into their proper operators.
	fn token_to_operator(token: char) -> Option<Operator> {
		match token {
			'^' => Some(Operator {
				op: OperatorType::Exponent,
				functional: true,
				precedence: 4,
				associates_left: false,
			}),
			'*' | '×' | 'x' => Some(Operator {
				op: OperatorType::Multiply,
				functional: true,
				precedence: 3,
				associates_left: true,
			}),
			'/' | '÷' => Some(Operator {
				op: OperatorType::Divide,
				functional: true,
				precedence: 3,
				associates_left: true,
			}),
			'+' => Some(Operator {
				op: OperatorType::Add,
				functional: true,
				precedence: 2,
				associates_left: true,
			}),
			'-' => Some(Operator {
				op: OperatorType::Subtract,
				functional: true,
				precedence: 2,
				associates_left: true,
			}),
			'(' => Some(Operator {
				op: OperatorType::ParenthesisLeft,
				functional: false,
				precedence: 0,
				associates_left: true,
			}),
			')' => Some(Operator {
				op: OperatorType::ParenthesisRight,
				functional: false,
				precedence: 0,
				associates_left: true,
			}),
			_ => None,
		}
	}

	// Split the command into tokens. Whitespace-separated values and operators are
	// individual tokens.
	let tokens = command
		.split_whitespace()
		.flat_map(|s| {
			let mut tokens = Vec::new();
			let mut start_index = 0;
			for (i, c) in s.char_indices() {
				if OPERATOR_SYMBOLS.contains(&c) {
					if start_index != i {
						tokens.push(&s[start_index..i]);
					}
					start_index = i + c.len_utf8();
					tokens.push(&s[i..start_index]);
				}
			}
			if start_index < s.len() {
				tokens.push(&s[start_index..]);
			}
			tokens
		})
		.collect::<Vec<_>>();

	// Parse the tokens into RPN.
	let mut output = Vec::new();
	let mut operator_stack: VecDeque<Operator> = VecDeque::new();
	for token in tokens {
		// Operators, including parentheses
		if token.chars().count() == 1 {
			if let Some(op) = token_to_operator(token.chars().next().unwrap()) {
				// True operators
				if op.functional {
					while let Some(other_op) = operator_stack.front() {
						if other_op.op != OperatorType::ParenthesisLeft
							&& (other_op.precedence > op.precedence
								|| (other_op.precedence == op.precedence && op.associates_left))
						{
							output.push(Evaluable::Operator(operator_stack.pop_front().unwrap()));
						} else {
							break;
						}
					}
					operator_stack.push_front(op);
				} else {
					// Parentheses
					if op.op == OperatorType::ParenthesisLeft {
						operator_stack.push_front(op);
					} else {
						loop {
							if let Some(other_op) = operator_stack.front() {
								if other_op.op == OperatorType::ParenthesisLeft {
									break;
								}
								output
									.push(Evaluable::Operator(operator_stack.pop_front().unwrap()));
							} else {
								return None;
							}
						}
						operator_stack.pop_front(); // Discard the left parenthesis
					}
				}
				continue;
			}
		}
		// Otherwise, it's a standard token
		if let Ok(dice) = token.parse::<Dice>() {
			output.push(Evaluable::Dice(dice));
			continue;
		}
		if let Ok(value) = token.parse::<f64>() {
			output.push(Evaluable::Num(value));
			continue;
		}
		return None;
	}
	while let Some(op) = operator_stack.pop_front() {
		if op.op == OperatorType::ParenthesisLeft || op.op == OperatorType::ParenthesisRight {
			return None;
		}
		output.push(Evaluable::Operator(op));
	}

	Some(output)
}

/// Evaluate the Reverse Polish Notation expression into final results.
fn evaluate_roll_command(rpn: &[Evaluable]) -> Option<(f64, Vec<Vec<u32>>)> {
	let mut dice_rolls = Vec::new();
	let mut stack = VecDeque::new();

	for operand in rpn {
		match operand {
			Evaluable::Dice(dice) => {
				let (rolls, value) = dice.eval();
				dice_rolls.push(rolls);
				stack.push_front(f64::from(value));
			}
			Evaluable::Num(value) => {
				stack.push_front(*value);
			}
			Evaluable::Operator(op) => {
				if stack.len() < 2 {
					return None;
				}
				let right = stack.pop_front().unwrap();
				let left = stack.pop_front().unwrap();
				let value = match op.op {
					OperatorType::Exponent => left.powf(right),
					OperatorType::Multiply => left * right,
					OperatorType::Divide => left / right,
					OperatorType::Add => left + right,
					OperatorType::Subtract => left - right,
					OperatorType::ParenthesisLeft | OperatorType::ParenthesisRight => {
						return None;
					}
				};
				stack.push_front(value);
			}
		}
	}
	if stack.len() != 1 {
		return None;
	}

	Some((stack.pop_front().unwrap(), dice_rolls))
}

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

/// Save a roll command for frequent use.
///
/// The command should be typed out exactly as you would when using the roll
/// command. (without the "-roll")
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
	#[description = "The name to save the command as."] names: String,
	#[rest]
	#[description = "The roll command to save. Type it out exactly how you would if you were \
	                 using the roll command."]
	command: String,
) -> Result<(), Error> {
	const IDENTIFIER_CHAR: char = ',';

	// Get the associated guild ID or exit
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id.0 as i64
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};
	let user_id = ctx.author().id.0 as i64;

	// Clean up the input
	let mut name = String::new();
	let mut aliases = Vec::new();
	for (i, identifier) in names
		.trim()
		.to_lowercase()
		.split(IDENTIFIER_CHAR)
		.enumerate()
	{
		if i == 0 {
			name = identifier.to_owned();
		} else {
			aliases.push(identifier.to_owned());
		}
	}

	let command = command.trim();

	// Verify that the command is valid
	if command.contains(ANNOTATION_CHAR) {
		reply(ctx, "You cannot include annotations on saved commands.").await?;
		return Ok(());
	}
	if parse_roll_command(command).is_none() {
		reply(ctx, "Invalid command.").await?;
		return Ok(());
	}

	// Create the new records and insert
	let conn = ctx.data().db_pool.get().unwrap();

	conn.transaction::<_, AnyhowError, _>(|| {
		// Insert the roll command
		let saved_roll = SavedRoll {
			id: None,
			guild_id,
			user_id,
			name: name.clone(),
			command: command.to_owned(),
		};
		replace_into(saved_rolls::table)
			.values(&saved_roll)
			.execute(&conn)
			.with_context(|| "failed to save the roll command to the database")?;

		// Get the ID of the saved roll that was just inserted
		let saved_roll_id = select(last_insert_rowid)
			.get_result::<i32>(&conn)
			.with_context(|| "failed to get the last-inserted record ID from the database")?;

		// Insert the roll aliases
		for alias in aliases.drain(..) {
			let roll_alias = SavedRollAlias {
				id: None,
				saved_roll_id,
				alias,
			};
			insert_or_ignore_into(saved_roll_aliases::table)
				.values(&roll_alias)
				.execute(&conn)
				.with_context(|| "failed to save a roll command alias to the database")?;
		}

		Ok(())
	})?;

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
		use self::{saved_roll_aliases::dsl::*, saved_rolls::dsl::*};

		let conn = ctx.data().db_pool.get().unwrap();

		let search_result = saved_rolls
			.left_join(saved_roll_aliases)
			.filter(name.like(&identifier_query))
			.or_filter(alias.like(&identifier_query))
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
