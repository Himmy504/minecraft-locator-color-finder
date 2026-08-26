use std::{
    fs::{self, OpenOptions},
    path::Path,
};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    fsutil::{replace_file, sibling_with_suffix},
    model::MatchRecord,
};

#[derive(Serialize)]
struct OutputRow<'a> {
    uuid: &'a str,
    snapshot_username: &'a str,
    current_username: &'a str,
    name_status: &'a str,
    internal_color: String,
    rendered_color: String,
    dataset_id: &'a str,
    verified_at_utc: &'a str,
}

pub fn write_matches_csv(
    destination: &Path,
    records: &[MatchRecord],
    dataset_id: &str,
) -> Result<()> {
    if let Some(parent) = destination.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    let partial = sibling_with_suffix(destination, ".part");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&partial)
        .with_context(|| format!("failed to create temporary output {}", partial.display()))?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(file);

    for record in records {
        writer
            .serialize(OutputRow {
                uuid: &record.uuid,
                snapshot_username: &record.snapshot_username,
                current_username: record.current_username.as_deref().unwrap_or(""),
                name_status: record.name_status.as_str(),
                internal_color: record.internal_color.hex(),
                rendered_color: record.rendered_color.hex(),
                dataset_id,
                verified_at_utc: record.verified_at_utc.as_deref().unwrap_or(""),
            })
            .with_context(|| format!("failed to write output row for UUID {}", record.uuid))?;
    }

    writer.flush().context("failed to flush CSV output")?;
    let file = writer
        .into_inner()
        .map_err(|error| error.into_error())
        .context("failed to finalize CSV output")?;
    file.sync_all().context("failed to sync CSV output")?;
    replace_file(&partial, destination).with_context(|| {
        format!(
            "failed to atomically replace output {}",
            destination.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{color::Rgb, model::NameStatus};

    use super::*;

    #[test]
    fn writes_headers_and_replaces_an_existing_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("matches.csv");
        fs::write(&path, "old contents").unwrap();
        let records = vec![MatchRecord {
            compact_uuid: "03dd8f28bb344feaac00d8e925d3f3c1".to_owned(),
            uuid: "03dd8f28-bb34-4fea-ac00-d8e925d3f3c1".to_owned(),
            snapshot_username: "Filiposz321".to_owned(),
            current_username: Some("Filiposz321".to_owned()),
            name_status: NameStatus::Unchanged,
            internal_color: Rgb::from_u32(0x3AEBEA),
            rendered_color: Rgb::from_u32(0x39E6E5),
            verified_at_utc: Some("2026-08-26T00:00:00Z".to_owned()),
            verification_error: None,
        }];

        write_matches_csv(&path, &records, "fixture").unwrap();
        let output = fs::read_to_string(path).unwrap();
        assert!(output.starts_with("uuid,snapshot_username,current_username,name_status"));
        assert!(output.contains("#3AEBEA,#39E6E5,fixture"));
        assert!(!output.contains("old contents"));
    }
}
