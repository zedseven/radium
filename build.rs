// Uses
use anyhow::Result;
use vergen::EmitBuilder;

// Pre-Build Processing
fn main() -> Result<()> {
	EmitBuilder::builder().git_sha(true).emit()?;

	Ok(())
}
