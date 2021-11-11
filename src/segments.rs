// Uses
use std::collections::HashMap;

use poise::serenity::model::id::GuildId;

// Constants
pub const TRACK_IDENTIFIER_LENGTH: usize = 16;

// Definitions
#[derive(Debug, Default)]
pub struct SegmentData {
	pub active_segments: HashMap<GuildId, GuildSegments>,
	// TODO: Build a proper cache type for this. Support mandatory values and a max entry count.
	pub cached_segments: HashMap<String, Vec<SkipSegment>>,
}

impl SegmentData {
	#[must_use]
	pub fn new() -> Self {
		Self {
			active_segments: HashMap::new(),
			cached_segments: HashMap::new(),
		}
	}
}

// This is kind of a backwards implementation, but it's done this way so that we
// don't have to constantly query what's currently playing in Lavalink
#[derive(Debug, Clone)]
pub struct GuildSegments {
	pub track_name: String,
	pub segments: Vec<SkipSegment>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SkipSegment {
	pub start: f32,
	pub end: f32,
	// Start and end segments should still be cacheable, but shouldn't be considered by the
	// mid-playback skipping
	pub is_at_an_end: bool,
}
