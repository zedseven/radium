// Uses
use std::{cmp::Reverse, collections::VecDeque, num::ParseIntError, str::FromStr};

use rand::{distributions::Uniform, thread_rng, Rng};

// Constants
const OPERATOR_SYMBOLS: [char; 10] = ['^', '*', '\u{d7}', 'x', '/', '\u{f7}', '+', '-', '(', ')'];

// Types
#[derive(Debug)]
pub enum Evaluable {
	Num(f64),
	Dice(Dice),
	Operator(Operator),
}

#[derive(Debug)]
pub struct Dice {
	pub size: u32,
	pub count: u32,
	pub modifier: Option<DiceModifier>,
}

#[derive(Debug)]
pub enum DiceModifier {
	Best(u32),  // Keep the best n values
	Worst(u32), // Keep the worst n values
}

impl Dice {
	pub fn eval(&self) -> (Vec<u32>, u32) {
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

pub enum ParseDiceError {
	Int(ParseIntError),
	Format,
	Value,
}

impl FromStr for Dice {
	type Err = ParseDiceError;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let processed = s.trim().to_lowercase();

		let Some(d_index) = processed.find('d') else { return Err(ParseDiceError::Format) };

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
pub struct Operator {
	pub op: OperatorType,
	pub functional: bool,
	pub precedence: u8,
	pub associates_left: bool,
}

#[derive(Eq, PartialEq, Debug)]
pub enum OperatorType {
	Exponent,
	Multiply,
	Divide,
	Add,
	Subtract,
	ParenthesisLeft,
	ParenthesisRight,
}

// Functions

/// Parse the roll command into a [Reverse Polish Notation](https://en.wikipedia.org/wiki/Reverse_Polish_notation) expression.
///
/// This is an implementation of the [Shunting-Yard Algorithm](https://en.wikipedia.org/wiki/Shunting-yard_algorithm).
pub fn parse_roll_command(command: &str) -> Result<Vec<Evaluable>, ()> {
	/// Sub-function for converting token chars into their proper operators.
	fn token_to_operator(token: char) -> Option<Operator> {
		match token {
			'^' => Some(Operator {
				op: OperatorType::Exponent,
				functional: true,
				precedence: 4,
				associates_left: false,
			}),
			'*' | '\u{d7}' | 'x' => Some(Operator {
				op: OperatorType::Multiply,
				functional: true,
				precedence: 3,
				associates_left: true,
			}),
			'/' | '\u{f7}' => Some(Operator {
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
								return Err(());
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
		return Err(());
	}
	while let Some(op) = operator_stack.pop_front() {
		if op.op == OperatorType::ParenthesisLeft || op.op == OperatorType::ParenthesisRight {
			return Err(());
		}
		output.push(Evaluable::Operator(op));
	}

	Ok(output)
}

/// Evaluate the Reverse Polish Notation expression into final results.
pub fn evaluate_roll_rpn(rpn: &[Evaluable]) -> Option<(f64, Vec<Vec<u32>>)> {
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
