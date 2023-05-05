# Radium (Radio) Bot
<img src="logo.png" alt="Logo" title="Logo" align="right" width="30%">

[![License: GPLv3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE.md)

A music & dice bot made for personal use.

I made this for use by my friends and I on [Discord](https://discord.com/).
You're welcome to use it if you want to, but it isn't intended for
widespread use.

## Infrastructure
- [Poise](https://github.com/serenity-rs/poise)
- [Serenity](https://github.com/serenity-rs/serenity)
- [Songbird](https://github.com/serenity-rs/songbird)
- [Lavalink](https://github.com/freyacodes/Lavalink) (with the [lavalink-rs](https://gitlab.com/vicky5124/lavalink-rs) Rust wrapper).
	- [LavaSrc Plugin](https://github.com/TopiSenpai/LavaSrc) for additional music service support (Spotify, Apple Music)
	- [DuncteBot Lavalink Plugin](https://github.com/DuncteBot/skybot-lavalink-plugin) for text-to-speech support
- [Diesel](https://diesel.rs/)
- [SQLite](https://sqlite.org/index.html)
- [SponsorBlock](https://sponsor.ajay.app/) (with my [sponsor-block-rs](https://github.com/zedseven/sponsor-block-rs) Rust wrapper).

(Lavalink and its associated plugins need to be installed and set up separately - this bot just interfaces with them)

## SponsorBlock Integration
While playing videos from YouTube, the bot will automatically skip sponsored segments,
intros and outros, non-music sections in music videos, and more.

This uses the fantastic [SponsorBlock](https://sponsor.ajay.app/) service, which is run on curated user-submitted
timestamps.

## Dice Rolling
Parses the roll expression into [Reverse Polish Notation](https://en.wikipedia.org/wiki/Reverse_Polish_notation),
then processes dice rolls into numbers (by rolling) and calculates the result.
Because of this, it supports arbitrary mathematical expressions - even with no dice
involved.

For example, you can do crazy stuff like this:
```
-roll (3d20b2 + 11) ^ (d4 * 2) / 2d100w
```

Obviously this is beyond what a typical game would ever really require, but it was fun
to implement.

The format for dice rolls is `<count>d<size>`.
You can also do (dis)advantage with either [**b**]est or [**w**]orst after the roll,
followed by the number of best/worst rolls you want to keep.

For example:
`6d8b4` to roll 6 d8s and keep the best 4.
