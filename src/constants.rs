// Linting Rules
#![allow(clippy::unreadable_literal)]

// Uses
use poise::serenity::{model::id::UserId, utils::Colour};
use sponsor_block::AcceptedCategories;

// Constants
pub const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CREATOR_ID: UserId = UserId(177584890554875904);
pub const SOURCE_LINK: &str = "https://github.com/zedseven/radium";
pub const CREATED_DATE: &str = "2021-08-30";
pub const PREFIX: &str = "-";
pub const MAIN_COLOUR: Colour = Colour(0xbf5c4e);
pub const ACCEPTED_CATEGORIES: AcceptedCategories = AcceptedCategories::all(); // We filter on use

// Operational Constants
pub const VIDEO_SEGMENT_CACHE_SIZE: usize = 2048;

// Utility Constants
pub const MILLIS_PER_SECOND: u64 = 1000;
pub const SECONDS_PER_MINUTE: u64 = 60;
pub const MINUTES_PER_HOUR: u64 = 60;
pub const MILLIS_PER_MINUTE: u64 = MILLIS_PER_SECOND * SECONDS_PER_MINUTE;
pub const MILLIS_PER_HOUR: u64 = MILLIS_PER_MINUTE * MINUTES_PER_HOUR;
pub const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR;
pub const MILLIS_PER_SECOND_F32: f32 = MILLIS_PER_SECOND as f32;
pub const SECONDS_PER_MINUTE_F32: f32 = SECONDS_PER_MINUTE as f32;
pub const MINUTES_PER_HOUR_F32: f32 = MINUTES_PER_HOUR as f32;
pub const MILLIS_PER_MINUTE_F32: f32 = MILLIS_PER_MINUTE as f32;
pub const MILLIS_PER_HOUR_F32: f32 = MILLIS_PER_HOUR as f32;
pub const SECONDS_PER_HOUR_F32: f32 = SECONDS_PER_HOUR as f32;
