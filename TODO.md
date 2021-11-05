# To-Do
- Clean up logo
  - Looks alright, but I really just threw it together, and it could be better
  - Maybe try flat shapes rather than line-work, as shapes are more readable at small resolutions (such as avatars)
- Now playing update for new tracks
  - I.e. when a new track starts playing, update the previous notification or post a new one with the track name and
  duration
- Shuffle (potentially with range specified)
- `ping` latency
  - Potentially requires a `ShardManager`, which is potentially a lot of additional work for a latency measurement
- Age-restricted videos
- Persistent queue across leaves and joins within guild, inc. different voice channels
- Handle moves between voice channels
- Make `skip` display next playing
  - This would be done automatically if now playing is shown for new tracks
- Queue track lengths and total playback time
  - Not sure if this is worth doing - doing it in a way that looks decent involves making queue entries use a monospace
  font, and the only way to do that is by putting the queue in code blocks. This makes it look uglier, and removes the
  links
- Consider merging now playing and queue
- Move ready into closure in framework (`_ready`)
- Escaping doesn't work fully
  - Seems to be a Discord issue, not really sure of a solution other than just replacing all special characters with
  Unicode lookalikes (like I already do with square brackets)
- Checkup command for stats etc.
- Priority play (pp) to play something right now, ahead of the rest of the queue
  - Could play the new track as soon as the command is issued, but that would mean either skipping the current track or
  skipping and re-adding it - either way, much more complicated and not intuitive 
  - Could insert right after current track, which would allow the user to decide what to do with whatever is currently
  playing
- Support timestamped YouTube videos
- SponsorBlock integration
