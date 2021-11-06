// Linting Rules
#![allow(clippy::unreadable_literal)]

// Uses
use poise::serenity::{model::id::UserId, utils::Colour};

// Constants
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CREATOR_ID: UserId = UserId(177584890554875904);
pub const SOURCE_LINK: &str = "https://github.com/zedseven/radium";
pub const CREATED_DATE: &str = "2021-08-30";
pub const PREFIX: &str = "-";
pub const MAIN_COLOUR: Colour = Colour(0xbf5c4e);
