//! The value syntax `--args`, `--tlm`, `--prm` and `--serial` take.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;

/// Parse hex bytes, tolerating the separators people paste: spaces, `:` and `_`.
pub fn bytes(text: &str) -> Result<Vec<u8>> {
    let digits: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '_')
        .collect();
    let digits = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
        .unwrap_or(&digits)
        .to_string();
    if !digits.len().is_multiple_of(2) {
        bail!("`{text}` has an odd number of hex digits");
    }
    (0..digits.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&digits[index..index + 2], 16)
                .with_context(|| format!("`{text}` is not hex"))
        })
        .collect()
}

/// Parse repeated `<key>=<hex>` arguments.
pub fn keyed<K>(entries: &[String], flag: &str) -> Result<BTreeMap<K, Vec<u8>>>
where
    K: std::str::FromStr + Ord,
    K::Err: std::fmt::Display,
{
    entries
        .iter()
        .map(|entry| {
            let (key, hex) = entry
                .split_once('=')
                .with_context(|| format!("{flag} takes <key>=<hex>, got `{entry}`"))?;
            let key = key
                .trim()
                .parse::<K>()
                .map_err(|err| anyhow::anyhow!("{flag}: `{key}` is not a valid key: {err}"))?;
            Ok((key, bytes(hex)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_hex() {
        assert_eq!(bytes("00ff10").expect("valid"), vec![0x00, 0xff, 0x10]);
        assert_eq!(bytes("").expect("valid"), Vec::<u8>::new());
    }

    /// Bytes get pasted in with whatever separators the source used.
    #[test]
    fn tolerates_separators_and_a_prefix() {
        let expected = vec![0xde, 0xad, 0xbe, 0xef];
        for text in [
            "deadbeef",
            "de ad be ef",
            "de:ad:be:ef",
            "dead_beef",
            "0xdeadbeef",
        ] {
            assert_eq!(bytes(text).expect("valid"), expected, "{text}");
        }
    }

    #[test]
    fn rejects_malformed_hex() {
        assert!(bytes("abc").is_err(), "odd digit count");
        assert!(bytes("zz").is_err(), "not hex");
    }

    #[test]
    fn parses_keyed_overrides() {
        let telemetry: BTreeMap<i64, Vec<u8>> =
            keyed(&["12=0001".to_string(), "3 = ff".to_string()], "--tlm").expect("valid");
        assert_eq!(telemetry.get(&12), Some(&vec![0x00, 0x01]));
        assert_eq!(telemetry.get(&3), Some(&vec![0xff]));
    }

    #[test]
    fn rejects_keyed_overrides_without_a_key_or_with_a_bad_one() {
        assert!(keyed::<i64>(&["0001".to_string()], "--tlm").is_err());
        assert!(keyed::<i64>(&["notanid=00".to_string()], "--tlm").is_err());
        assert!(keyed::<i64>(&["1=notahexvalue".to_string()], "--tlm").is_err());
    }
}
