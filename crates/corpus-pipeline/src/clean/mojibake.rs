//! Mojibake repair for the dominant artifact family: UTF-8 text mis-decoded as
//! Windows-1252 (`â€™`, `Ã©`, ...). See docs/CLEANING_PLAN.md §2.
//!
//! Guards:
//! 1. Windows-1252 (not Latin-1) round-trip.
//! 2. Only attempt when indicator patterns are present.
//! 3. Accept only if it reduces indicators and adds no U+FFFD.
//! 4. Iterate to a fixed point, capped at 3 passes.

use encoding_rs::WINDOWS_1252;

const MAX_PASSES: usize = 3;
const INDICATORS: &[&str] = &["Ã", "Â", "â€", "â„", " Å", "Ã©", "Ã¨", "Ã¶", "Ã¼", "Ã¤"];

/// Count mojibake indicator occurrences in `text`.
fn indicator_count(text: &str) -> usize {
    INDICATORS.iter().map(|pat| text.matches(pat).count()).sum()
}

fn replacement_count(text: &str) -> usize {
    text.matches('\u{FFFD}').count()
}

/// One Windows-1252 round-trip: re-encode each character that maps to a single
/// CP1252 byte back to that byte, while passing characters that have no CP1252
/// mapping through as their raw UTF-8 bytes (they are not part of the mojibake).
/// The reassembled bytes are then decoded as UTF-8. Returns `None` if the result
/// is not valid UTF-8.
fn roundtrip(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        if (ch as u32) < 0x80 {
            bytes.push(ch as u8);
            continue;
        }
        let encoded = ch.encode_utf8(&mut buf);
        let (cp1252, _, had_unmappable) = WINDOWS_1252.encode(encoded);
        if had_unmappable {
            // Not a CP1252 character, so not part of the mojibake: keep as-is.
            bytes.extend_from_slice(encoded.as_bytes());
        } else {
            bytes.extend_from_slice(&cp1252);
        }
    }
    std::str::from_utf8(&bytes).ok().map(|s| s.to_string())
}

/// Repair mojibake if present and the repair is an improvement; otherwise return
/// the input unchanged.
pub fn fix_mojibake(text: &str) -> String {
    let mut current = text.to_string();
    for _ in 0..MAX_PASSES {
        let indicators = indicator_count(&current);
        if indicators == 0 {
            break;
        }
        let Some(candidate) = roundtrip(&current) else {
            break;
        };
        let improves = indicator_count(&candidate) < indicators
            && replacement_count(&candidate) <= replacement_count(&current);
        if improves {
            current = candidate;
        } else {
            break;
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_smart_quote_artifact() {
        // "it’s" mis-decoded becomes "itâ€™s".
        assert_eq!(fix_mojibake("itâ€™s"), "it\u{2019}s");
    }

    #[test]
    fn repairs_accented_artifact() {
        // "café" mis-decoded becomes "cafÃ©".
        assert_eq!(fix_mojibake("cafÃ©"), "café");
    }

    #[test]
    fn leaves_clean_text_untouched() {
        assert_eq!(fix_mojibake("Soomaaliya waa dal"), "Soomaaliya waa dal");
        assert_eq!(fix_mojibake("café crème"), "café crème");
    }

    #[test]
    fn leaves_somali_untouched() {
        let s = "Waxaan tagay magaalada Muqdisho oo ah caasimadda dalka.";
        assert_eq!(fix_mojibake(s), s);
    }
}
