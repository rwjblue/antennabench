pub(crate) fn normalize_wspr_reporter_id(value: &str) -> Option<String> {
    let reporter_id = value.to_ascii_uppercase();
    let valid_length = (3..=12).contains(&reporter_id.len());
    let valid_chars = reporter_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'/'));
    let has_letter = reporter_id.bytes().any(|byte| byte.is_ascii_alphabetic());

    (valid_length && valid_chars && has_letter).then_some(reporter_id)
}

#[cfg(test)]
mod tests {
    use super::normalize_wspr_reporter_id;

    #[test]
    fn accepts_wsprnet_reporter_identifiers() {
        for (raw, normalized) in [
            ("dc5al-r", "DC5AL-R"),
            ("G4WNC-1", "G4WNC-1"),
            ("EA8/DF4UE", "EA8/DF4UE"),
            ("AC0G/ND", "AC0G/ND"),
            ("G0BZB-SWL", "G0BZB-SWL"),
            ("KFS", "KFS"),
            ("WESSEX", "WESSEX"),
        ] {
            assert_eq!(normalize_wspr_reporter_id(raw).as_deref(), Some(normalized));
        }
    }

    #[test]
    fn rejects_invalid_reporter_identifiers() {
        for value in [
            "",
            " ",
            "AB",
            "123",
            "K1 ABC",
            "K1.ABC",
            "K1_ABC",
            "ABCDEFGHIJKLM",
            "K1ÄBC",
        ] {
            assert_eq!(normalize_wspr_reporter_id(value), None, "{value:?}");
        }
    }
}
