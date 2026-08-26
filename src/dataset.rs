use std::{
    collections::HashSet,
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use anyhow::{Context, Result, bail};
use csv::{ReaderBuilder, StringRecord};
use indicatif::ProgressBar;

use crate::{
    color::Rgb,
    model::{MatchRecord, NameStatus},
    uuid::ParsedUuid,
};

#[derive(Debug)]
pub struct ScanResult {
    pub rows_scanned: u64,
    pub duplicate_matches: u64,
    pub matches: Vec<MatchRecord>,
}

pub fn scan_dataset(
    path: &Path,
    target_preimages: &HashSet<u32>,
    rendered_target: Rgb,
    progress: Option<&ProgressBar>,
) -> Result<ScanResult> {
    let file =
        File::open(path).with_context(|| format!("failed to open dataset {}", path.display()))?;
    let source: Box<dyn Read> = if is_zstandard(path) {
        Box::new(
            zstd::stream::read::Decoder::new(file)
                .with_context(|| format!("failed to open Zstandard stream {}", path.display()))?,
        )
    } else {
        Box::new(BufReader::new(file))
    };

    let mut csv = ReaderBuilder::new()
        .has_headers(false)
        .flexible(false)
        .from_reader(source);
    let mut record = StringRecord::new();
    let mut rows_scanned = 0_u64;
    let mut duplicate_matches = 0_u64;
    let mut seen_matches = HashSet::new();
    let mut matches = Vec::new();

    loop {
        let has_record = csv
            .read_record(&mut record)
            .with_context(|| format!("failed to parse CSV near row {}", rows_scanned + 1))?;
        if !has_record {
            break;
        }
        rows_scanned += 1;

        let line = record.position().map_or(rows_scanned, csv::Position::line);
        if record.len() != 2 {
            bail!(
                "invalid dataset row {line}: expected exactly uuid,username but found {} columns",
                record.len()
            );
        }

        let uuid_text = record.get(0).expect("record length was checked");
        let username = record.get(1).expect("record length was checked");
        let uuid = ParsedUuid::parse(uuid_text)
            .with_context(|| format!("invalid UUID on dataset row {line}"))?;
        if !valid_java_username(username) {
            bail!(
                "invalid Java username {username:?} on dataset row {line}; expected 1-16 ASCII letters, digits, or underscores"
            );
        }

        let internal = uuid.internal_rgb();
        if target_preimages.contains(&internal) {
            if seen_matches.insert(uuid.compact().to_owned()) {
                matches.push(MatchRecord {
                    compact_uuid: uuid.compact().to_owned(),
                    uuid: uuid.canonical().to_owned(),
                    snapshot_username: username.to_owned(),
                    current_username: None,
                    name_status: NameStatus::NotRequested,
                    internal_color: Rgb::from_u32(internal),
                    rendered_color: rendered_target,
                    verified_at_utc: None,
                    verification_error: None,
                });
            } else {
                duplicate_matches += 1;
            }
        }

        if rows_scanned.is_multiple_of(100_000)
            && let Some(progress) = progress
        {
            progress.set_message(format!(
                "Scanned {rows_scanned} profiles; {} matches",
                matches.len()
            ));
        }
    }

    if let Some(progress) = progress {
        progress.finish_with_message(format!(
            "Scanned {rows_scanned} profiles; {} matches",
            matches.len()
        ));
    }

    Ok(ScanResult {
        rows_scanned,
        duplicate_matches,
        matches,
    })
}

fn is_zstandard(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zst"))
}

fn valid_java_username(username: &str) -> bool {
    (1..=16).contains(&username.len())
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::tempdir;

    use crate::color::rendered_preimages;

    use super::*;

    const MATCHING_UUID: &str = "03dd8f28bb344feaac00d8e925d3f3c1";
    const NON_MATCHING_UUID: &str = "069a79f444e94726a5befca90e38aaf5";

    fn write_zstd(path: &Path, contents: &str) {
        let file = File::create(path).unwrap();
        let mut encoder = zstd::stream::write::Encoder::new(file, 1).unwrap();
        encoder.write_all(contents.as_bytes()).unwrap();
        encoder.finish().unwrap();
    }

    #[test]
    fn streams_zstd_and_deduplicates_matching_uuids() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("players.csv.zst");
        write_zstd(
            &path,
            &format!(
                "{NON_MATCHING_UUID},Notch\r\n{MATCHING_UUID},Filiposz321\r\n{MATCHING_UUID},Filiposz321\r\n"
            ),
        );

        let target = Rgb::from_u32(0x39E6E5);
        let result = scan_dataset(&path, &rendered_preimages(target), target, None).unwrap();
        assert_eq!(result.rows_scanned, 3);
        assert_eq!(result.duplicate_matches, 1);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].snapshot_username, "Filiposz321");
        assert_eq!(result.matches[0].internal_color, Rgb::from_u32(0x3AEBEA));
    }

    #[test]
    fn reports_malformed_rows() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("players.csv");
        std::fs::write(&path, "not-a-uuid,Bad\n").unwrap();

        let error =
            scan_dataset(&path, &HashSet::new(), Rgb::from_u32(0x39E6E5), None).unwrap_err();
        assert!(format!("{error:#}").contains("dataset row 1"));
    }

    #[test]
    #[ignore = "requires the complete 1.65 GB pinned dataset in the application cache"]
    fn pinned_full_snapshot_has_74_matches() {
        let path = crate::download::default_dataset_path().unwrap();
        assert!(
            crate::download::validate_file(
                &path,
                crate::download::DATASET_SIZE,
                crate::download::DATASET_SHA1,
            )
            .unwrap(),
            "download the pinned dataset first with `cargo run --release -- download`"
        );

        let target = Rgb::from_u32(0x39E6E5);
        let result = scan_dataset(&path, &rendered_preimages(target), target, None).unwrap();
        assert_eq!(result.rows_scanned, 65_936_987);
        assert_eq!(result.matches.len(), 74);
        assert!(
            result
                .matches
                .iter()
                .all(|record| record.rendered_color == target)
        );
    }
}
