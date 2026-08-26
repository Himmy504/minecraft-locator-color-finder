use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use indicatif::ProgressBar;
use reqwest::{
    StatusCode,
    blocking::{Client, Response},
    header::{CONTENT_RANGE, RANGE},
};
use sha1::{Digest, Sha1};

use crate::fsutil::{replace_file, sibling_with_suffix};

pub const DATASET_ID: &str = "minecraft-uuids-2025-09-01";
pub const DATASET_FILE_NAME: &str = "players.csv.zst";
pub const DATASET_URL: &str =
    "https://archive.org/download/minecraft-uuids-2025-09-01/players.csv.zst";
pub const DATASET_SIZE: u64 = 1_649_493_087;
pub const DATASET_SHA1: &str = "26b25dbe6728bf6398e8ba4d5eaaed335ab0cea4";

pub fn default_dataset_path() -> Result<PathBuf> {
    let directories = ProjectDirs::from("com", "locator-color-finder", "Locator Color Finder")
        .context("could not determine the operating-system cache directory")?;
    Ok(directories.cache_dir().join(DATASET_FILE_NAME))
}

pub fn http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60 * 60 * 6))
        .user_agent(concat!("locator-color-finder/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to construct HTTP client")
}

pub fn ensure_default_dataset(force: bool, progress: Option<&ProgressBar>) -> Result<PathBuf> {
    let destination = default_dataset_path()?;
    let partial = sibling_with_suffix(&destination, ".part");
    let client = http_client()?;

    if force && partial.exists() {
        fs::remove_file(&partial).with_context(|| {
            format!(
                "failed to remove partial dataset before forced download {}",
                partial.display()
            )
        })?;
    }

    if destination.exists() && !force {
        if let Some(progress) = progress {
            progress.set_message("Verifying cached dataset");
        }
        if validate_file(&destination, DATASET_SIZE, DATASET_SHA1)? {
            if let Some(progress) = progress {
                progress.finish_with_message("Cached dataset verified");
            }
            return Ok(destination);
        }
    }

    if destination.exists() {
        fs::remove_file(&destination).with_context(|| {
            format!(
                "failed to remove invalid cached dataset {}",
                destination.display()
            )
        })?;
    }

    download_dataset(
        &client,
        DATASET_URL,
        &destination,
        DATASET_SIZE,
        DATASET_SHA1,
        progress,
    )?;
    Ok(destination)
}

pub fn download_dataset(
    client: &Client,
    url: &str,
    destination: &Path,
    expected_size: u64,
    expected_sha1: &str,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache directory {}", parent.display()))?;
    }

    let partial = sibling_with_suffix(destination, ".part");
    let mut downloaded = partial.metadata().map_or(0, |metadata| metadata.len());
    if downloaded > expected_size {
        fs::remove_file(&partial).with_context(|| {
            format!(
                "failed to remove oversized partial download {}",
                partial.display()
            )
        })?;
        downloaded = 0;
    }

    if downloaded < expected_size {
        if let Some(progress) = progress {
            progress.set_length(expected_size);
            progress.set_position(downloaded);
            progress.set_message(if downloaded == 0 {
                "Downloading profile dataset".to_owned()
            } else {
                format!("Resuming profile dataset at {downloaded} bytes")
            });
        }

        let mut request = client.get(url);
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }
        let response = request
            .send()
            .with_context(|| format!("failed to download dataset from {url}"))?;

        let (mut response, append) = prepare_response(response, downloaded)?;
        if !append {
            downloaded = 0;
            if let Some(progress) = progress {
                progress.set_position(0);
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&partial)
            .with_context(|| format!("failed to open partial download {}", partial.display()))?;

        let mut buffer = vec![0_u8; 256 * 1024];
        loop {
            let count = response
                .read(&mut buffer)
                .context("failed while reading dataset response")?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])
                .context("failed while writing partial dataset")?;
            downloaded += count as u64;
            if downloaded > expected_size {
                bail!(
                    "dataset download exceeded expected size of {expected_size} bytes; partial file retained at {}",
                    partial.display()
                );
            }
            if let Some(progress) = progress {
                progress.set_position(downloaded);
            }
        }
        file.sync_all().context("failed to sync partial dataset")?;
    }

    let actual_size = partial
        .metadata()
        .with_context(|| format!("failed to inspect partial dataset {}", partial.display()))?
        .len();
    if actual_size != expected_size {
        bail!(
            "dataset download is incomplete: expected {expected_size} bytes, received {actual_size}; rerun to resume"
        );
    }

    if let Some(progress) = progress {
        progress.set_message("Verifying downloaded dataset checksum");
    }
    if !validate_file(&partial, expected_size, expected_sha1)? {
        fs::remove_file(&partial)
            .with_context(|| format!("failed to remove corrupt download {}", partial.display()))?;
        bail!(
            "downloaded dataset failed its size or SHA-1 integrity check; the corrupt partial file was removed"
        );
    }

    replace_file(&partial, destination).with_context(|| {
        format!(
            "failed to move verified dataset into place at {}",
            destination.display()
        )
    })?;
    if let Some(progress) = progress {
        progress.finish_with_message("Dataset downloaded and verified");
    }
    Ok(())
}

