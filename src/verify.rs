use std::{
    thread,
    time::{Duration, SystemTime},
};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use indicatif::ProgressBar;
use rayon::prelude::*;
use reqwest::{StatusCode, blocking::Client, header::RETRY_AFTER};
use serde::Deserialize;

use crate::{
    model::{MatchRecord, NameStatus},
    uuid::ParsedUuid,
};

pub const MOJANG_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile/lookup";

#[derive(Clone)]
pub struct Verifier {
    client: Client,
    base_url: String,
    maximum_retries: usize,
    base_backoff: Duration,
}

#[derive(Debug)]
enum VerificationResult {
    Found(String),
    NotFound,
    Failed(String),
}

#[derive(Debug, Deserialize)]
struct ProfileResponse {
    id: String,
    name: String,
}

impl Verifier {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("locator-color-finder/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to construct Mojang verification client")?;
        Ok(Self {
            client,
            base_url: MOJANG_PROFILE_URL.to_owned(),
            maximum_retries: 3,
            base_backoff: Duration::from_secs(1),
        })
    }

    #[cfg(test)]
    fn for_test(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            base_url,
            maximum_retries: 3,
            base_backoff: Duration::ZERO,
        }
    }

    pub fn verify_matches(
        &self,
        matches: &mut [MatchRecord],
        progress: Option<&ProgressBar>,
    ) -> Result<()> {
        if matches.is_empty() {
            if let Some(progress) = progress {
                progress.finish_with_message("No matching usernames to verify");
            }
            return Ok(());
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .thread_name(|index| format!("mojang-verifier-{index}"))
            .build()
            .context("failed to create four-thread verification pool")?;

        let results: Vec<VerificationResult> = pool.install(|| {
            matches
                .par_iter()
                .map(|record| {
                    let result = self.verify_one(&record.compact_uuid);
                    if let Some(progress) = progress {
                        progress.inc(1);
                    }
                    result
                })
                .collect()
        });

        for (record, result) in matches.iter_mut().zip(results) {
            record.verified_at_utc = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
            match result {
                VerificationResult::Found(current) => {
                    record.name_status = if current == record.snapshot_username {
                        NameStatus::Unchanged
                    } else {
                        NameStatus::Renamed
                    };
                    record.current_username = Some(current);
                    record.verification_error = None;
                }
                VerificationResult::NotFound => {
                    record.name_status = NameStatus::NotFound;
                    record.current_username = None;
                    record.verification_error = None;
                }
                VerificationResult::Failed(error) => {
                    record.name_status = NameStatus::VerificationFailed;
                    record.current_username = None;
                    record.verification_error = Some(error);
                }
            }
        }

        if let Some(progress) = progress {
            progress.finish_with_message("Mojang username verification finished");
        }
        Ok(())
    }

    fn verify_one(&self, compact_uuid: &str) -> VerificationResult {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), compact_uuid);
        for attempt in 0..=self.maximum_retries {
            let response = match self.client.get(&url).send() {
                Ok(response) => response,
                Err(error) => {
                    if attempt == self.maximum_retries {
                        return VerificationResult::Failed(format!(
                            "request failed after {} attempts: {error}",
                            attempt + 1
                        ));
                    }
                    thread::sleep(self.exponential_backoff(attempt));
                    continue;
                }
            };

            let status = response.status();
            if status == StatusCode::OK {
                let profile: ProfileResponse = match response.json() {
                    Ok(profile) => profile,
                    Err(error) => {
                        return VerificationResult::Failed(format!(
                            "Mojang returned invalid JSON: {error}"
                        ));
                    }
                };
                let returned_uuid = match ParsedUuid::parse(&profile.id) {
                    Ok(uuid) => uuid,
                    Err(error) => {
                        return VerificationResult::Failed(format!(
                            "Mojang returned an invalid profile UUID: {error}"
                        ));
                    }
                };
                if returned_uuid.compact() != compact_uuid {
                    return VerificationResult::Failed(format!(
                        "Mojang returned UUID {} for requested UUID {compact_uuid}",
                        returned_uuid.compact()
                    ));
                }
                return VerificationResult::Found(profile.name);
            }
            if status == StatusCode::NOT_FOUND || status == StatusCode::NO_CONTENT {
                return VerificationResult::NotFound;
            }

            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && attempt < self.maximum_retries
            {
                let delay = retry_after(response.headers())
                    .unwrap_or_else(|| self.exponential_backoff(attempt));
                thread::sleep(delay);
                continue;
            }

            return VerificationResult::Failed(format!("Mojang returned HTTP {status}"));
        }
        unreachable!("verification loop always returns")
    }

    fn exponential_backoff(&self, retry_index: usize) -> Duration {
        self.base_backoff
            .saturating_mul(1_u32.checked_shl(retry_index as u32).unwrap_or(u32::MAX))
    }
}

fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let when = httpdate::parse_http_date(value).ok()?;
    Some(
        when.duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use tiny_http::{Header, Response, Server, StatusCode as TinyStatus};

    use crate::color::Rgb;

    use super::*;

    fn match_record() -> MatchRecord {
        MatchRecord {
            compact_uuid: "03dd8f28bb344feaac00d8e925d3f3c1".to_owned(),
            uuid: "03dd8f28-bb34-4fea-ac00-d8e925d3f3c1".to_owned(),
            snapshot_username: "OldName".to_owned(),
            current_username: None,
            name_status: NameStatus::NotRequested,
            internal_color: Rgb::from_u32(0x3AEBEA),
            rendered_color: Rgb::from_u32(0x39E6E5),
            verified_at_utc: None,
            verification_error: None,
        }
    }

    #[test]
    fn retries_429_and_records_current_name() {
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = format!("http://{}", server.server_addr());
        let calls = Arc::new(AtomicUsize::new(0));
        let server_calls = Arc::clone(&calls);
        let handle = thread::spawn(move || {
            for _ in 0..2 {
                let request = server.recv().unwrap();
                let call = server_calls.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    let header = Header::from_bytes("Retry-After", "0").unwrap();
                    request
                        .respond(Response::empty(TinyStatus(429)).with_header(header))
                        .unwrap();
                } else {
                    let body = r#"{"id":"03dd8f28bb344feaac00d8e925d3f3c1","name":"Filiposz321"}"#;
                    request.respond(Response::from_string(body)).unwrap();
                }
            }
        });

        let verifier = Verifier::for_test(address);
        let mut records = vec![match_record()];
        verifier.verify_matches(&mut records, None).unwrap();
        handle.join().unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(records[0].name_status, NameStatus::Renamed);
        assert_eq!(records[0].current_username.as_deref(), Some("Filiposz321"));
        assert!(records[0].verified_at_utc.is_some());
    }

    #[test]
    fn preserves_match_when_profile_is_not_found() {
        let server = Server::http("127.0.0.1:0").unwrap();
        let address = format!("http://{}", server.server_addr());
        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            request.respond(Response::empty(TinyStatus(404))).unwrap();
        });

        let verifier = Verifier::for_test(address);
        let mut records = vec![match_record()];
        verifier.verify_matches(&mut records, None).unwrap();
        handle.join().unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name_status, NameStatus::NotFound);
    }
}
