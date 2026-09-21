use crate::{etree, text, Document, Kind, NodeId};
use regex::Regex;

enum Expression {
    Tag(String),
    Attribute {
        key: String,
        value: String,
        operation: String,
        insensitive: bool,
        pattern: Option<Regex>,
    },
    All(Vec<usize>),
    Combined(usize, u8, usize),
    Relative(String, Vec<usize>),
    Contains(String, bool),
    Matches(Regex, bool),
    Nth(i64, i64, bool, bool),
    Only(bool),
    Pseudo(String),
    Language(String),
}

pub(crate) struct Selector {
    expressions: Vec<Expression>,
    group: Vec<usize>,
}

struct Parser<'source> {
    source: &'source [u8],
    index: usize,
    expressions: Vec<Expression>,
}

fn space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c')
}
fn name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte > 127
}
fn name_char(byte: u8) -> bool {
    name_start(byte) || byte.is_ascii_digit() || byte == b'-'
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.source.get(self.index).copied()
    }
    fn push(&mut self, expression: Expression) -> usize {
        let index = self.expressions.len();
        self.expressions.push(expression);
        index
    }
    fn whitespace(&mut self) -> bool {
        let start = self.index;
        while let Some(byte) = self.peek() {
            if space(byte) {
                self.index += 1;
                continue;
            }
            if self.source[self.index..].starts_with(b"/*") {
                if let Some(end) = self.source[self.index + 2..]
                    .windows(2)
                    .position(|pair| pair == b"*/")
                {
                    self.index += end + 4;
                    continue;
                }
            }
            break;
        }
        self.index != start
    }
    fn consume(&mut self, byte: u8) -> Option<()> {
        if self.peek()? != byte {
            return None;
        }
        self.index += 1;
        Some(())
    }
    fn opening(&mut self) -> Option<()> {
        self.consume(b'(')?;
        self.whitespace();
        Some(())
    }
    fn closing(&mut self) -> Option<()> {
        self.whitespace();
        self.consume(b')')
    }
    fn escape(&mut self, output: &mut Vec<u8>) -> Option<()> {
        self.consume(b'\\')?;
        let byte = self.peek()?;
        if matches!(byte, b'\r' | b'\n' | b'\x0c') {
            return None;
        }
        if byte.is_ascii_hexdigit() {
            let start = self.index;
            while self.index - start < 6 && self.peek().is_some_and(|byte| byte.is_ascii_hexdigit())
            {
                self.index += 1
            }
            let scalar = u32::from_str_radix(
                std::str::from_utf8(&self.source[start..self.index]).ok()?,
                16,
            )
            .ok()?;
            let mut encoded = [0; 4];
            output.extend_from_slice(
                char::from_u32(scalar)
                    .unwrap_or('\u{fffd}')
                    .encode_utf8(&mut encoded)
                    .as_bytes(),
            );
            if self.peek() == Some(b'\r') {
                self.index += 1;
                if self.peek() == Some(b'\n') {
                    self.index += 1
                }
            } else if self.peek().is_some_and(space) {
                self.index += 1
            }
        } else {
            output.push(byte);
            self.index += 1
        }
        Some(())
    }
    fn name(&mut self) -> Option<String> {
        let mut output = Vec::new();
        while let Some(byte) = self.peek() {
            if name_char(byte) {
                output.push(byte);
                self.index += 1
            } else if byte == b'\\' {
                self.escape(&mut output)?
            } else {
                break;
            }
        }
        (!output.is_empty()).then(|| String::from_utf8_lossy(&output).into_owned())
    }
    fn identifier(&mut self) -> Option<String> {
        let start = self.index;
        while self.peek() == Some(b'-') {
            self.index += 1
        }
        let prefixes = self.index - start;
        if !self
            .peek()
            .is_some_and(|byte| name_start(byte) || byte == b'\\')
        {
            return None;
        }
        Some("-".repeat(prefixes) + &self.name()?)
    }
    fn string(&mut self) -> Option<String> {
        let quote = self.peek()?;
        self.index += 1;
        let mut output = Vec::new();
        while let Some(byte) = self.peek() {
            if byte == quote {
                self.index += 1;
                return Some(String::from_utf8_lossy(&output).into_owned());
            }
            if matches!(byte, b'\r' | b'\n' | b'\x0c') {
                return None;
            }
            if byte == b'\\' {
                if self
                    .source
                    .get(self.index + 1)
                    .is_some_and(|byte| matches!(byte, b'\r' | b'\n' | b'\x0c'))
                {
                    self.index += 2;
                    if self.source[self.index - 1] == b'\r' && self.peek() == Some(b'\n') {
                        self.index += 1
                    }
                } else {
                    self.escape(&mut output)?
                }
            } else {
                output.push(byte);
                self.index += 1
            }
        }
        None
    }
    fn value(&mut self) -> Option<String> {
        if matches!(self.peek(), Some(b'\'' | b'"')) {
            self.string()
        } else {
            self.identifier()
        }
    }
    fn pattern(&mut self) -> Option<Regex> {
        if self.index + 2 > self.source.len() {
            return None;
        }
        let start = self.index;
        let mut nesting = 0i64;
        while let Some(byte) = self.peek() {
            match byte {
                b'(' | b'[' => nesting += 1,
                b')' | b']' => nesting -= 1,
                _ => {}
            }
            if nesting < 0 {
                return go_regex(std::str::from_utf8(&self.source[start..self.index]).ok()?);
            }
            self.index += 1;
        }
        None
    }
    fn attribute(&mut self) -> Option<usize> {
        self.consume(b'[')?;
        self.whitespace();
        let key = self.identifier()?.to_ascii_lowercase();
        self.whitespace();
        if self.peek() == Some(b']') {
            self.index += 1;
            return Some(self.push(Expression::Attribute {
                key,
                value: String::new(),
                operation: String::new(),
                insensitive: false,
                pattern: None,
            }));
        }
        if self.index + 2 >= self.source.len() {
            return None;
        }
        let operation = if self.peek() == Some(b'=') {
            self.index += 1;
            "=".to_owned()
        } else {
            let first = self.peek()?;
            self.index += 1;
            self.consume(b'=')?;
            String::from_utf8(vec![first, b'=']).ok()?
        };
        if !matches!(
            operation.as_str(),
            "=" | "!=" | "~=" | "|=" | "^=" | "$=" | "*=" | "#="
        ) {
            return None;
        }
        self.whitespace();
        let (value, pattern) = if operation == "#=" {
            (String::new(), Some(self.pattern()?))
        } else {
            (self.value()?, None)
        };
        self.whitespace();
        let insensitive = matches!(self.peek(), Some(b'i' | b'I'));
        if insensitive {
            self.index += 1
        }
        self.whitespace();
        self.consume(b']')?;
        Some(self.push(Expression::Attribute {
            key,
            value,
            operation,
            insensitive,
            pattern,
        }))
    }
    fn integer(&mut self) -> Option<i64> {
        let start = self.index;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.index += 1
        }
        std::str::from_utf8(&self.source[start..self.index])
            .ok()?
            .parse()
            .ok()
    }
    fn nth(&mut self) -> Option<(i64, i64)> {
        if matches!(self.peek(), Some(b'o' | b'O' | b'e' | b'E')) {
            return match self.name()?.to_ascii_lowercase().as_str() {
                "odd" => Some((2, 1)),
                "even" => Some((2, 0)),
                _ => None,
            };
        }
        let sign = if self.peek() == Some(b'-') {
            self.index += 1;
            -1
        } else {
            if self.peek() == Some(b'+') {
                self.index += 1
            }
            1
        };
        let factor = if matches!(self.peek(), Some(b'n' | b'N')) {
            1
        } else {
            self.integer()?
        } * sign;
        if !matches!(self.peek(), Some(b'n' | b'N')) {
            self.peek()?;
            return Some((0, factor));
        }
        self.index += 1;
        self.whitespace();
        let offset_sign = match self.peek()? {
            b'+' => 1,
            b'-' => -1,
            _ => return Some((factor, 0)),
        };
        self.index += 1;
        self.whitespace();
        Some((factor, self.integer()? * offset_sign))
    }
    fn pseudo(&mut self) -> Option<usize> {
        self.consume(b':')?;
        let name = self.identifier()?.to_ascii_lowercase();
        let expression = match name.as_str() {
            "not" | "has" | "haschild" => {
                self.opening()?;
                let group = self.group()?;
                self.closing()?;
                Expression::Relative(name, group)
            }
            "contains" | "containsown" => {
                self.opening()?;
                let value = text::to_lower(&self.value()?);
                self.closing()?;
                Expression::Contains(value, name == "containsown")
            }
            "matches" | "matchesown" => {
                self.opening()?;
                let pattern = self.pattern()?;
                self.closing()?;
                Expression::Matches(pattern, name == "matchesown")
            }
            "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
                self.opening()?;
                let (factor, offset) = self.nth()?;
                self.closing()?;
                Expression::Nth(
                    factor,
                    offset,
                    name.contains("last"),
                    name.ends_with("of-type"),
                )
            }
            "first-child" | "last-child" | "first-of-type" | "last-of-type" => {
                Expression::Nth(0, 1, name.starts_with("last"), name.ends_with("of-type"))
            }
            "only-child" | "only-of-type" => Expression::Only(name == "only-of-type"),
            "lang" => {
                self.opening()?;
                let language = text::to_lower(&self.identifier()?);
                self.closing()?;
                Expression::Language(language)
            }
            "input" | "empty" | "root" | "link" | "enabled" | "disabled" | "checked"
            | "visited" | "hover" | "active" | "focus" | "target" => Expression::Pseudo(name),
            _ => return None,
        };
        Some(self.push(expression))
    }
    fn simple(&mut self) -> Option<usize> {
        let mut parts = Vec::new();
        match self.peek()? {
            b'*' => {
                self.index += 1;
                if self.index + 2 < self.source.len()
                    && self.source[self.index..].starts_with(b"|*")
                {
                    self.index += 2
                }
            }
            b'#' | b'.' | b'[' | b':' => {}
            _ => {
                let tag = self.identifier()?.to_ascii_lowercase();
                parts.push(self.push(Expression::Tag(tag)))
            }
        }
        loop {
            let expression = match self.peek() {
                Some(b'#') => {
                    self.index += 1;
                    let value = self.name()?;
                    self.push(Expression::Attribute {
                        key: "id".into(),
                        value,
                        operation: "=".into(),
                        insensitive: false,
                        pattern: None,
                    })
                }
                Some(b'.') => {
                    self.index += 1;
                    let value = self.identifier()?;
                    self.push(Expression::Attribute {
                        key: "class".into(),
                        value,
                        operation: "~=".into(),
                        insensitive: false,
                        pattern: None,
                    })
                }
                Some(b'[') => self.attribute()?,
                Some(b':') => self.pseudo()?,
                _ => break,
            };
            parts.push(expression);
        }
        Some(if parts.len() == 1 {
            parts[0]
        } else {
            self.push(Expression::All(parts))
        })
    }
    fn selector(&mut self) -> Option<usize> {
        self.whitespace();
        let mut expression = self.simple()?;
        loop {
            let mut combinator = if self.whitespace() { b' ' } else { 0 };
            match self.peek() {
                Some(byte @ (b'+' | b'>' | b'~')) => {
                    combinator = byte;
                    self.index += 1;
                    self.whitespace();
                }
                None | Some(b',' | b')') => return Some(expression),
                _ => {}
            }
            if combinator == 0 {
                return Some(expression);
            }
            let second = self.simple()?;
            expression = self.push(Expression::Combined(expression, combinator, second));
        }
    }
    fn group(&mut self) -> Option<Vec<usize>> {
        let mut group = vec![self.selector()?];
        while self.peek() == Some(b',') {
            self.index += 1;
            group.push(self.selector()?)
        }
        Some(group)
    }
}

