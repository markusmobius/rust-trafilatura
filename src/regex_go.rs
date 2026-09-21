use crate::go_unicode;
use regex::{Regex, RegexBuilder};
use std::fmt::Write;

type Ranges = Vec<(u32, u32)>;

#[derive(Clone, Copy, Default)]
struct Flags {
    fold: bool,
    multiline: bool,
    dot_newline: bool,
    ungreedy: bool,
}

struct Term {
    source: String,
    copies: usize,
}

#[derive(Default)]
struct Frame {
    flags: Flags,
    terms: Vec<Term>,
    alternatives: Vec<Term>,
}

impl Frame {
    fn append(&mut self, source: String) {
        self.terms.push(Term { source, copies: 1 })
    }
    fn alternate(&mut self) {
        let terms = std::mem::take(&mut self.terms);
        let copies = terms.iter().map(|term| term.copies).max().unwrap_or(1);
        self.alternatives.push(Term {
            source: terms.into_iter().map(|term| term.source).collect(),
            copies,
        });
    }
    fn finish(mut self) -> Term {
        self.alternate();
        let copies = self
            .alternatives
            .iter()
            .map(|term| term.copies)
            .max()
            .unwrap_or(1);
        Term {
            source: self
                .alternatives
                .into_iter()
                .map(|term| term.source)
                .collect::<Vec<_>>()
                .join("|"),
            copies,
        }
    }
}

struct Parser<'source> {
    source: &'source str,
    index: usize,
}

fn merged(mut ranges: Ranges) -> Ranges {
    ranges.sort_unstable();
    let mut output: Ranges = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = output.last_mut().filter(|last| start <= last.1 + 1) {
            last.1 = last.1.max(end)
        } else {
            output.push((start, end))
        }
    }
    output
}

fn negate(ranges: Ranges) -> Ranges {
    let mut output = Vec::new();
    let mut next = 0;
    for (start, end) in merged(ranges) {
        if next < start {
            output.push((next, start - 1))
        }
        next = end + 1;
    }
    if next <= 0x10ffff {
        output.push((next, 0x10ffff))
    }
    output
}

fn folded(ranges: Ranges, fold: bool) -> Ranges {
    if !fold {
        return ranges;
    }
    let original = merged(ranges);
    let mut output = original.clone();
    for &(scalar, _) in go_unicode::SIMPLE_FOLD {
        let position = original.partition_point(|&(start, _)| start <= scalar);
        if position == 0 || scalar > original[position - 1].1 {
            continue;
        }
        let mut next = scalar;
        loop {
            let index = go_unicode::SIMPLE_FOLD
                .binary_search_by_key(&next, |&(scalar, _)| scalar)
                .unwrap();
            next = go_unicode::SIMPLE_FOLD[index].1;
            if next == scalar {
                break;
            }
            output.push((next, next));
        }
    }
    merged(output)
}

fn class_pattern(ranges: Ranges) -> String {
    let mut output = String::from("[");
    for (start, end) in merged(ranges) {
        for (start, end) in [(start, end.min(0xd7ff)), (start.max(0xe000), end)] {
            if start > end {
                continue;
            }
            write!(output, "\\x{{{start:x}}}").unwrap();
            if start != end {
                write!(output, "-\\x{{{end:x}}}").unwrap()
            }
        }
    }
    if output == "[" {
        return String::from(r"[^\x{0}-\x{10ffff}]");
    }
    output.push(']');
    output
}

fn canonical(name: &str) -> String {
    name.bytes()
        .filter(|byte| !matches!(byte, b'_' | b'-' | b' '))
        .map(|byte| byte.to_ascii_lowercase() as char)
        .collect()
}

