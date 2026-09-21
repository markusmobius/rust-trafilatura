use crate::Url;
use std::fmt::Write;

fn absolute_url(value: &str) -> Option<Url> {
    Url::request(value).filter(|url| matches!(url.scheme.as_str(), "http" | "https"))
}

pub fn is_absolute_url(value: &str) -> bool {
    absolute_url(value).is_some()
}

pub fn get_domain_url(value: &str) -> String {
    absolute_url(value).map_or_else(String::new, |url| url.hostname().into_owned())
}

pub fn get_base_url(value: &str) -> String {
    absolute_url(value).map_or_else(String::new, |url| {
        format!("{}://{}", url.scheme, url.hostname())
    })
}

fn join_path(base: &[u8], reference: &str) -> Vec<u8> {
    let mut joined = base.to_vec();
    if !joined.is_empty() {
        joined.push(b'/');
    }
    joined.extend_from_slice(reference.as_bytes());
    let rooted = joined.starts_with(b"/");
    let mut segments: Vec<&[u8]> = Vec::new();
    for segment in joined.split(|&byte| byte == b'/') {
        match segment {
            b"" | b"." => {}
            b".." => {
                if segments.last().is_some_and(|last| *last != b"..") {
                    segments.pop();
                } else if !rooted {
                    segments.push(segment);
                }
            }
            _ => segments.push(segment),
        }
    }
    let mut result = Vec::new();
    if rooted {
        result.push(b'/');
    }
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            result.push(b'/');
        }
        result.extend_from_slice(segment);
    }
    if result.is_empty() {
        result.push(b'.');
    }
    result
}

pub fn create_absolute_url(value: &str, base: Option<&Url>) -> String {
    let Some(base) = base else {
        return value.into();
    };
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with("data:")
        || value.starts_with("javascript:")
        || Url::request(value)
            .is_some_and(|url| !url.scheme.is_empty() && !url.hostname().is_empty())
    {
        return value.into();
    }
    let normalized = if value.starts_with('/') {
        value.as_bytes().to_vec()
    } else {
        join_path(&base.path, value)
    };
    let mut parse_input = String::new();
    for chunk in normalized.utf8_chunks() {
        parse_input.push_str(chunk.valid());
        for byte in chunk.invalid() {
            write!(parse_input, "%{byte:02X}").unwrap();
        }
    }
    Url::parse(&parse_input).map_or_else(
        || String::from_utf8_lossy(&normalized).into_owned(),
        |mut reference| {
            let path_end = normalized
                .iter()
                .position(|byte| matches!(byte, b'?' | b'#'))
                .unwrap_or(normalized.len());
            if std::str::from_utf8(&normalized[..path_end]).is_err() {
                reference.raw_path.clear();
            }
            base.resolve(reference).to_string()
        },
    )
}

pub fn validate_url(value: &str, base: Option<&Url>) -> (String, bool) {
    if is_absolute_url(value) {
        return (value.into(), true);
    }
    let resolved = create_absolute_url(value, base);
    if is_absolute_url(&resolved) {
        (resolved, true)
    } else {
        (value.into(), false)
    }
}
