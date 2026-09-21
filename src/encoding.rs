mod decoder;
mod tables;

use crate::Error;
use unicode_normalization::{is_nfc_stream_safe_quick, IsNormalized, UnicodeNormalization};

pub(crate) fn decode(input: &[u8]) -> Result<String, Error> {
    if utf8(input) == 100 {
        return Ok(decode_as(input, "UTF-8"));
    }
    let mut best = ("", 0);
    for result in scores(input) {
        if result.1 > best.1 {
            best = result;
        }
    }
    if best.1 == 0 {
        return Err(Error::CharsetNotDetected);
    }
    Ok(decode_as(input, best.0))
}

pub(crate) fn decode_as(input: &[u8], charset: &str) -> String {
    let encoding =
        encoding_rs::Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let decoded = if encoding == encoding_rs::UTF_8 {
        encoding.decode_without_bom_handling(input).0.into_owned()
    } else {
        decoder::decode(input, encoding)
    };
    if !decoded.contains('\u{ad}')
        && is_nfc_stream_safe_quick(decoded.chars()) == IsNormalized::Yes
    {
        decoded
    } else {
        decoded
            .nfd()
            .stream_safe()
            .filter(|&character| character != '\u{ad}')
            .stream_safe()
            .nfc()
            .collect()
    }
}

pub(crate) fn scores(raw: &[u8]) -> Vec<(&'static str, i32)> {
    let input = strip_tags(raw);
    let has_c1 = input.iter().any(|byte| (0x80..=0x9f).contains(byte));
    let mut scores = vec![
        ("UTF-8", utf8(raw)),
        (
            "UTF-16BE",
            if raw.starts_with(&[0xfe, 0xff]) {
                100
            } else {
                0
            },
        ),
        (
            "UTF-16LE",
            if raw.starts_with(&[0xff, 0xfe]) && !raw.starts_with(&[0xff, 0xfe, 0, 0]) {
                100
            } else {
                0
            },
        ),
        ("UTF-32BE", utf32(raw, false)),
        ("UTF-32LE", utf32(raw, true)),
    ];
    for &(charset, c1_charset, map, ngrams) in tables::SINGLE_BYTE {
        let charset = if has_c1 && !c1_charset.is_empty() {
            c1_charset
        } else {
            charset
        };
        scores.push((charset, single_byte(&input, map, ngrams)));
    }
    for &(charset, decoder, common) in tables::MULTI_BYTE {
        scores.push((charset, multi_byte(raw, decoder, common)));
    }
    for &(charset, escapes) in tables::ISO_2022 {
        scores.push((charset, iso_2022(&input, escapes)));
    }
    scores
}

fn strip_tags(raw: &[u8]) -> Vec<u8> {
    let mut stripped = Vec::with_capacity(8192);
    let (mut open, mut bad) = (0, 0);
    let mut in_markup = false;
    for &byte in raw {
        if byte == b'<' {
            if in_markup {
                bad += 1;
            }
            in_markup = true;
            open += 1;
        }
        if !in_markup {
            stripped.push(byte);
            if stripped.len() >= 8192 {
                break;
            }
        }
        if byte == b'>' {
            in_markup = false;
        }
    }
    if open < 5 || open / 5 < bad || (stripped.len() < 100 && raw.len() > 600) {
        raw[..raw.len().min(8192)].to_vec()
    } else {
        stripped
    }
}

fn unicode_confidence(bom: bool, valid: u32, invalid: u32, ascii: bool) -> i32 {
    if (bom || valid > 3) && invalid == 0 {
        100
    } else if (bom && valid > invalid * 10) || (valid > 0 && invalid == 0) {
        80
    } else if ascii && valid == 0 && invalid == 0 {
        10
    } else if valid > invalid * 10 {
        25
    } else {
        0
    }
}

fn utf8(raw: &[u8]) -> i32 {
    let (mut valid, mut invalid) = (0, 0);
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte & 0x80 == 0 {
            index += 1;
            continue;
        }
        let mut trailing: u8 = if byte & 0xe0 == 0xc0 {
            1
        } else if byte & 0xf0 == 0xe0 {
            2
        } else if byte & 0xf8 == 0xf0 {
            3
        } else {
            invalid += 1;
            if invalid > 5 {
                break;
            }
            0
        };
        index += 1;
        while index < raw.len() {
            if raw[index] & 0xc0 != 0x80 {
                invalid += 1;
                break;
            }
            trailing = trailing.wrapping_sub(1);
            if trailing == 0 {
                valid += 1;
                break;
            }
            index += 1;
        }
        index += 1;
    }
    unicode_confidence(raw.starts_with(&[0xef, 0xbb, 0xbf]), valid, invalid, true)
}

fn utf32(raw: &[u8], little: bool) -> i32 {
    let (mut valid, mut invalid) = (0, 0);
    for &bytes in raw.as_chunks::<4>().0 {
        let character = if little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        };
        if character >= 0x10ffff || (0xd800..=0xdfff).contains(&character) {
            invalid += 1;
        } else {
            valid += 1;
        }
    }
    let bom = if little {
        &[0xff, 0xfe, 0, 0]
    } else {
        &[0, 0, 0xfe, 0xff]
    };
    unicode_confidence(raw.starts_with(bom), valid, invalid, false)
}

