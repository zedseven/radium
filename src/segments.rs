// Uses
use std::collections::HashMap;

use lavalink_rs::model::GuildId;
use lru::LruCache;

use crate::constants::VIDEO_SEGMENT_CACHE_SIZE;

// Definitions
#[derive(Debug)]
pub struct SegmentData {
	pub active_segments: HashMap<GuildId, GuildSegments>,
	// This will have a problem if we're inserting close to or more than CACHE_SIZE entries before
	// tracks can be finished, but this isn't a pressing issue by any means. A solution to that
	// would be to support mandatory values that can not be removed from the cache until we're done
	// using them.
	pub cached_segments: LruCache<String, Option<Vec<SkipSegment>>>,
}

impl SegmentData {
	#[must_use]
	pub fn new() -> Self {
		Self {
			active_segments: HashMap::new(),
			cached_segments: LruCache::new(VIDEO_SEGMENT_CACHE_SIZE),
		}
	}
}

// This is kind of a backwards implementation, but it's done this way so that we
// don't have to constantly query what's currently playing in Lavalink
#[derive(Debug, Clone)]
pub struct GuildSegments {
	pub track_identifier: String,
	pub segments: Vec<SkipSegment>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SkipSegment {
	pub start: f32,
	pub end: f32,
	// Start segments should still be cacheable, but shouldn't be considered by the mid-playback
	// skipping
	pub is_at_start: bool,
	pub is_at_end: bool,
}

impl SkipSegment {
	pub fn is_at_an_end(&self) -> bool {
		self.is_at_start || self.is_at_end
	}
}
