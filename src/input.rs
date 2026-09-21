use crate::{core::extract_tree, encoding, parse_html, Error, ExtractResult, Options, Tree};
use std::io::Read;

pub fn extract(mut reader: impl Read, options: &Options) -> Result<ExtractResult, Error> {
    if !options.input_encoding.is_empty()
        && encoding_rs::Encoding::for_label(options.input_encoding.as_bytes()).is_none()
    {
        return Err(Error::UnsupportedCharset(options.input_encoding.clone()));
    }
    let mut input = Vec::new();
    reader.read_to_end(&mut input)?;
    if input.starts_with(&[0x1f, 0x8b]) {
        let mut decoded = Vec::new();
        flate2::read::MultiGzDecoder::new(input.as_slice()).read_to_end(&mut decoded)?;
        input = decoded;
    }
    let source = if options.input_encoding.is_empty() {
        encoding::decode(&input)?
    } else {
        encoding::decode_as(&input, &options.input_encoding)
    };
    let document = parse_html(&source);
    extract_tree(Tree { document, root: 0 }, options)
}
