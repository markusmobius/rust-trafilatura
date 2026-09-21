use std::borrow::Cow;

fn unicode_escape(source: &[u8]) -> Option<u16> {
    if source.len() < 6 || &source[..2] != b"\\u" {
        return None;
    }
    let mut scalar = 0u16;
    for &byte in &source[2..6] {
        scalar = scalar * 16 + (byte as char).to_digit(16)? as u16;
    }
    Some(scalar)
}

pub(crate) fn prepare(source: &str) -> Option<Cow<'_, str>> {
    let bytes = source.as_bytes();
    let mut replacement: Option<Vec<u8>> = None;
    let mut quoted = false;
    let mut depth = 0usize;
    let mut offset = 0;
    while offset < bytes.len() {
        let byte = bytes[offset];
        if quoted && byte == b'\\' {
            if let Some(scalar) = unicode_escape(&bytes[offset..]) {
                if (0xd800..=0xdbff).contains(&scalar)
                    && unicode_escape(&bytes[offset + 6..])
                        .is_some_and(|next| (0xdc00..=0xdfff).contains(&next))
                {
                    offset += 12;
                    continue;
                }
                if (0xd800..=0xdfff).contains(&scalar) {
                    replacement.get_or_insert_with(|| bytes.to_vec())[offset + 2..offset + 6]
                        .copy_from_slice(b"fffd");
                }
                offset += 6;
            } else {
                offset += 2;
            }
            continue;
        }
        if byte == b'"' {
            quoted = !quoted;
        } else if !quoted {
            match byte {
                b'[' | b'{' => {
                    depth += 1;
                    if depth > 10_000 {
                        return None;
                    }
                }
                b']' | b'}' => depth = depth.checked_sub(1)?,
                _ => {}
            }
        }
        offset += 1;
    }
    Some(match replacement {
        Some(bytes) => Cow::Owned(String::from_utf8(bytes).unwrap()),
        None => Cow::Borrowed(source),
    })
}