pub(crate) fn go_regex(source: &str) -> Option<Regex> {
    crate::regex_go::compile(source)
}

fn equal_fold(first: &str, second: &str) -> bool {
    let mut second = second.chars();
    for character in first.chars() {
        let Some(other) = second.next() else {
            return false;
        };
        if character == other {
            continue;
        }
        let start = character as u32;
        let mut folded = start;
        loop {
            folded = crate::go_unicode::SIMPLE_FOLD
                .binary_search_by_key(&folded, |&(scalar, _)| scalar)
                .ok()
                .map_or(folded, |index| crate::go_unicode::SIMPLE_FOLD[index].1);
            if folded == other as u32 {
                break;
            }
            if folded == start {
                return false;
            }
        }
    }
    second.next().is_none()
}

fn node_text(document: &Document, root: NodeId, own: bool) -> String {
    if own {
        return document.nodes[root]
            .children
            .iter()
            .filter(|&&child| document.nodes[child].kind == Kind::Text)
            .map(|&child| document.nodes[child].data.as_str())
            .collect();
    }
    let mut output = String::new();
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        match document.nodes[index].kind {
            Kind::Text => output.push_str(&document.nodes[index].data),
            Kind::Element => pending.extend(document.nodes[index].children.iter().rev()),
            _ => {}
        }
    }
    output
}