fn prepare_response(response: Response, resume_offset: u64) -> Result<(Response, bool)> {
    if resume_offset == 0 {
        if !response.status().is_success() {
            bail!("dataset server returned HTTP {}", response.status());
        }
        return Ok((response, false));
    }

    if response.status() == StatusCode::PARTIAL_CONTENT {
        let expected_prefix = format!("bytes {resume_offset}-");
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !content_range.starts_with(&expected_prefix) {
            bail!(
                "dataset server returned an unexpected Content-Range {content_range:?}; expected it to begin with {expected_prefix:?}"
            );
        }
        Ok((response, true))
    } else if response.status() == StatusCode::OK {
        // The server ignored Range; safely restart the partial file from byte zero.
        Ok((response, false))
    } else {
        bail!(
            "dataset server returned HTTP {} while resuming at byte {resume_offset}",
            response.status()
        )
    }
}

pub fn validate_file(path: &Path, expected_size: u64, expected_sha1: &str) -> Result<bool> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to inspect dataset {}", path.display()))?;
    if metadata.len() != expected_size {
        return Ok(false);
    }

    let file = File::open(path)
        .with_context(|| format!("failed to open dataset {} for hashing", path.display()))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = Sha1::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .with_context(|| format!("failed while hashing dataset {}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()).eq_ignore_ascii_case(expected_sha1))
}

#[cfg(test)]
mod tests {
    use std::thread;

    use tempfile::tempdir;
    use tiny_http::{Header, Response, Server, StatusCode as TinyStatus};

    use super::*;

    fn sha1_hex(contents: &[u8]) -> String {
        let mut hasher = Sha1::new();
        hasher.update(contents);
        format!("{:x}", hasher.finalize())
    }

    fn test_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap()
    }

    #[test]
    fn resumes_a_partial_download_with_range() {
        let payload = b"abcdefghij";
        let directory = tempdir().unwrap();
        let destination = directory.path().join("dataset.zst");
        let partial = sibling_with_suffix(&destination, ".part");
        fs::write(&partial, &payload[..4]).unwrap();

        let server = Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/dataset", server.server_addr());
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            let range = request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Range"))
                .map(|header| header.value.as_str());
            assert_eq!(range, Some("bytes=4-"));
            let content_range = Header::from_bytes("Content-Range", "bytes 4-9/10").unwrap();
            request
                .respond(
                    Response::from_data(b"efghij".to_vec())
                        .with_status_code(TinyStatus(206))
                        .with_header(content_range),
                )
                .unwrap();
        });

        download_dataset(
            &test_client(),
            &url,
            &destination,
            payload.len() as u64,
            &sha1_hex(payload),
            None,
        )
        .unwrap();
        handle.join().unwrap();

        assert_eq!(fs::read(destination).unwrap(), payload);
        assert!(!partial.exists());
    }

    #[test]
    fn restarts_when_a_server_ignores_range() {
        let payload = b"replacement";
        let directory = tempdir().unwrap();
        let destination = directory.path().join("dataset.zst");
        fs::write(sibling_with_suffix(&destination, ".part"), b"bad").unwrap();

        let server = Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/dataset", server.server_addr());
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            request
                .respond(Response::from_data(payload.to_vec()))
                .unwrap();
        });

        download_dataset(
            &test_client(),
            &url,
            &destination,
            payload.len() as u64,
            &sha1_hex(payload),
            None,
        )
        .unwrap();
        handle.join().unwrap();
        assert_eq!(fs::read(destination).unwrap(), payload);
    }

    #[test]
    fn removes_a_download_that_fails_checksum_validation() {
        let payload = b"bad payload";
        let directory = tempdir().unwrap();
        let destination = directory.path().join("dataset.zst");
        let partial = sibling_with_suffix(&destination, ".part");

        let server = Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/dataset", server.server_addr());
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            request
                .respond(Response::from_data(payload.to_vec()))
                .unwrap();
        });

        let error = download_dataset(
            &test_client(),
            &url,
            &destination,
            payload.len() as u64,
            &sha1_hex(b"different!!"),
            None,
        )
        .unwrap_err();
        handle.join().unwrap();

        assert!(error.to_string().contains("integrity check"));
        assert!(!destination.exists());
        assert!(!partial.exists());
    }
}