fn single_byte(input: &[u8], map: &[u8], table: &[u32]) -> i32 {
    let (mut ngram, mut count, mut hits) = (0u32, 0u32, 0u32);
    let mut ignore_space = false;
    for byte in input
        .iter()
        .map(|&byte| map[byte as usize])
        .filter(|&byte| byte != 0)
        .chain([0x20])
    {
        if !(byte == 0x20 && ignore_space) {
            ngram = ((ngram << 8) | u32::from(byte)) & 0xffffff;
            count += 1;
            if table.binary_search(&ngram).is_ok() {
                hits += 1;
            }
        }
        ignore_space = byte == 0x20;
    }
    let rate = if count == 0 {
        0.0
    } else {
        hits as f32 / count as f32
    };
    if rate > 0.33 {
        98
    } else {
        (rate * 300.0) as i32
    }
}

fn multi_byte(raw: &[u8], decoder: &str, common: &[u16]) -> i32 {
    let (mut total, mut bad, mut double, mut common_count) = (0, 0, 0, 0);
    let mut rest = raw;
    while !rest.is_empty() {
        let (character, used, error) = decode_one(rest, decoder);
        rest = &rest[used..];
        if rest.is_empty() {
            break;
        }
        total += 1;
        if error {
            bad += 1;
        } else if character > 0xff {
            double += 1;
            if common.binary_search(&character).is_ok() {
                common_count += 1;
            }
        }
        if bad >= 2 && bad * 5 >= double {
            return 0;
        }
    }
    if double <= 10 && bad == 0 {
        return if double == 0 && total < 10 { 0 } else { 10 };
    }
    if double < 20 * bad {
        return 0;
    }
    let scale = 90.0 / (f64::from(double) / 4.0).ln();
    (((f64::from(common_count) + 1.0).ln() * scale + 10.0) as i32).clamp(0, 100)
}

fn decode_one(raw: &[u8], decoder: &str) -> (u16, usize, bool) {
    let first = raw[0];
    let character = u16::from(first);
    let single = match decoder {
        "sjis" => first <= 0x7f || (first > 0xa0 && first <= 0xdf),
        "euc" => first <= 0x8d,
        "big5" => first <= 0x7f || first == 0xff,
        "gb_18030" => first <= 0x80,
        _ => unreachable!(),
    };
    if single {
        return (character, 1, false);
    }
    let Some(&second) = raw.get(1) else {
        return (character, raw.len(), true);
    };
    let character = character << 8 | u16::from(second);
    match decoder {
        "sjis" => (character, 2, !(0x40..=0xfe).contains(&second)),
        "big5" => (
            character,
            2,
            second < 0x40 || second == 0x7f || second == 0xff,
        ),
        "euc" => {
            if (0xa1..=0xfe).contains(&first) || first == 0x8e {
                return (character, 2, second < 0xa1);
            }
            if first == 0x8f {
                let Some(&third) = raw.get(2) else {
                    return (0, raw.len(), true);
                };
                return (character | u16::from(third), 3, third < 0xa1);
            }
            (character, 2, false)
        }
        "gb_18030" => {
            if !(0x81..=0xfe).contains(&first) {
                return (character, 2, false);
            }
            if (0x40..=0x7e).contains(&second) || (0x80..=0xfe).contains(&second) {
                return (character, 2, false);
            }
            if second.is_ascii_digit() {
                let Some(&third) = raw.get(2) else {
                    return (0, raw.len(), true);
                };
                if (0x81..=0xfe).contains(&third) {
                    let Some(&fourth) = raw.get(3) else {
                        return (0, raw.len(), true);
                    };
                    if fourth.is_ascii_digit() {
                        return (u16::from(third) << 8 | u16::from(fourth), 4, false);
                    }
                    return (character, 4, true);
                }
                return (character, 3, true);
            }
            (character, 2, true)
        }
        _ => unreachable!(),
    }
}

fn iso_2022(input: &[u8], escapes: &[&[u8]]) -> i32 {
    let (mut hits, mut misses, mut shifts) = (0, 0, 0);
    let mut index = 0;
    while index < input.len() {
        if input[index] == 0x1b {
            if let Some(escape) = escapes
                .iter()
                .find(|escape| input[index + 1..].starts_with(escape))
            {
                hits += 1;
                index += escape.len();
            } else {
                misses += 1;
            }
        } else if input[index] == 0x0e || input[index] == 0x0f {
            shifts += 1;
        }
        index += 1;
    }
    if hits == 0 {
        return 0;
    }
    let mut quality = (100 * hits - 100 * misses) / (hits + misses);
    if hits + shifts < 5 {
        quality -= (5 - hits - shifts) * 10;
    }
    quality.max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_decode_preserves_imported_decoder() {
        let check = |input: &[u8]| {
            let expected: String = decoder::decode(input, encoding_rs::UTF_8)
                .nfd()
                .stream_safe()
                .filter(|&character| character != '\u{ad}')
                .stream_safe()
                .nfc()
                .collect();
            assert_eq!(decode_as(input, "UTF-8"), expected, "input {input:?}");
        };
        check(&[]);
        for first in 0..=u8::MAX {
            check(&[first]);
            for second in 0..=u8::MAX {
                check(&[first, second]);
            }
        }
        for sample in [
            &[0xef, 0xbb, 0xbf, b'A'][..],
            &[0xe0, 0x80, 0x80],
            &[0xed, 0xa0, 0x80],
            &[0xf0, 0x80, 0x80, 0x80],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xf0, 0x9f, 0x92, 0xa9],
        ] {
            for length in 0..=sample.len() {
                for byte in 0..=u8::MAX {
                    let mut input = sample[..length].to_vec();
                    input.push(byte);
                    check(&input);
                }
            }
        }
    }
}
