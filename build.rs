// Uses
use anyhow::Result;
use vergen::{vergen, Config};

// Pre-Build Processing
fn main() -> Result<()> {
	vergen(Config::default())
}
