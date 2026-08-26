use crate::color::Rgb;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameStatus {
    NotRequested,
    Unchanged,
    Renamed,
    NotFound,
    VerificationFailed,
}

impl NameStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Unchanged => "unchanged",
            Self::Renamed => "renamed",
            Self::NotFound => "not_found",
            Self::VerificationFailed => "verification_failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchRecord {
    pub compact_uuid: String,
    pub uuid: String,
    pub snapshot_username: String,
    pub current_username: Option<String>,
    pub name_status: NameStatus,
    pub internal_color: Rgb,
    pub rendered_color: Rgb,
    pub verified_at_utc: Option<String>,
    pub verification_error: Option<String>,
}