fn disabled_fieldset(document: &Document, mut root: NodeId) -> bool {
    while let Some(parent) = document.nodes[root].parent {
        let previous_legend = document.nodes[parent]
            .children
            .iter()
            .take_while(|&&child| child != root)
            .any(|&child| document.nodes[child].tag == "legend");
        if document.nodes[parent].tag == "fieldset"
            && document.nodes[parent].has_attr("disabled")
            && (document.nodes[root].tag != "legend" || previous_legend)
        {
            return true;
        }
        root = parent;
    }
    false
}

impl Selector {
    pub(crate) fn parse(source: &str) -> Option<Self> {
        let mut parser = Parser {
            source: source.as_bytes(),
            index: 0,
            expressions: Vec::new(),
        };
        let group = parser.group()?;
        (parser.index == source.len()).then_some(Self {
            expressions: parser.expressions,
            group,
        })
    }
    pub(crate) fn matches(&self, document: &Document, root: NodeId) -> bool {
        self.group
            .iter()
            .any(|&index| self.expression(index, document, root))
    }
    fn expression(&self, index: usize, document: &Document, root: NodeId) -> bool {
        let node = &document.nodes[root];
        match &self.expressions[index] {
            Expression::Tag(tag) => node.kind == Kind::Element && node.tag == *tag,
            Expression::All(parts) => {
                if parts.is_empty() {
                    node.kind == Kind::Element
                } else {
                    parts
                        .iter()
                        .all(|&index| self.expression(index, document, root))
                }
            }
            Expression::Attribute {
                key,
                value,
                operation,
                insensitive,
                pattern,
            } => {
                if node.kind != Kind::Element {
                    return false;
                }
                let equals = |first: &str, second: &str| {
                    if *insensitive {
                        equal_fold(first, second)
                    } else {
                        first == second
                    }
                };
                let attributes = || node.attrs.iter().filter(|attribute| attribute.key == *key);
                if operation == "!=" {
                    return attributes().all(|attribute| !equals(&attribute.value, value));
                }
                attributes().any(|attribute| {
                    let actual = attribute.value.as_str();
                    match operation.as_str() {
                        "" => true,
                        "=" => equals(actual, value),
                        "~=" => {
                            let mut remaining = actual;
                            while !remaining.is_empty() {
                                if let Some(position) = remaining.bytes().position(space) {
                                    if equals(&remaining[..position], value) {
                                        return true;
                                    }
                                    remaining = &remaining[position + 1..];
                                } else {
                                    return equals(remaining, value);
                                }
                            }
                            false
                        }
                        "|=" => {
                            equals(actual, value)
                                || (actual.as_bytes().get(value.len()) == Some(&b'-')
                                    && actual
                                        .get(..value.len())
                                        .is_some_and(|prefix| equals(prefix, value)))
                        }
                        "^=" | "$=" | "*=" => {
                            if text::trim_space(actual).is_empty() {
                                return false;
                            }
                            let (actual, value) = if *insensitive {
                                (text::to_lower(actual), text::to_lower(value))
                            } else {
                                (actual.into(), value.clone())
                            };
                            match operation.as_str() {
                                "^=" => actual.starts_with(&value),
                                "$=" => actual.ends_with(&value),
                                _ => actual.contains(&value),
                            }
                        }
                        "#=" => pattern.as_ref().unwrap().is_match(actual),
                        _ => false,
                    }
                })
            }
            Expression::Combined(first, combinator, second) => {
                if !self.expression(*second, document, root) {
                    return false;
                }
                let Some(parent) = node.parent else {
                    return false;
                };
                match combinator {
                    b'>' => self.expression(*first, document, parent),
                    b' ' => {
                        let mut ancestor = Some(parent);
                        while let Some(parent) = ancestor {
                            if self.expression(*first, document, parent) {
                                return true;
                            }
                            ancestor = document.nodes[parent].parent;
                        }
                        false
                    }
                    _ => {
                        let siblings = &document.nodes[parent].children;
                        let position = siblings.iter().position(|&child| child == root).unwrap();
                        for &sibling in siblings[..position].iter().rev() {
                            if *combinator == b'+'
                                && matches!(
                                    document.nodes[sibling].kind,
                                    Kind::Text | Kind::Comment
                                )
                            {
                                continue;
                            }
                            let matched = self.expression(*first, document, sibling);
                            if matched || *combinator == b'+' {
                                return matched;
                            }
                        }
                        false
                    }
                }
            }
            Expression::Relative(name, group) => {
                if node.kind != Kind::Element {
                    return false;
                }
                let matches = |root| {
                    group
                        .iter()
                        .any(|&index| self.expression(index, document, root))
                };
                if name == "not" {
                    return !matches(root);
                }
                let mut pending: Vec<_> = node.children.iter().rev().copied().collect();
                while let Some(child) = pending.pop() {
                    if matches(child) {
                        return true;
                    }
                    if name == "has" && document.nodes[child].kind == Kind::Element {
                        pending.extend(document.nodes[child].children.iter().rev());
                    }
                }
                false
            }
            Expression::Contains(value, own) => {
                text::to_lower(&node_text(document, root, *own)).contains(value)
            }
            Expression::Matches(pattern, own) => pattern.is_match(&node_text(document, root, *own)),
            Expression::Nth(factor, offset, last, of_type) => {
                if node.kind != Kind::Element {
                    return false;
                }
                let Some(parent) = node.parent else {
                    return false;
                };
                let siblings: Vec<_> = document.nodes[parent]
                    .children
                    .iter()
                    .copied()
                    .filter(|&child| {
                        document.nodes[child].kind == Kind::Element
                            && (!of_type || document.nodes[child].tag == node.tag)
                    })
                    .collect();
                let Some(position) = siblings.iter().position(|&child| child == root) else {
                    return false;
                };
                let position = if *last {
                    siblings.len() - position
                } else {
                    position + 1
                } as i64;
                let distance = position.wrapping_sub(*offset);
                if *factor == 0 {
                    distance == 0
                } else {
                    distance.wrapping_rem(*factor) == 0 && distance.wrapping_div(*factor) >= 0
                }
            }
            Expression::Only(of_type) => {
                node.kind == Kind::Element
                    && node.parent.is_some_and(|parent| {
                        document.nodes[parent]
                            .children
                            .iter()
                            .filter(|&&child| {
                                document.nodes[child].kind == Kind::Element
                                    && (!of_type || document.nodes[child].tag == node.tag)
                            })
                            .count()
                            == 1
                    })
            }
            Expression::Language(language) => {
                let mut ancestor = Some(root);
                while let Some(index) = ancestor {
                    let node = &document.nodes[index];
                    if node.kind == Kind::Element
                        && node.attrs.iter().any(|attribute| {
                            attribute.key == "lang"
                                && (attribute.value == *language
                                    || attribute.value.starts_with(&format!("{language}-")))
                        })
                    {
                        return true;
                    }
                    ancestor = node.parent;
                }
                false
            }
            Expression::Pseudo(name) => {
                if node.kind != Kind::Element {
                    return false;
                }
                match name.as_str() {
                    "input" => matches!(
                        node.tag.as_str(),
                        "input" | "select" | "textarea" | "button"
                    ),
                    "empty" => !node.children.iter().any(|&child| {
                        document.nodes[child].kind == Kind::Element
                            || (document.nodes[child].kind == Kind::Text
                                && !text::trim_space(&document.nodes[child].data).is_empty())
                    }),
                    "root" => node
                        .parent
                        .is_some_and(|parent| document.nodes[parent].kind == Kind::Document),
                    "link" => {
                        matches!(node.tag.as_str(), "a" | "area" | "link") && node.has_attr("href")
                    }
                    "enabled" | "disabled" => {
                        let disabled = match node.tag.as_str() {
                            "a" | "area" | "link" => {
                                return name == "enabled" && node.has_attr("href")
                            }
                            "optgroup" | "menuitem" | "fieldset" => node.has_attr("disabled"),
                            "button" | "input" | "select" | "textarea" | "option" => {
                                node.has_attr("disabled") || disabled_fieldset(document, root)
                            }
                            _ => return false,
                        };
                        disabled == (name == "disabled")
                    }
                    "checked" => match node.tag.as_str() {
                        "input" | "menuitem" => {
                            node.has_attr("checked")
                                && node.attrs.iter().any(|attribute| {
                                    attribute.key == "type"
                                        && matches!(
                                            attribute.value.to_ascii_lowercase().as_str(),
                                            "checkbox" | "radio"
                                        )
                                })
                        }
                        "option" => node.has_attr("selected"),
                        _ => false,
                    },
                    _ => false,
                }
            }
        }
    }
    pub(crate) fn prune(&self, document: &mut Document, root: NodeId) {
        let selected: Vec<_> = document
            .elements(root)
            .into_iter()
            .filter(|&element| self.matches(document, element))
            .collect();
        for element in selected.into_iter().rev() {
            etree::remove(document, element, true)
        }
    }
}
