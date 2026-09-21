use encoding_rs::{Encoding, BIG5, EUC_JP, GB18030, ISO_2022_JP, SHIFT_JIS};

pub(super) fn decode(input: &[u8], encoding: &'static Encoding) -> String {
    if encoding == encoding_rs::REPLACEMENT {
        return "\u{fffd}".into();
    }
    if encoding == ISO_2022_JP {
        return iso_2022_jp(input);
    }
    if ![BIG5, EUC_JP, GB18030, SHIFT_JIS].contains(&encoding) {
        let decoded = encoding.decode_without_bom_handling(input).0;
        let replace_c1 =
            encoding.name().starts_with("windows-") || encoding.name().starts_with("ISO-8859-");
        return decoded
            .chars()
            .map(|character| {
                if replace_c1 && ('\u{80}'..='\u{9f}').contains(&character) {
                    '\u{fffd}'
                } else {
                    character
                }
            })
            .collect();
    }
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest[0] < 0x80 {
            output.push(char::from(rest[0]));
            index += 1;
            continue;
        }
        let (length, valid) = if encoding == EUC_JP {
            euc_length(rest)
        } else if encoding == BIG5 {
            big5_length(rest)
        } else if encoding == GB18030 {
            gb_length(rest)
        } else {
            sjis_length(rest)
        };
        if valid && encoding == GB18030 {
            let character = match length {
                1 => 0x20ac,
                2 => {
                    let offset = if rest[1] < 0x7f {
                        rest[1] - 0x40
                    } else {
                        rest[1] - 0x41
                    };
                    let pointer = usize::from(rest[0] - 0x81) * 190 + usize::from(offset);
                    u32::from(super::tables::GB_DECODE.get(pointer).copied().unwrap_or(0))
                }
                _ => {
                    let pointer = ((u32::from(rest[0] - 0x81) * 10 + u32::from(rest[1] - 0x30))
                        * 126
                        + u32::from(rest[2] - 0x81))
                        * 10
                        + u32::from(rest[3] - 0x30);
                    if pointer < 39420 {
                        let position = super::tables::GB_RANGES
                            .partition_point(|&(start, _)| start <= pointer);
                        let (start, mapped) = super::tables::GB_RANGES[position - 1];
                        pointer + mapped - start
                    } else {
                        pointer - 189000 + 0x10000
                    }
                }
            };
            output.push(
                char::from_u32(character)
                    .filter(|&character| character != '\0')
                    .unwrap_or('\u{fffd}'),
            );
        } else if valid {
            let decoded = encoding.decode_without_bom_handling(&rest[..length]).0;
            if decoded.contains('\u{fffd}')
                || (encoding == SHIFT_JIS
                    && decoded
                        .chars()
                        .any(|character| ('\u{e000}'..='\u{e757}').contains(&character)))
            {
                output.push('\u{fffd}');
            } else {
                output.push_str(&decoded);
            }
        } else {
            output.push('\u{fffd}');
        }
        index += length;
    }
    output
}

fn euc_length(input: &[u8]) -> (usize, bool) {
    let first = input[0];
    let Some(&second) = input.get(1) else {
        return (1, false);
    };
    if first == 0x8e {
        if second < 0xa1 || second == 0xff {
            return (1, false);
        }
        return (2, second <= 0xdf);
    }
    if first == 0x8f {
        let Some(&third) = input.get(2) else {
            return (if (0xa1..0xfe).contains(&second) { 2 } else { 1 }, false);
        };
        if !(0xa1..=0xfe).contains(&second) {
            return (1, false);
        }
        if !(0xa1..=0xfe).contains(&third) {
            return (2, false);
        }
        return (3, true);
    }
    if (0xa1..=0xfe).contains(&first) && (0xa1..=0xfe).contains(&second) {
        (2, true)
    } else {
        (1, false)
    }
}

