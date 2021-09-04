use std::{cmp::Reverse, collections::VecDeque, num::ParseIntError, str::FromStr};

use poise::{command, serenity::model::misc::Mentionable};
use rand::{thread_rng, Rng};

use crate::{
	util::{is_slash_context, reply, reply_embed, reply_plain},
	Error,
	PoiseContext,
};

const ANNOTATION_CHAR: char = '!';
const OPERATOR_SYMBOLS: [char; 10] = ['^', '*', '×', 'x', '/', '÷', '+', '-', '(', ')'];
const MAX_FIELD_VALUE: usize = 1024;

/// Roll a die.
#[command(slash_command, aliases("eval", "evaluate"))]
pub async fn roll(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "The dice to roll."]
	command: String,
) -> Result<(), Error> {
	let slash_command = is_slash_context(&ctx);

	let annotation_index = command.find(ANNOTATION_CHAR);
	let command_slice = match annotation_index {
		Some(index) => command[0..index].trim(),
		None => command.trim(),
	};

	match parse_roll_command(command_slice) {
		Some(rpn) => {
			if let Some((result, dice_rolls)) = evaluate_roll_command(rpn) {
				// Display preparation
				let mut rolls_string = String::new();
				let rolls_count = dice_rolls.len();
				if rolls_count > 0 {
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
				}

				// Annotation parsing
				let annotation;
				if let Some(index) = annotation_index {
					annotation = command[(index + 1)..].trim();
				} else {
					annotation = "";
				}

				// Display
				let dice_rolls_len = dice_rolls.len();
				let display_big_result =
					dice_rolls_len > 1 || (dice_rolls_len == 1 && dice_rolls[0].len() >= 5);
				if display_big_result {
					if rolls_string.len() > MAX_FIELD_VALUE {
						rolls_string = String::from("*…clipped because there were too many values*")
					}
					reply_embed(ctx, |e| {
						if !slash_command {
							e.field("For:", ctx.author().mention(), true);
						}
						if !annotation.is_empty() {
							e.field("Reason:", format!("`{}`", annotation), true);
						}
						e.field("Command:", format!("`{}`", command_slice), false);
						e.field("Rolls:", rolls_string, false).field(
							"Result:",
							format!("`{}`", result),
							false,
						)
					})
					.await?;
				} else {
					let mut display = String::new();
					if !slash_command {
						display.push_str(ctx.author().mention().to_string().as_str());
					}
					if !annotation.is_empty() {
						display.push_str(" `");
						display.push_str(annotation);
						display.push('`');
					}
					if slash_command {
						display.push_str(" `");
						display.push_str(command_slice);
						display.push('`');
					}
					display.push_str(": ");
					display.push_str(rolls_string.as_str());
					if !(dice_rolls_len == 1
						&& dice_rolls[0].len() == 1
						&& dice_rolls[0][0] == result)
					{
						if !rolls_string.is_empty() {
							display.push(' ');
						}
						display.push_str("Result: `");
						display.push_str(result.to_string().as_str());
						display.push('`');
					}

					reply_plain(ctx, display.trim()).await?;
				}
			} else {
				reply(ctx, "Invalid command.").await?;
				return Ok(());
			}
		}
		None => {
			reply(ctx, "Invalid command.").await?;
			return Ok(());
		}
	}

	Ok(())
}

#[derive(Debug)]
enum Evaluable {
	Num(u32),
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
		let mut rng = thread_rng();
		let mut rolls = Vec::new();
		for _ in 0..self.count {
			rolls.push(rng.gen_range(1..=self.size));
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

		let dice_count = if d_index != 0 {
			s[0..d_index].parse::<u32>().map_err(ParseDiceError::Int)?
		} else {
			1
		};

		let remaining = &s[(d_index + 1)..];
		let b_index = remaining.find('b');
		let w_index = remaining.find('w');

		let mod_index = if b_index.is_some() {
			if w_index.is_some() {
				return Err(ParseDiceError::Format);
			} else {
				b_index
			}
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
				let n = remaining[(i + 1)..]
					.parse::<u32>()
					.map_err(ParseDiceError::Int)?;
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

/// Parse the roll command into a Reverse Polish Notation expression.
fn parse_roll_command(command: &str) -> Option<Vec<Evaluable>> {
	// Split the command into tokens. Whitespace-separated values and operators are
	// individual tokens.
	let tokens = command
		.split_whitespace()
		.map(|s| {
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
		.flatten()
		.collect::<Vec<&str>>();

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
		if let Ok(value) = token.parse::<u32>() {
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
fn evaluate_roll_command(rpn: Vec<Evaluable>) -> Option<(u32, Vec<Vec<u32>>)> {
	let mut dice_rolls = Vec::new();
	let mut stack = VecDeque::new();

	for operand in rpn {
		match operand {
			Evaluable::Dice(dice) => {
				let (rolls, value) = dice.eval();
				dice_rolls.push(rolls);
				stack.push_front(value);
			}
			Evaluable::Num(value) => {
				stack.push_front(value);
			}
			Evaluable::Operator(op) => {
				if stack.len() < 2 {
					return None;
				}
				let right = stack.pop_front().unwrap();
				let left = stack.pop_front().unwrap();
				let value = match op.op {
					OperatorType::Exponent => left.pow(right),
					OperatorType::Multiply => left * right,
					OperatorType::Divide => left / right,
					OperatorType::Add => left + right,
					OperatorType::Subtract => left - right,
					_ => {
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
