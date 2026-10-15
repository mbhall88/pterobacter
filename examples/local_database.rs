//! Generate local archives from the existing synthetic rescue and historical-MLST fixtures.
#[path = "../tests/common/mod.rs"]
mod common;
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: cargo run --example local_database -- NEW_DIRECTORY")?,
    );
    fs::create_dir(&destination)?;
    let fixture = common::database::Fixture::new();
    fs::create_dir(destination.join("archives"))?;
    for entry in fs::read_dir(&fixture.archive_dir)? {
        let entry = entry?;
        fs::copy(
            entry.path(),
            destination.join("archives").join(entry.file_name()),
        )?;
    }
    fs::copy(&fixture.catalogue_path, destination.join("catalogue.json"))?;
    fs::copy(&fixture.query, destination.join("fixed-query.fa"))?;
    fs::write(
        destination.join("catalogue.sha256"),
        format!("{}\n", fixture.catalogue_pin),
    )?;
    println!("{}", destination.display());
    Ok(())
}
