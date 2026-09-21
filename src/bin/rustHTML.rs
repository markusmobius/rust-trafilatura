use rust_trafilatura::{parse_html, text, Document, Kind, NodeId};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{json, value::RawValue, Value};
use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "../json_input.rs"]
mod json_input;

fn decode<Output: serde::de::DeserializeOwned>(source: &str) -> Option<Output> {
    let source = json_input::prepare(source)?;
    let mut decoder = serde_json::Deserializer::from_str(&source);
    decoder.disable_recursion_limit();
    let result = Output::deserialize(&mut decoder).ok()?;
    decoder.end().ok()?;
    Some(result)
}

#[derive(Default)]
struct TaskInput {
    html_path: String,
    html: String,
    url: String,
    run_readability: bool,
    run_distiller: bool,
    run_trafilatura: bool,
    run_meta: bool,
    run_date: bool,
    verbose: bool,
}

struct OrderedFields(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for OrderedFields {
    fn deserialize<Decoder: serde::Deserializer<'de>>(
        decoder: Decoder,
    ) -> Result<Self, Decoder::Error> {
        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = OrderedFields;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a worker request object")
            }
            fn visit_unit<Error: serde::de::Error>(self) -> Result<OrderedFields, Error> {
                Ok(OrderedFields(Vec::new()))
            }
            fn visit_map<Fields: MapAccess<'de>>(
                self,
                mut fields: Fields,
            ) -> Result<OrderedFields, Fields::Error> {
                let mut ordered = Vec::new();
                while let Some((key, value)) = fields.next_entry::<String, Box<RawValue>>()? {
                    let key = key
                        .replace('\u{17f}', "s")
                        .replace('\u{212a}', "k")
                        .to_ascii_lowercase();
                    ordered.push((key, value));
                }
                Ok(OrderedFields(ordered))
            }
        }
        decoder.deserialize_any(FieldVisitor)
    }
}

impl<'de> Deserialize<'de> for TaskInput {
    fn deserialize<Decoder: serde::Deserializer<'de>>(
        decoder: Decoder,
    ) -> Result<Self, Decoder::Error> {
        let mut task = TaskInput::default();
        for (key, value) in OrderedFields::deserialize(decoder)?.0 {
            match key.as_str() {
                "htmlpath" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.html_path = value;
                    }
                }
                "html" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.html = value;
                    }
                }
                "url" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.url = value;
                    }
                }
                "runreadability" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.run_readability = value;
                    }
                }
                "rundistiller" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.run_distiller = value;
                    }
                }
                "runtrafilatura" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.run_trafilatura = value;
                    }
                }
                "runmeta" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.run_meta = value;
                    }
                }
                "rundate" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.run_date = value;
                    }
                }
                "verbose" => {
                    if let Ok(value) = serde_json::from_str(value.get()) {
                        task.verbose = value;
                    }
                }
                _ => {}
            }
        }
        Ok(task)
    }
}

#[derive(Default, Debug, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct ProcessedContent {
    text: String,
    title: String,
    raw: String,
    urls: Option<Vec<String>>,
}

fn postprocess(document: &Document, root: NodeId) -> ProcessedContent {
    let mut output = ProcessedContent::default();
    let mut pending = vec![(root, false, false)];
    while let Some((current, leaving, close_marker)) = pending.pop() {
        let node = &document.nodes[current];
        if leaving {
            if close_marker {
                output.text.push_str("__marker:END__");
            }
            if node.kind == Kind::Element
                && !matches!(
                    node.tag.as_str(),
                    "a" | "span"
                        | "b"
                        | "strong"
                        | "i"
                        | "em"
                        | "mark"
                        | "small"
                        | "del"
                        | "ins"
                        | "sub"
                        | "sup"
                )
            {
                output.text.push('\n');
            }
            continue;
        }
        let mut close_marker = false;
        if node.kind == Kind::Text {
            output.text.push_str(&node.data.replace('\n', " "));
        } else if node.kind == Kind::Element {
            let key = match node.tag.as_str() {
                "a" => Some("href"),
                "img" | "video" => Some("src"),
                _ => None,
            };
            if let Some(key) = key {
                for attribute in node.attrs.iter().filter(|attribute| attribute.key == key) {
                    let urls = output.urls.get_or_insert_default();
                    output
                        .text
                        .push_str(&format!("__marker:{}:{}__", node.tag, urls.len()));
                    urls.push(attribute.value.clone());
                    close_marker |= node.tag == "a";
                }
            }
            if matches!(
                node.tag.as_str(),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "strong" | "b" | "i" | "em"
            ) {
                output
                    .text
                    .push_str(&format!("__marker:format:{}__", node.tag));
                close_marker = true;
            }
            if matches!(node.tag.as_str(), "figure" | "picture") {
                output.text.push_str("__marker:figure__");
                close_marker = true;
            }
        }
        pending.push((current, true, close_marker));
        pending.extend(
            node.children
                .iter()
                .rev()
                .map(|&child| (child, false, false)),
        );
    }
    output.text = text::unescape_html(&output.text);
    output
}

