//! Custom clap `value_parser` implementations used across multiple
//! subcommands. Keeping them here lets per-command modules stay focused on
//! their dispatch logic.

/// Validates a `-H/--header` value and normalizes it to a single
/// `"Name: Value"` string. Used by `bird raw` (GET / POST / PUT / DELETE).
///
/// Returns an `Err(String)` on malformed input so clap renders the message
/// as a usage error and bird's runner maps it to exit `2`. xurl-rs accepts
/// `Vec<String>` here verbatim, so normalizing on bird's side keeps the
/// surface predictable: the header passed to xurl is what the user typed,
/// minus surrounding whitespace.
pub fn parse_header_kv(s: &str) -> Result<String, String> {
    let (name, value) = s
        .split_once(':')
        .ok_or_else(|| format!("header must be in 'Name: Value' form, got: {s:?}"))?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        return Err(format!("header name must not be empty (got: {s:?})"));
    }
    Ok(format!("{name}: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_well_formed_header() {
        assert_eq!(
            parse_header_kv("X-Custom: foo").expect("valid"),
            "X-Custom: foo"
        );
    }

    #[test]
    fn trims_whitespace_around_name_and_value() {
        assert_eq!(
            parse_header_kv("  X-Custom  :   foo bar  ").expect("valid"),
            "X-Custom: foo bar"
        );
    }

    #[test]
    fn splits_on_first_colon_only() {
        assert_eq!(
            parse_header_kv("Authorization: Bearer abc:def").expect("valid"),
            "Authorization: Bearer abc:def"
        );
    }

    #[test]
    fn rejects_missing_colon() {
        let err = parse_header_kv("X-Custom foo").expect_err("malformed");
        assert!(err.contains("Name: Value"));
    }

    #[test]
    fn rejects_empty_name() {
        let err = parse_header_kv(": value").expect_err("empty name");
        assert!(err.contains("header name"));
    }
}
