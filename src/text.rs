use crate::{go_unicode::KEEP_RANGES, Node};
use regex::Regex;
use std::{borrow::Cow, collections::HashSet, sync::LazyLock};

static IMAGE_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[^ \t\n\f\r]+\.(avif|bmp|gif|hei[cf]|jpe?g|png|webp)(?-u:\b|$)").unwrap()
});

pub fn is_space(character: char) -> bool {
    matches!(character, '\u{9}'..='\u{d}' | '\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

pub fn trim_space(text: &str) -> &str {
    text.trim_matches(is_space)
}

pub fn trim(text: &str) -> String {
    trim_cow(text).into_owned()
}

pub(crate) fn trim_cow(text: &str) -> Cow<'_, str> {
    let text = trim_space(text);
    let mut previous_space = false;
    if text.chars().all(|character| {
        let is_normalized = if character == ' ' {
            !previous_space
        } else {
            !is_space(character)
        };
        previous_space = character == ' ';
        is_normalized
    }) {
        return Cow::Borrowed(text);
    }
    let mut output = String::with_capacity(text.len());
    for part in text.split(is_space).filter(|part| !part.is_empty()) {
        if !output.is_empty() {
            output.push(' ');
        }
        output.push_str(part);
    }
    Cow::Owned(output)
}

pub fn word_count(text: &str) -> usize {
    text.split(is_space).filter(|part| !part.is_empty()).count()
}

pub fn remove_control_characters(text: &str) -> String {
    text.chars()
        .filter(|&character| {
            if character.is_ascii() {
                return matches!(character, '\u{9}'..='\u{d}' | '\u{20}'..='\u{7e}');
            }
            let scalar = character as u32;
            let index = KEEP_RANGES.partition_point(|&(_, end)| end < scalar);
            KEEP_RANGES
                .get(index)
                .is_some_and(|&(start, _)| start <= scalar)
        })
        .collect()
}

pub fn unescape_html(text: &str) -> String {
    use markup5ever::data::{C1_REPLACEMENTS, NAMED_ENTITIES};

    let mut remaining = text;
    let mut output = String::with_capacity(text.len());
    while let Some(offset) = remaining.find('&') {
        output.push_str(&remaining[..offset]);
        remaining = &remaining[offset..];
        let bytes = remaining.as_bytes();
        if bytes.get(1) == Some(&b'#') && bytes.len() > 2 {
            let hexadecimal = matches!(bytes.get(2), Some(b'x' | b'X'));
            let start = if hexadecimal { 3 } else { 2 };
            let mut end = start;
            let mut value = 0u32;
            while let Some(&byte) = bytes.get(end) {
                let digit = if hexadecimal {
                    (byte as char).to_digit(16)
                } else if byte.is_ascii_digit() {
                    Some(u32::from(byte - b'0'))
                } else {
                    None
                };
                let Some(digit) = digit else { break };
                if value <= 0x10ffff {
                    value = value * if hexadecimal { 16 } else { 10 } + digit;
                }
                end += 1;
            }
            if end > start {
                if bytes.get(end) == Some(&b';') {
                    end += 1;
                }
                let character = if value == 0 {
                    '\u{fffd}'
                } else {
                    char::from_u32(value).unwrap_or('\u{fffd}')
                };
                output.push(if (0x80..=0x9f).contains(&value) {
                    C1_REPLACEMENTS[(value - 0x80) as usize].unwrap_or(character)
                } else {
                    character
                });
                remaining = &remaining[end..];
                continue;
            }
        } else {
            let mut end = 1;
            while bytes.get(end).is_some_and(u8::is_ascii_alphanumeric) {
                end += 1;
            }
            if bytes.get(end) == Some(&b';') {
                end += 1;
            }
            let name = &remaining[1..end];
            let mut matched = NAMED_ENTITIES
                .get(name)
                .copied()
                .filter(|&(first, _)| first != 0)
                .map(|points| (points, end));
            if matched.is_none() {
                for length in (2..=name.len().saturating_sub(1).min(6)).rev() {
                    if let Some(&(first, 0)) = NAMED_ENTITIES.get(&name[..length]) {
                        if first != 0 {
                            matched = Some(((first, 0), length + 1));
                            break;
                        }
                    }
                }
            }
            if let Some(((first, second), consumed)) = matched {
                output.push(char::from_u32(first).unwrap());
                if second != 0 {
                    output.push(char::from_u32(second).unwrap());
                }
                remaining = &remaining[consumed..];
                continue;
            }
        }
        output.push('&');
        remaining = &remaining[1..];
    }
    output.push_str(remaining);
    output
}

fn case_flags(character: char) -> u8 {
    let scalar = character as u32;
    let ranges = crate::go_unicode::CASE_RANGES;
    ranges[ranges.partition_point(|&(_, end, _)| end < scalar)].2
}

fn case_mapping(character: char) -> Option<&'static (u32, &'static str, &'static str, char)> {
    let mappings = crate::go_unicode::CASE_MAPPINGS;
    mappings
        .binary_search_by_key(&(character as u32), |&(scalar, _, _, _)| scalar)
        .ok()
        .map(|index| &mappings[index])
}