fn unicode_group(name: &str, mut negative: bool, fold: bool) -> Option<Ranges> {
    let name = if let Some(name) = name.strip_prefix('^') {
        negative = !negative;
        name
    } else {
        name
    };
    let mut name = canonical(name);
    if let Ok(index) =
        go_unicode::REGEX_UNICODE_ALIASES.binary_search_by_key(&name.as_str(), |&(name, _)| name)
    {
        name = go_unicode::REGEX_UNICODE_ALIASES[index].1.into()
    }
    let index = go_unicode::REGEX_UNICODE_GROUPS
        .binary_search_by_key(&name.as_str(), |&(name, _, _)| name)
        .ok()?;
    let (_, regular, folded) = go_unicode::REGEX_UNICODE_GROUPS[index];
    let ranges = if fold { folded } else { regular }.to_vec();
    Some(if negative { negate(ranges) } else { ranges })
}

fn ascii_group(name: &str, fold: bool) -> Option<Ranges> {
    let negative = name.starts_with('^');
    let name = name.strip_prefix('^').unwrap_or(name);
    let ranges = match name {
        "alnum" => vec![(48, 57), (65, 90), (97, 122)],
        "alpha" => vec![(65, 90), (97, 122)],
        "ascii" => vec![(0, 127)],
        "blank" => vec![(9, 9), (32, 32)],
        "cntrl" => vec![(0, 31), (127, 127)],
        "digit" => vec![(48, 57)],
        "graph" => vec![(33, 126)],
        "lower" => vec![(97, 122)],
        "print" => vec![(32, 126)],
        "punct" => vec![(33, 47), (58, 64), (91, 96), (123, 126)],
        "space" => vec![(9, 13), (32, 32)],
        "upper" => vec![(65, 90)],
        "word" => vec![(48, 57), (65, 90), (95, 95), (97, 122)],
        "xdigit" => vec![(48, 57), (65, 70), (97, 102)],
        _ => return None,
    };
    let ranges = folded(ranges, fold);
    Some(if negative { negate(ranges) } else { ranges })
}