fn metadata_value(metadata: &rust_trafilatura::Metadata) -> Value {
    json!({
        "Title": metadata.title, "Author": metadata.author, "URL": metadata.url,
        "Hostname": metadata.hostname, "Description": metadata.description,
        "Sitename": metadata.sitename,
        "Date": metadata.date.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
        "Categories": metadata.categories, "Tags": metadata.tags, "ID": metadata.id,
        "Fingerprint": metadata.fingerprint, "License": metadata.license,
        "Language": metadata.language, "Image": metadata.image, "PageType": metadata.page_type,
    })
}

fn empty_output() -> Value {
    let processed = json!(ProcessedContent::default());
    let date = json!({"Date": "0001-01-01T00:00:00Z", "HasTime": false, "HasTimezone": false});
    let mut metadata = metadata_value(&rust_trafilatura::Metadata::default());
    metadata["Categories"] = Value::Null;
    metadata["Tags"] = Value::Null;
    json!({
        "Readability": {"Processed": processed, "Meta": {"Byline": "", "Excerpt": "", "SiteName": "", "Image": "", "Favicon": ""}},
        "DomDistiller": {
            "Processed": processed,
            "Meta": {"Title": "", "Type": "", "URL": "", "Description": "", "Publisher": "", "Copyright": "", "Author": "",
                "Article": {"PublishedTime": "", "ModifiedTime": "", "ExpirationTime": "", "Section": "", "Authors": null}, "Images": null},
            "Pagination": {"NextPage": "", "PrevPage": ""}
        },
        "Trafilatura": {"Processed": processed, "Meta": metadata},
        "Meta": {"Title": "", "MetaTags": null, "HeadCanonicalURL": "", "JSONLdScripts": null},
        "HtmlDate": {"PublishDate": date, "LastDate": date},
    })
}

fn go_json(value: &Value) -> io::Result<Vec<u8>> {
    Ok(serde_json::to_string(value)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .into_bytes())
}

fn process(command: &str) -> io::Result<Vec<u8>> {
    let task: TaskInput = decode(command).unwrap_or_default();
    if task.run_readability || task.run_distiller || task.run_meta || task.run_date {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "only RunTrafilatura is supported by this worker",
        ));
    }
    let mut output = empty_output();
    if !task.run_trafilatura {
        return go_json(&output);
    }
    let source = if task.html_path.is_empty() {
        task.html
    } else {
        let Ok(mut bytes) = std::fs::read(task.html_path) else {
            return go_json(&output);
        };
        if bytes.starts_with(b"||XOR||") {
            bytes.drain(..7);
            for byte in &mut bytes {
                *byte ^= 255
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    };
    let Some(url) = rust_trafilatura::Url::request(&task.url) else {
        return go_json(&output);
    };
    let options = rust_trafilatura::Options {
        original_url: Some(url),
        exclude_comments: true,
        include_images: true,
        include_links: true,
        enable_fallback: true,
        ..Default::default()
    };
    if let Ok(result) = rust_trafilatura::extract_document(&parse_html(&source), &options) {
        let raw = result
            .content_node
            .document
            .outer_html(result.content_node.root);
        let rendered = parse_html(&format!("<html><body>{raw}</body></html>"));
        let mut processed = postprocess(&rendered, rendered.tagged(0, "body")[0]);
        processed.title = text::unescape_html(&result.metadata.title);
        if task.verbose {
            processed.raw = raw
        }
        output["Trafilatura"] =
            json!({"Processed": processed, "Meta": metadata_value(&result.metadata)});
    }
    go_json(&output)
}

fn file_loop(input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    writeln!(output, "ready")?;
    output.flush()?;
    for line in input.lines() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 2 {
            break;
        }
        let result = process(fields[0]).and_then(|mut result| {
            for byte in &mut result {
                *byte ^= 255
            }
            std::fs::write(text::trim_space(fields[1]), result)
        });
        writeln!(output, "{}", if result.is_ok() { "ok" } else { "error" })?;
        output.flush()?;
    }
    Ok(())
}

