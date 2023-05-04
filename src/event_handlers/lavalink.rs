// Uses
use std::{
	sync::{Arc, Mutex},
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use lavalink_rs::{
	gateway::LavalinkEventHandler,
	model::{GuildId, PlayerDestroyed, PlayerUpdate, TrackStart, TrackStuck, WebSocketClosed},
	LavalinkClient,
};
use serenity::async_trait;
use tokio::time::{sleep, Instant};

use crate::{constants::MILLIS_PER_SECOND_F32, segments::TrackSegments, DataArc};

// The event handler for all Lavalink events
pub struct LavalinkHandler {
	pub data: Arc<Mutex<Option<DataArc>>>,
}

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
	/// During track playback, check regularly if we're close to a segment to
	/// skip.
	async fn player_update(&self, client: LavalinkClient, event: PlayerUpdate) {
		/// The amount of time before an update event should be considered
		/// invalid.
		const UPDATE_INVALIDATION_MINIMUM: i64 = 200;
		/// The number of seconds between updates.
		const UPDATE_DELAY_PERIOD: f32 = 5.0;
		/// The amount of delay that seek operations have before completing.
		const SEEK_DELAY: f32 = 0.085;
		/// A bit of extra 'fuzz' to prevent re-seeking to the same segment.
		const SEGMENT_END_EPSILON: f32 = 0.1;

		// Since this update happens within a synchronous context, check to see if the
		// received event is no longer valid. (the start of this function has been
		// delayed, and another update likely already skipped)
		let current_system_millis = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.expect("time went backwards")
			.as_millis() as i64;
		if current_system_millis - event.state.time >= UPDATE_INVALIDATION_MINIMUM {
			println!(
				"Current system time ({}) is greater than the event time ({}) by {} millis. \
				 Exiting.",
				current_system_millis,
				event.state.time,
				current_system_millis - event.state.time
			);
			return;
		}

		let event_start_time = Instant::now();

		let guild_segments_opt = {
			let data_handle = self.data.lock().unwrap();
			let segment_data_handle = data_handle.as_ref().unwrap().segment_data.lock().unwrap();
			segment_data_handle
				.active_segments
				.get(&event.guild_id)
				.cloned()
		};
		let mut change_guild_track = None;
		'seek_block: {
			if let Some(guild_segments) = guild_segments_opt {
				let position_f32 = event.state.position as f32 / MILLIS_PER_SECOND_F32;
				let mut next_segment_opt = None;
				for segment in &guild_segments.segments {
					// Segments at the start are handled by Lavalink itself - don't touch them.
					// We also skip segments that have already passed.
					if segment.is_at_start || segment.end - SEGMENT_END_EPSILON <= position_f32 {
						continue;
					}
					next_segment_opt = Some(segment);
					break;
				}

				// Process the upcoming segment if it's present
				if let Some(next_segment) = next_segment_opt {
					let mut time_until_segment = next_segment.start - position_f32;
					if time_until_segment < UPDATE_DELAY_PERIOD {
						// Verify the segment we're looking at is for the current track
						// We check this here and not sooner because it requires fetching the
						// current node for Lavalink, so we don't want to do that every update
						let current_track_identifier = {
							client
								.nodes()
								.await
								.get(&event.guild_id.0)
								.expect(
									"there should always be a node for the guild that received a \
									 `player_update` event",
								)
								.now_playing
								.as_ref()
								.expect(
									"there should always be a `now_playing` track within the \
									 `player_update` event",
								)
								.track
								.info
								.as_ref()
								.expect("playing track is missing all info")
								.identifier
								.clone()
						};
						if !current_track_identifier.eq(guild_segments.track_identifier.as_str()) {
							change_guild_track = Some(Some(current_track_identifier));
							break 'seek_block;
						}

						// Update time_until_segment since time may have elapsed
						time_until_segment -= (Instant::now() - event_start_time).as_secs_f32();

						// Wait until it's time to seek if necessary
						// (should always be < UPDATE_RESOLUTION)
						if time_until_segment > SEEK_DELAY {
							sleep(Duration::from_secs_f32(time_until_segment - SEEK_DELAY)).await;
						}

						// Seek
						// We discard the potential error because there's nothing to be done about
						// it here
						client
							.seek(event.guild_id, Duration::from_secs_f32(next_segment.end))
							.await
							.ok();
					}
				}
			}
		}

		// Update the active track for the guild if necessary
		// The nested Option kinda sucks, but it represents the need to make a change
		// followed by the need to set it to a specific value or just to unset it
		if let Some(change_active_track) = change_guild_track {
			update_segment_data(&self.data, event.guild_id, change_active_track);
		}
	}

	// Update the active segments info for new tracks
	async fn track_start(&self, client: LavalinkClient, event: TrackStart) {
		let identifier = client
			.decode_track(event.track)
			.await
			.expect("unable to decode event track string")
			.identifier;
		update_segment_data(&self.data, event.guild_id, Some(identifier));
	}

	// Automatically skip if a track is stuck
	async fn track_stuck(&self, client: LavalinkClient, event: TrackStuck) {
		println!("A currently-playing track is stuck. Skipping.");
		#[allow(clippy::dbg_macro)]
		{
			dbg!(&event);
		}
		client.skip(event.guild_id).await;
	}

	async fn websocket_closed(&self, _client: LavalinkClient, event: WebSocketClosed) {
		#[allow(clippy::dbg_macro)]
		{
			dbg!(&event);
		}
	}

	async fn player_destroyed(&self, _client: LavalinkClient, event: PlayerDestroyed) {
		#[allow(clippy::dbg_macro)]
		{
			dbg!(&event);
		}
	}
}

/// Updates the active track for a guild.
///
/// If `new_track` is [`None`], the active track is unset.
///
/// If no cached segments can be found for the value of `new_track`, the active
/// track is also unset.
fn update_segment_data(
	data: &Arc<Mutex<Option<DataArc>>>,
	guild_id: GuildId,
	new_track: Option<String>,
) {
	// Acquire a lock for the segment data
	let data_handle = data.lock().unwrap();
	let mut segment_data_handle = data_handle.as_ref().unwrap().segment_data.lock().unwrap();

	// Make the change
	let mut successfully_set_new_track = false;
	if let Some(new_track_name) = new_track {
		// Get the cached segment data if it exists (if it doesn't, the active_segments
		// entry will be removed)
		if let Some(Some(new_segments)) = segment_data_handle
			.cached_segments
			.get(&new_track_name)
			.cloned()
		{
			segment_data_handle.active_segments.insert(
				guild_id,
				TrackSegments {
					track_identifier: new_track_name,
					segments:         new_segments,
				},
			);
			successfully_set_new_track = true;
		}
	}
	// Either no cached segments exist for the new track name, or we were asked to
	// unset it
	if !successfully_set_new_track {
		segment_data_handle.active_segments.remove(&guild_id);
	}
}