impl Parser<'_> {
    fn remaining(&self) -> &str {
        &self.source[self.index..]
    }
    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }
    fn next(&mut self) -> Option<char> {
        let next = self.peek()?;
        self.index += next.len_utf8();
        Some(next)
    }
    fn character(&mut self) -> Option<u32> {
        let character = self.next()?;
        if character != '\\' {
            return Some(character as u32);
        }
        let escaped = self.next()?;
        match escaped {
            'a' => Some(7),
            'f' => Some(12),
            'n' => Some(10),
            'r' => Some(13),
            't' => Some(9),
            'v' => Some(11),
            '0'..='7' => {
                let mut value = escaped.to_digit(8)?;
                let mut digits = 1;
                while digits < 3
                    && self
                        .peek()
                        .is_some_and(|character| matches!(character, '0'..='7'))
                {
                    value = value * 8 + self.next()?.to_digit(8)?;
                    digits += 1;
                }
                if digits == 1 && escaped != '0' {
                    None
                } else {
                    Some(value)
                }
            }
            'x' => {
                if self.peek() == Some('{') {
                    self.next();
                    let mut value = 0u32;
                    let mut count = 0;
                    while self.peek() != Some('}') {
                        value = value
                            .checked_mul(16)?
                            .checked_add(self.next()?.to_digit(16)?)?;
                        if value > 0x10ffff {
                            return None;
                        }
                        count += 1;
                    }
                    self.next();
                    (count > 0).then_some(value)
                } else {
                    Some(self.next()?.to_digit(16)? * 16 + self.next()?.to_digit(16)?)
                }
            }
            _ if !escaped.is_ascii_alphanumeric() => Some(escaped as u32),
            _ => None,
        }
    }
    fn group_escape(&mut self, fold: bool) -> Option<Option<Ranges>> {
        if !self.remaining().starts_with('\\') {
            return Some(None);
        }
        let escaped = self.remaining().chars().nth(1)?;
        let ranges = match escaped {
            'p' | 'P' => {
                self.index += 2;
                let name = if self.peek() == Some('{') {
                    self.next();
                    let end = self.remaining().find('}')?;
                    let name = self.remaining()[..end].to_owned();
                    self.index += end + 1;
                    name
                } else {
                    self.next()?.to_string()
                };
                unicode_group(&name, escaped == 'P', fold)?
            }
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.index += 2;
                let ranges = match escaped.to_ascii_lowercase() {
                    'd' => vec![(48, 57)],
                    's' => vec![(9, 10), (12, 13), (32, 32)],
                    _ => vec![(48, 57), (65, 90), (95, 95), (97, 122)],
                };
                let ranges = folded(ranges, fold);
                if escaped.is_ascii_uppercase() {
                    negate(ranges)
                } else {
                    ranges
                }
            }
            _ => return Some(None),
        };
        Some(Some(ranges))
    }
    fn class(&mut self, fold: bool) -> Option<String> {
        self.next();
        let negative = self.peek() == Some('^');
        if negative {
            self.next();
        }
        let mut ranges = Vec::new();
        let mut first = true;
        while self.peek() != Some(']') || first {
            first = false;
            if self.remaining().starts_with("[:") {
                if let Some(end) = self.remaining().find(":]") {
                    ranges.extend(ascii_group(&self.remaining()[2..end], fold)?);
                    self.index += end + 2;
                    continue;
                }
            }
            if let Some(group) = self.group_escape(fold)? {
                ranges.extend(group);
                continue;
            }
            let start = self.character()?;
            let end = if self.peek() == Some('-') && self.remaining().chars().nth(1) != Some(']') {
                self.next();
                self.character()?
            } else {
                start
            };
            if end < start {
                return None;
            }
            ranges.extend(folded(vec![(start, end)], fold));
        }
        self.next();
        Some(class_pattern(if negative {
            negate(ranges)
        } else {
            ranges
        }))
    }
    fn repeat_number(&mut self) -> Option<usize> {
        if self.peek() == Some('0')
            && self
                .remaining()
                .as_bytes()
                .get(1)
                .is_some_and(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        let start = self.index;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.next();
        }
        if start == self.index {
            return None;
        }
        Some(self.source[start..self.index].parse().unwrap_or(usize::MAX))
    }
    fn counted_repeat(&mut self) -> Option<(usize, Option<usize>)> {
        self.next();
        let minimum = self.repeat_number()?;
        let maximum = if self.peek() == Some(',') {
            self.next();
            if self.peek() == Some('}') {
                None
            } else {
                Some(self.repeat_number()?)
            }
        } else {
            Some(minimum)
        };
        if self.next()? != '}' {
            return None;
        }
        Some((minimum, maximum))
    }
    fn flags(&mut self, mut flags: Flags) -> Option<(Flags, bool)> {
        let mut negative = false;
        let mut seen_flag = false;
        loop {
            match self.next()? {
                '-' if !negative => {
                    negative = true;
                    seen_flag = false
                }
                'i' => {
                    flags.fold = !negative;
                    seen_flag = true
                }
                'm' => {
                    flags.multiline = !negative;
                    seen_flag = true
                }
                's' => {
                    flags.dot_newline = !negative;
                    seen_flag = true
                }
                'U' => {
                    flags.ungreedy = !negative;
                    seen_flag = true
                }
                ':' if !negative || seen_flag => return Some((flags, true)),
                ')' if !negative || seen_flag => return Some((flags, false)),
                _ => return None,
            }
        }
    }
}