fn read_frame(mut input: impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 8];
    loop {
        match input.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    input.read_exact(&mut header[1..])?;
    let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
    let chunk_size = u32::from_be_bytes(header[4..].try_into().unwrap());
    if chunk_size == 0 && length != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "TCP chunk size is zero",
        ));
    }
    let mut body = Vec::new();
    if input.take(length).read_to_end(&mut body)? as u64 != length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete TCP frame",
        ));
    }
    Ok(Some(body))
}

fn send_frame(mut output: impl Write, message: &Value) -> io::Result<()> {
    let content = go_json(message)?;
    let length = u32::try_from(content.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    output.write_all(&length.to_be_bytes())?;
    output.write_all(&(1024u32 * 1024).to_be_bytes())?;
    for chunk in content.chunks(1024 * 1024) {
        output.write_all(chunk)?
    }
    output.flush()
}

fn tcp_loop(port: &str, guid: &str) -> io::Result<()> {
    let mut connection = TcpStream::connect(format!("localhost:{port}"))?;
    send_frame(
        &mut connection,
        &json!({"MType": 0, "GUID": guid, "Content": ""}),
    )?;
    let mut request_guid = String::new();
    let mut command = String::new();
    while let Some(message) = read_frame(&mut connection)? {
        let Some(fields) = decode::<OrderedFields>(&String::from_utf8_lossy(&message)) else {
            break;
        };
        let mut valid = true;
        for (key, value) in fields.0 {
            let target = match key.as_str() {
                "guid" => &mut request_guid,
                "command" => &mut command,
                _ => continue,
            };
            if let Ok(value) = serde_json::from_str::<String>(value.get()) {
                *target = value;
            } else if value.get() != "null" {
                valid = false;
            }
        }
        if !valid {
            break;
        }
        let content = String::from_utf8(process(&command)?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        send_frame(
            &mut connection,
            &json!({"MType": 1, "GUID": request_guid, "Content": content}),
        )?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments.as_slice() {
        [] => file_loop(io::stdin().lock(), io::stdout().lock()),
        [port, _parent_id, guid] => tcp_loop(port, guid),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected no arguments or port parent-id readiness-guid",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_postprocessor() {
        let document = parse_html("<body><h2>Title</h2><p>A\n<a href=\"https://example.org/?a=1&amp;b=2\">link<strong>bold</strong></a>&amp;amp;<br>end</p><figure><img src=\"photo.jpg\"><video src=\"clip.mp4\"></video></figure></body>");
        let result = postprocess(&document, document.tagged(0, "body")[0]);
        assert_eq!(result.text, "__marker:format:h2__Title__marker:END__\nA __marker:a:0__link__marker:format:strong__bold__marker:END____marker:END__&\nend\n__marker:figure____marker:img:1__\n__marker:video:2__\n__marker:END__\n\n");
        assert_eq!(
            result.urls,
            Some(vec![
                "https://example.org/?a=1&b=2".into(),
                "photo.jpg".into(),
                "clip.mp4".into()
            ])
        );
        let empty = parse_html("<p>text</p>");
        assert_eq!(postprocess(&empty, empty.tagged(0, "body")[0]).urls, None);
    }

    #[test]
    fn worker_schema_and_errors() {
        let empty: Value = serde_json::from_slice(&process("{}").unwrap()).unwrap();
        assert_eq!(empty.as_object().unwrap().len(), 5);
        assert_eq!(empty["Trafilatura"]["Meta"]["Date"], "0001-01-01T00:00:00Z");
        assert!(empty["Trafilatura"]["Meta"]["Categories"].is_null());
        assert!(empty["Readability"]["Processed"]["Urls"].is_null());
        assert_eq!(process("invalid JSON").unwrap(), process("{}").unwrap());
        assert!(process(r#"{"RunMeta":true}"#).is_err());
        assert_eq!(
            process(r#"{"RunTrafilatura":true,"URL":"invalid"}"#).unwrap(),
            process("{}").unwrap()
        );
        let html = "<html><head><title>Example &amp;amp; title</title></head><body><article><p>Example article with <a href='/news'>a link</a> and sufficient text for extraction.</p></article></body></html>";
        let command = json!({"HTML": html, "URL": "https://example.org/", "RunTrafilatura": true, "Verbose": true}).to_string();
        let result: Value = serde_json::from_slice(&process(&command).unwrap()).unwrap();
        assert_eq!(
            result["Trafilatura"]["Processed"]["Title"],
            "Example & title"
        );
        assert!(result["Trafilatura"]["Processed"]["Text"]
            .as_str()
            .unwrap()
            .contains("Example article"));
        assert!(result["Trafilatura"]["Processed"]["Raw"]
            .as_str()
            .unwrap()
            .starts_with("<body>"));
    }

    #[test]
    fn partial_and_ordered_request_fields() {
        let task = decode::<TaskInput>(
            r#"{"HTML":"before\ud800after","RunTrafilatura":true,"unknown":1e10000,"URL":1e10000}"#,
        )
        .unwrap();
        assert_eq!(task.html, "before\u{fffd}after");
        assert!(task.run_trafilatura);
        assert!(task.url.is_empty());
        let command = format!(
            "{{\"unknown\":{}0{},\"RunTrafilatura\":true}}",
            "[".repeat(9_999),
            "]".repeat(9_999)
        );
        assert!(decode::<TaskInput>(&command).unwrap().run_trafilatura);
        let command = format!(
            "{{\"unknown\":{}0{}}}",
            "[".repeat(10_000),
            "]".repeat(10_000)
        );
        assert!(decode::<TaskInput>(&command).is_none());
        let task: TaskInput = serde_json::from_str(r#"{"HTML":"retained","URL":123,"RunTrafilatura":true,"Verbose":"wrong","HTMLPath":null}"#).unwrap();
        assert_eq!(task.html, "retained");
        assert!(task.url.is_empty());
        assert!(task.run_trafilatura);
        assert!(!task.verbose);
        let task: TaskInput = serde_json::from_str(r#"{"html":"first","HTML":"second","html":false,"Html":null,"rUNtRAFILATURA":true,"RunTrafilatura":false}"#).unwrap();
        assert_eq!(task.html, "second");
        assert!(!task.run_trafilatura);
        assert_eq!(
            process(r#"{"RunTrafilatura":true,"HTML":"incomplete""#).unwrap(),
            process("{}").unwrap()
        );
    }

    #[test]
    fn tcp_frames_and_file_protocol() {
        struct ShortIo(std::io::Cursor<Vec<u8>>);
        impl Read for ShortIo {
            fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
                let count = output.len().min(3);
                self.0.read(&mut output[..count])
            }
        }
        impl Write for ShortIo {
            fn write(&mut self, input: &[u8]) -> io::Result<usize> {
                self.0.write(&input[..input.len().min(3)])
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let message = json!({"MType": 1, "GUID": "sample", "Content": "Caf\u{e9} < & \u{2028}"});
        let mut bytes = ShortIo(std::io::Cursor::new(Vec::new()));
        send_frame(&mut bytes, &message).unwrap();
        send_frame(&mut bytes, &message).unwrap();
        bytes.0.set_position(0);
        for _ in 0..2 {
            let frame = read_frame(&mut bytes).unwrap().unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&frame).unwrap(), message);
        }
        assert!(read_frame(&mut bytes).unwrap().is_none());
        assert!(read_frame([0, 0, 0, 4, 0, 0, 0, 1, 2].as_slice()).is_err());
        assert!(read_frame([0, 0, 0, 4, 0, 0, 0, 0].as_slice()).is_err());
        let mut output = Vec::new();
        file_loop("{}\t\ninvalid\n".as_bytes(), &mut output).unwrap();
        assert_eq!(output, b"ready\nerror\n");
    }
}