pub fn is_upper(character: char) -> bool {
    case_flags(character) & 16 != 0
}

pub fn to_lower(text: &str) -> String {
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    text.chars()
        .map(|character| {
            if character.is_ascii() {
                character.to_ascii_lowercase()
            } else {
                case_mapping(character).map_or(character, |mapping| mapping.3)
            }
        })
        .collect()
}

pub fn title_case(text: &str) -> String {
    let characters: Vec<_> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut mid_word = false;
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        let flags = case_flags(character);
        let was_mid = flags & 8 != 0;
        if flags & 1 != 0 {
            if mid_word && character == '\u{3a3}' {
                let following = index + 1;
                let mut consumed = following;
                let mut sigma = '\u{3c2}';
                let mut previous_mid = false;
                for _ in 0..31 {
                    let Some(&next) = characters.get(consumed) else {
                        break;
                    };
                    let next_flags = case_flags(next);
                    if next_flags & 2 == 0 {
                        if next_flags & 1 != 0 {
                            sigma = '\u{3c3}';
                        }
                        break;
                    }
                    let next_mid = next_flags & 8 != 0;
                    if (previous_mid && next_mid) || next_flags & 4 != 0 {
                        mid_word = false;
                    }
                    previous_mid = next_mid;
                    consumed += 1;
                }
                output.push(sigma);
                output.extend(&characters[following..consumed]);
                index = consumed;
                continue;
            }
            if let Some(mapping) = case_mapping(character) {
                output.push_str(if mid_word { mapping.2 } else { mapping.1 });
            } else {
                output.push(character);
            }
            mid_word = true;
        } else {
            output.push(character);
            if flags & 4 != 0 {
                mid_word = false;
            }
        }
        index += 1;
        if was_mid
            && characters
                .get(index)
                .is_some_and(|&next| case_flags(next) & 8 != 0)
        {
            mid_word = false;
        }
    }
    output
}

pub fn remove_emojis(text: &str) -> String {
    use crate::go_unicode::{
        EMOJIS, EMOJI_SCALARS, EMOJI_TRIM_RANGES, GRAPHEME_RANGES, GRAPHEME_TRANSITIONS,
    };

    let mut start = 0;
    let mut state = 0;
    let mut clusters = String::with_capacity(text.len());
    for (offset, character) in text.char_indices() {
        let scalar = character as u32;
        let property =
            GRAPHEME_RANGES[GRAPHEME_RANGES.partition_point(|&(_, end, _)| end < scalar)].2;
        let (next, boundary) = GRAPHEME_TRANSITIONS[state][property];
        if boundary {
            let cluster = &text[start..offset];
            if EMOJIS.binary_search(&cluster).is_err() {
                clusters.push_str(cluster);
            }
            start = offset;
        }
        state = next;
    }
    let final_cluster = &text[start..];
    if EMOJIS.binary_search(&final_cluster).is_err() {
        clusters.push_str(final_cluster);
    }
    let singles_removed: String = clusters
        .chars()
        .filter(|&character| EMOJI_SCALARS.binary_search(&(character as u32)).is_err())
        .collect();
    singles_removed
        .trim_matches(|character: char| {
            let scalar = character as u32;
            let index = EMOJI_TRIM_RANGES.partition_point(|&(_, end)| end < scalar);
            !EMOJI_TRIM_RANGES
                .get(index)
                .is_some_and(|&(start, _)| start <= scalar)
        })
        .into()
}

pub fn is_image_file(source: &str) -> bool {
    !source.is_empty() && source.len() <= 8192 && IMAGE_EXTENSION.is_match(source)
}

pub fn is_image_element(element: &Node) -> bool {
    element.attrs.iter().any(|attribute| {
        (attribute.key == "src" || attribute.key.starts_with("data-src"))
            && is_image_file(&attribute.value)
    })
}

pub fn uniquify_lists(currents: &[impl AsRef<str>]) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for current in currents {
        let current = current.as_ref();
        let separator = if current.matches(';').count() > current.matches(',').count() {
            ';'
        } else {
            ','
        };
        for entry in current.split(separator) {
            let entry = trim(entry).replace(['\'', '"'], "");
            if !entry.is_empty() && seen.insert(entry.clone()) {
                result.push(entry);
            }
        }
    }
    result
}
