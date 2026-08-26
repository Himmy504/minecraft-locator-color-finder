use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedUuid {
    compact: String,
    canonical: String,
    hash: u32,
}

impl ParsedUuid {
    pub fn parse(input: &str) -> Result<Self, ParseUuidError> {
        let compact: String = input
            .chars()
            .filter(|character| *character != '-')
            .collect();
        if compact.len() != 32 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ParseUuidError(format!(
                "invalid UUID {input:?}; expected 32 hexadecimal digits with optional canonical hyphens"
            )));
        }

        if input.contains('-') {
            let bytes = input.as_bytes();
            if input.len() != 36
                || bytes.get(8) != Some(&b'-')
                || bytes.get(13) != Some(&b'-')
                || bytes.get(18) != Some(&b'-')
                || bytes.get(23) != Some(&b'-')
            {
                return Err(ParseUuidError(format!(
                    "invalid UUID {input:?}; hyphens must use the canonical 8-4-4-4-12 layout"
                )));
            }
        }

        let compact = compact.to_ascii_lowercase();
        let mut hash = 0_u32;
        for offset in [0, 8, 16, 24] {
            let segment = u32::from_str_radix(&compact[offset..offset + 8], 16)
                .map_err(|_| ParseUuidError(format!("invalid hexadecimal UUID {input:?}")))?;
            hash ^= segment;
        }

        let canonical = format!(
            "{}-{}-{}-{}-{}",
            &compact[0..8],
            &compact[8..12],
            &compact[12..16],
            &compact[16..20],
            &compact[20..32]
        );

        Ok(Self {
            compact,
            canonical,
            hash,
        })
    }

    pub fn compact(&self) -> &str {
        &self.compact
    }

    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    pub const fn hash(&self) -> u32 {
        self.hash
    }

    pub const fn internal_rgb(&self) -> u32 {
        self.hash & 0x00ff_ffff
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseUuidError(String);

impl fmt::Display for ParseUuidError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ParseUuidError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduces_java_uuid_hash_code() {
        let notch = ParsedUuid::parse("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap();
        assert_eq!(notch.hash(), 0xE9F5688E);
        assert_eq!(notch.internal_rgb(), 0xF5688E);

        let matching = ParsedUuid::parse("03dd8f28bb344feaac00d8e925d3f3c1").unwrap();
        assert_eq!(matching.hash(), 0x313AEBEA);
        assert_eq!(matching.internal_rgb(), 0x3AEBEA);
    }

    #[test]
    fn normalizes_case_and_hyphens() {
        let parsed = ParsedUuid::parse("03DD8F28-BB34-4FEA-AC00-D8E925D3F3C1").unwrap();
        assert_eq!(parsed.compact(), "03dd8f28bb344feaac00d8e925d3f3c1");
        assert_eq!(parsed.canonical(), "03dd8f28-bb34-4fea-ac00-d8e925d3f3c1");
    }

    #[test]
    fn rejects_malformed_values() {
        assert!(ParsedUuid::parse("not-a-uuid").is_err());
        assert!(ParsedUuid::parse("03dd8f28-bb344fea-ac00-d8e925d3f3c1").is_err());
        assert!(ParsedUuid::parse("03dd8f28bb344feaac00d8e925d3f3cz").is_err());
    }
}
