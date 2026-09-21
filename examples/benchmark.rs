use rust_trafilatura::{
    extract_document, parse_html, Document, ExtractionFocus, Kind, NodeId, Options, Url,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::time::Instant;

#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Deserialize)]
struct Page {
    file: String,
    url: String,
    html: String,
    with: Vec<String>,
    without: Vec<String>,
}

#[derive(Default, Debug, PartialEq, Serialize)]
struct Counts {
    true_positives: usize,
    false_negatives: usize,
    false_positives: usize,
    true_negatives: usize,
}

impl Counts {
    fn evaluate(&mut self, content: &str, page: &Page) {
        for snippet in &page.with {
            if !content.is_empty() && content.contains(snippet) {
                self.true_positives += 1
            } else {
                self.false_negatives += 1
            }
        }
        for snippet in &page.without {
            if !content.is_empty() && content.contains(snippet) {
                self.false_positives += 1
            } else {
                self.true_negatives += 1
            }
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Request {
    fallback: bool,
    outputs: bool,
    input_tree: bool,
    focus: String,
    comments: bool,
    images: bool,
    links: bool,
    exclude_tables: bool,
    deduplicate: bool,
    target_language: String,
}

#[derive(Deserialize)]
struct PageRequest {
    id: String,
    url: String,
    html_path: String,
}

fn page_prediction(request: PageRequest, options: &Options, native: bool) -> Value {
    let mut output = json!({
        "id": request.id, "text": "", "comments": "",
        "metadata": {"title": "", "authors": null, "date": "", "language": ""},
    });
    let options = Options {
        original_url: Url::request(&request.url),
        ..options.clone()
    };
    let result = std::fs::File::open(&request.html_path)
        .map_err(rust_trafilatura::Error::from)
        .and_then(|file| rust_trafilatura::extract(file, &options));
    if native {
        return match result {
            Ok(result) => json!({
                "file": request.id, "text": result.content_text, "comments": result.comments_text,
                "selected_text": result.content_node.document.text(result.content_node.root),
                "selected_comments": result.comments_node.as_ref().map(|body| body.document.text(body.root)).unwrap_or_default(),
                "html": result.content_node.document.outer_html(result.content_node.root),
                "comments_html": result.comments_node.as_ref().map(|body| body.document.outer_html(body.root)).unwrap_or_default(),
                "metadata": metadata_value(&result.metadata), "error": "",
            }),
            Err(error) => {
                json!({"file": request.id, "text": "", "comments": "", "selected_text": "", "selected_comments": "", "html": "", "comments_html": "", "metadata": null, "error": error.to_string()})
            }
        };
    }
    match result {
        Ok(result) => {
            let authors: Vec<_> = result
                .metadata
                .author
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .collect();
            let date = if result.metadata.date == rust_trafilatura::Metadata::default().date {
                String::new()
            } else {
                result.metadata.date.format("%Y-%m-%d").to_string()
            };
            output["text"] = json!(result.content_text);
            output["comments"] = json!(result.comments_text);
            output["metadata"] = json!({
                "title": result.metadata.title,
                "authors": if authors.is_empty() { Value::Null } else { json!(authors) },
                "date": date,
                "language": result.metadata.language,
            });
        }
        Err(error) => output["error"] = json!(error.to_string()),
    }
    output
}

fn run_page_requests(
    reader: impl BufRead,
    mut writer: impl Write,
    options: &Options,
    native: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for line in reader.lines() {
        let request: PageRequest = serde_json::from_str(&line?)?;
        if request.id.is_empty() || request.html_path.is_empty() {
            return Err("each input requires id and html_path".into());
        }
        serde_json::to_writer(&mut writer, &page_prediction(request, options, native))?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

fn run_jsonl() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = Options {
        exclude_comments: true,
        ..Default::default()
    };
    let mut native = false;
    let mut arguments = std::env::args().skip(2);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--native-output" => native = true,
            "--fallback" => options.enable_fallback = true,
            "--comments" => options.exclude_comments = false,
            "--focus" => {
                options.focus = match arguments.next().as_deref() {
                    Some("balanced") => ExtractionFocus::Balanced,
                    Some("precision") => ExtractionFocus::FavorPrecision,
                    Some("recall") => ExtractionFocus::FavorRecall,
                    _ => return Err("--focus requires balanced, precision, or recall".into()),
                };
            }
            _ => return Err(format!("unknown JSONL argument: {argument}").into()),
        }
    }
    run_page_requests(
        io::stdin().lock(),
        io::BufWriter::new(io::stdout().lock()),
        &options,
        native,
    )
}

fn metadata_value(metadata: &rust_trafilatura::Metadata) -> Value {
    json!({
        "Title": metadata.title, "Author": metadata.author, "URL": metadata.url,
        "Hostname": metadata.hostname, "Description": metadata.description, "Sitename": metadata.sitename,
        "Date": metadata.date.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
        "Categories": metadata.categories, "Tags": metadata.tags, "ID": metadata.id,
        "Fingerprint": metadata.fingerprint, "License": metadata.license,
        "Language": metadata.language, "Image": metadata.image, "PageType": metadata.page_type,
    })
}

fn input_snapshot(document: &Document, root: NodeId) -> Value {
    let node = &document.nodes[root];
    match node.kind {
        Kind::Document => return input_snapshot(document, document.tagged(root, "html")[0]),
        Kind::Comment => return json!({"comment": node.data}),
        Kind::Text => return json!(node.data),
        _ => {}
    }
    let attributes: Vec<_> = node
        .attrs
        .iter()
        .map(|attribute| (&attribute.key, &attribute.value))
        .collect();
    let children: Vec<_> = node
        .children
        .iter()
        .filter(|&&child| {
            matches!(
                document.nodes[child].kind,
                Kind::Element | Kind::Text | Kind::Comment
            )
        })
        .map(|&child| input_snapshot(document, child))
        .collect();
    json!({"tag": node.tag, "attributes": attributes, "children": children})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: benchmark CORPUS_JSON | --jsonl [--fallback] [--comments] [--focus MODE]")?;
    if path == "--jsonl" {
        return run_jsonl();
    }
    let pages: Vec<Page> = serde_json::from_reader(io::BufReader::new(std::fs::File::open(path)?))?;
    let documents: Vec<Document> = pages.iter().map(|page| parse_html(&page.html)).collect();
    let mut writer = io::BufWriter::new(io::stdout().lock());
    writeln!(writer, "{{\"ready\":{}}}", pages.len())?;
    writer.flush()?;
    for line in io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        let focus = match request.focus.as_str() {
            "" | "balanced" => ExtractionFocus::Balanced,
            "precision" => ExtractionFocus::FavorPrecision,
            "recall" => ExtractionFocus::FavorRecall,
            other => return Err(format!("unknown focus: {other}").into()),
        };
        let configured: Vec<_> = pages
            .iter()
            .map(|page| Options {
                original_url: Url::request(&page.url),
                enable_fallback: request.fallback,
                focus,
                exclude_comments: !request.comments,
                exclude_tables: request.exclude_tables,
                include_images: request.images,
                include_links: request.links,
                deduplicate: request.deduplicate,
                target_language: request.target_language.clone(),
                ..Default::default()
            })
            .collect();
        let mut counts = Counts::default();
        let mut errors = Vec::new();
        let mut outputs = Vec::new();
        let started = Instant::now();
        for ((page, document), options) in pages.iter().zip(&documents).zip(&configured) {
            match extract_document(document, options) {
                Ok(result) => {
                    counts.evaluate(&result.content_text, page);
                    if request.outputs {
                        let mut output = json!({
                            "file": page.file, "text": result.content_text, "comments": result.comments_text,
                            "selected_text": result.content_node.document.text(result.content_node.root),
                            "selected_comments": result.comments_node.as_ref().map(|body| body.document.text(body.root)).unwrap_or_default(),
                            "html": result.content_node.document.outer_html(result.content_node.root),
                            "comments_html": result.comments_node.as_ref().map(|body| body.document.outer_html(body.root)).unwrap_or_default(),
                            "metadata": metadata_value(&result.metadata), "error": "",
                        });
                        if request.input_tree {
                            output["input_tree"] = input_snapshot(document, 0)
                        }
                        outputs.push(output);
                    }
                }
                Err(error) => {
                    counts.evaluate("", page);
                    errors.push(json!({"file": page.file, "error": error.to_string()}));
                    if request.outputs {
                        let mut output = json!({"file": page.file, "text": "", "comments": "", "selected_text": "", "selected_comments": "", "html": "", "comments_html": "", "metadata": null, "error": error.to_string()});
                        if request.input_tree {
                            output["input_tree"] = input_snapshot(document, 0)
                        }
                        outputs.push(output);
                    }
                }
            }
        }
        let response = json!({"elapsed_ns": started.elapsed().as_nanos(), "counts": counts, "errors": errors, "outputs": outputs});
        serde_json::to_writer(&mut writer, &response)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_flushes_each_response() {
        #[derive(Default)]
        struct Output {
            bytes: Vec<u8>,
            flushes: usize,
        }
        impl Write for Output {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        let path = std::env::temp_dir().join(format!(
            "rust-trafilatura-jsonl-{}.html",
            std::process::id()
        ));
        std::fs::write(&path, "<article><p>This article explains a scientific experiment and its results for the interested reader.</p></article>").unwrap();
        let mut input = String::new();
        for id in ["page-one", "page-two"] {
            input.push_str(
                &json!({"id": id, "url": "https://example.org/article", "html_path": path})
                    .to_string(),
            );
            input.push('\n');
        }
        let mut output = Output::default();
        let options = Options {
            exclude_comments: true,
            html_date_mode: rust_trafilatura::HtmlDateMode::Disabled,
            ..Default::default()
        };
        run_page_requests(io::Cursor::new(&input), &mut output, &options, false).unwrap();
        let mut native_output = Vec::new();
        run_page_requests(io::Cursor::new(&input), &mut native_output, &options, true).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(output.flushes, 2);
        let predictions: Vec<Value> = String::from_utf8(output.bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(predictions[0]["id"], "page-one");
        assert_eq!(predictions[1]["id"], "page-two");
        for prediction in predictions {
            assert!(prediction.get("error").is_none(), "{prediction}");
            assert_eq!(prediction["metadata"]["language"], "en");
            assert!(prediction["text"]
                .as_str()
                .unwrap()
                .contains("scientific experiment"));
        }
        for line in String::from_utf8(native_output).unwrap().lines() {
            let prediction: Value = serde_json::from_str(line).unwrap();
            assert_eq!(prediction["error"], "");
            assert_eq!(prediction["metadata"]["Language"], "en");
            assert!(prediction["html"].as_str().unwrap().contains("<p>"));
        }
    }

    #[test]
    fn duplicate_labels_and_empty_errors() {
        let page = Page {
            file: String::new(),
            url: String::new(),
            html: String::new(),
            with: vec!["article".into(), "missing".into(), "article".into()],
            without: vec!["navigation".into(), "footer".into()],
        };
        let mut counts = Counts::default();
        counts.evaluate("article navigation", &page);
        assert_eq!(
            counts,
            Counts {
                true_positives: 2,
                false_negatives: 1,
                false_positives: 1,
                true_negatives: 1
            }
        );
        counts.evaluate("", &page);
        assert_eq!(counts.false_negatives, 4);
        assert_eq!(counts.true_negatives, 3);
    }
}