fn big5_length(input: &[u8]) -> (usize, bool) {
    if !(0x81..=0xfe).contains(&input[0]) {
        return (1, false);
    }
    let Some(&second) = input.get(1) else {
        return (1, false);
    };
    if second < 0x40 {
        (1, false)
    } else {
        (
            2,
            (0x40..0x7f).contains(&second) || (0xa1..=0xfe).contains(&second),
        )
    }
}

fn sjis_length(input: &[u8]) -> (usize, bool) {
    let first = input[0];
    if first == 0x80 || (0xa1..0xe0).contains(&first) {
        return (1, true);
    }
    if !((0x81..0xa0).contains(&first) || (0xe0..0xfd).contains(&first)) {
        return (1, false);
    }
    let Some(&second) = input.get(1) else {
        return (1, false);
    };
    if second < 0x40 || second == 0x7f {
        (1, false)
    } else {
        (2, second < 0xfd)
    }
}

fn gb_length(input: &[u8]) -> (usize, bool) {
    let first = input[0];
    if first == 0x80 {
        return (1, true);
    }
    if first == 0xff {
        return (1, false);
    }
    let Some(&second) = input.get(1) else {
        return (1, false);
    };
    if (0x40..0x7f).contains(&second) || (0x80..0xff).contains(&second) {
        return (2, true);
    }
    if (0x30..0x40).contains(&second)
        && input.len() >= 4
        && (0x81..0xff).contains(&input[2])
        && (0x30..0x3a).contains(&input[3])
    {
        let pointer = ((u32::from(first - 0x81) * 10 + u32::from(second - 0x30)) * 126
            + u32::from(input[2] - 0x81))
            * 10
            + u32::from(input[3] - 0x30);
        if pointer < 39420 || (189000..189000 + 0x100000).contains(&pointer) {
            return (4, true);
        }
    }
    (1, false)
}

fn iso_2022_jp(input: &[u8]) -> String {
    let mut output = String::new();
    let (mut state, mut index) = (0, 0);
    while index < input.len() {
        let first = input[index];
        if first >= 0x80 {
            output.push('\u{fffd}');
            index += 1;
            continue;
        }
        if first == 0x1b {
            let mut escape = None;
            for (sequence, next_state) in [
                (b"\x1b$@".as_slice(), 2),
                (b"\x1b$B", 2),
                (b"\x1b$(D", 3),
                (b"\x1b(B", 0),
                (b"\x1b(J", 0),
                (b"\x1b(I", 1),
            ] {
                if input[index..].starts_with(sequence) {
                    escape = Some((sequence.len(), next_state));
                    break;
                }
            }
            if let Some((length, next_state)) = escape {
                state = next_state;
                index += length;
            } else {
                output.push('\u{fffd}');
                index += 1;
            }
            continue;
        }
        match state {
            0 => {
                output.push(char::from(first));
                index += 1;
            }
            1 => {
                output.push(if (0x21..0x60).contains(&first) {
                    char::from_u32(u32::from(first) + 0xff61 - 0x21).unwrap()
                } else {
                    '\u{fffd}'
                });
                index += 1;
            }
            _ if first == b'\n' => {
                state = 0;
                output.push('\n');
                index += 1;
            }
            _ => {
                let Some(&second) = input.get(index + 1) else {
                    output.push('\u{fffd}');
                    break;
                };
                let pointer = usize::from(first.wrapping_sub(0x21)) * 94
                    + usize::from(second.wrapping_sub(0x21));
                if pointer < 94 * 94 {
                    let bytes = [
                        0x8f,
                        0xa1 + (pointer / 94) as u8,
                        0xa1 + (pointer % 94) as u8,
                    ];
                    let encoded = if state == 2 { &bytes[1..] } else { &bytes[..] };
                    let decoded = EUC_JP.decode_without_bom_handling(encoded).0;
                    output.push_str(&decoded);
                } else {
                    output.push('\u{fffd}');
                }
                index += 2;
            }
        }
    }
    output
}