pub(crate) fn compile(source: &str) -> Option<Regex> {
    let mut parser = Parser { source, index: 0 };
    let mut frames = vec![Frame::default()];
    let mut previous_repeat = false;
    while let Some(character) = parser.peek() {
        let frame = frames.last_mut()?;
        let mut repetition = None;
        let mut copies = 1;
        match character {
            '(' => {
                parser.next();
                let mut flags = frame.flags;
                let mut group = true;
                if parser.peek() == Some('?') {
                    parser.next();
                    if parser.remaining().starts_with("P<") || parser.remaining().starts_with('<') {
                        if parser.peek() == Some('P') {
                            parser.next();
                        }
                        parser.next();
                        let end = parser.remaining().find('>')?;
                        let name = &parser.remaining()[..end];
                        if name.is_empty()
                            || !name
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                        {
                            return None;
                        }
                        parser.index += end + 1;
                    } else {
                        (flags, group) = parser.flags(flags)?
                    }
                }
                if group {
                    frames.push(Frame {
                        flags,
                        ..Default::default()
                    });
                    if frames.len() > 1000 {
                        return None;
                    }
                } else {
                    frame.flags = flags
                }
            }
            ')' => {
                parser.next();
                if frames.len() == 1 {
                    return None;
                }
                let mut group = frames.pop()?.finish();
                group.source = format!("(?:{})", group.source);
                frames.last_mut()?.terms.push(group);
            }
            '|' => {
                parser.next();
                frame.alternate()
            }
            '^' | '$' => {
                parser.next();
                frame.append(if frame.flags.multiline {
                    format!("(?m:{character})")
                } else if character == '^' {
                    r"\A".into()
                } else {
                    r"\z".into()
                })
            }
            '.' => {
                parser.next();
                frame.append(if frame.flags.dot_newline {
                    "(?s:.)".into()
                } else {
                    ".".into()
                })
            }
            '[' => {
                let pattern = parser.class(frame.flags.fold)?;
                frame.append(pattern)
            }
            '*' | '+' | '?' => {
                parser.next();
                repetition = Some(character.to_string())
            }
            '{' => {
                let start = parser.index;
                if let Some((minimum, maximum)) = parser.counted_repeat() {
                    if minimum > 1000
                        || maximum.is_some_and(|maximum| maximum > 1000 || maximum < minimum)
                    {
                        return None;
                    }
                    copies = maximum.unwrap_or(minimum).max(1);
                    repetition = Some(source[start..parser.index].into());
                } else {
                    parser.index = start + 1;
                    frame.append(class_pattern(vec![(123, 123)]))
                }
            }
            '\\' if parser.remaining().starts_with(r"\Q") => {
                parser.index += 2;
                while parser.peek().is_some() && !parser.remaining().starts_with(r"\E") {
                    let scalar = parser.next()? as u32;
                    frame.append(class_pattern(folded(
                        vec![(scalar, scalar)],
                        frame.flags.fold,
                    )));
                }
                if parser.remaining().starts_with(r"\E") {
                    parser.index += 2
                }
            }
            '\\' if matches!(
                parser.remaining().chars().nth(1),
                Some('A' | 'z' | 'b' | 'B')
            ) =>
            {
                parser.next();
                let escaped = parser.next()?;
                frame.append(if matches!(escaped, 'b' | 'B') {
                    format!("(?-u:\\{escaped})")
                } else {
                    format!("\\{escaped}")
                });
            }
            _ => {
                let ranges = if let Some(group) = parser.group_escape(frame.flags.fold)? {
                    group
                } else {
                    let scalar = parser.character()?;
                    folded(vec![(scalar, scalar)], frame.flags.fold)
                };
                frame.append(class_pattern(ranges));
            }
        }
        if let Some(repetition) = repetition {
            let frame = frames.last_mut()?;
            if previous_repeat {
                return None;
            }
            let term = frame.terms.last_mut()?;
            if copies >= 2 && term.copies.saturating_mul(copies) > 1000 {
                return None;
            }
            term.copies = term.copies.saturating_mul(copies);
            let mut ungreedy = frame.flags.ungreedy;
            if parser.peek() == Some('?') {
                parser.next();
                ungreedy = !ungreedy
            }
            term.source = format!(
                "(?:{}){repetition}{}",
                term.source,
                if ungreedy { "?" } else { "" }
            );
            previous_repeat = true;
        } else {
            previous_repeat = false
        }
    }
    if frames.len() != 1 {
        return None;
    }
    let pattern = frames.pop()?.finish().source;
    RegexBuilder::new(&pattern)
        .nest_limit(10_000)
        .size_limit(128 << 20)
        .build()
        .ok()
}
