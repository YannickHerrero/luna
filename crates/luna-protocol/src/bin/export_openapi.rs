use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("packages/protocol/generated/openapi.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_string_pretty(&luna_protocol::openapi())? + "\n",
    )?;
    Ok(())
}
