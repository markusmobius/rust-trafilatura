#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExtractionFocus {
    #[default]
    Balanced,
    FavorRecall,
    FavorPrecision,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HtmlDateMode {
    #[default]
    Default,
    Fast,
    Extensive,
    Disabled,
}

#[derive(Clone, Default)]
pub struct DateOptions {
    pub enable_fallback: bool,
    pub html_date_mode: HtmlDateMode,
    pub html_date_options: Option<rust_htmldate::Options>,
    pub html_date_override: Option<rust_htmldate::ExtractionResult>,
}

impl DateOptions {
    pub fn resolved_options(&self, metadata_url: &str) -> Option<rust_htmldate::Options> {
        if self.html_date_override.is_some() {
            return None;
        }
        let mut options = if let Some(custom) = &self.html_date_options {
            custom.clone()
        } else {
            let skip_extensive_search = match self.html_date_mode {
                HtmlDateMode::Default => !self.enable_fallback,
                HtmlDateMode::Fast => true,
                HtmlDateMode::Extensive => false,
                HtmlDateMode::Disabled => return None,
            };
            rust_htmldate::Options {
                use_original_date: true,
                skip_extensive_search,
                ..Default::default()
            }
        };
        options.url = metadata_url.into();
        Some(options)
    }
}

#[derive(Clone, Default)]
pub struct Options {
    pub config: Option<Config>,
    pub original_url: Option<crate::Url>,
    pub input_encoding: String,
    pub target_language: String,
    pub enable_fallback: bool,
    pub fallback_candidates: Option<FallbackCandidates>,
    pub focus: ExtractionFocus,
    pub exclude_comments: bool,
    pub exclude_tables: bool,
    pub include_images: bool,
    pub include_links: bool,
    pub blacklisted_authors: Vec<String>,
    pub deduplicate: bool,
    pub has_essential_metadata: bool,
    pub max_tree_size: i64,
    pub enable_log: bool,
    pub html_date_mode: HtmlDateMode,
    pub html_date_options: Option<rust_htmldate::Options>,
    pub html_date_override: Option<rust_htmldate::ExtractionResult>,
    pub prune_selector: String,
}

impl Options {
    pub(crate) fn log(&self, message: std::fmt::Arguments<'_>) {
        if self.enable_log {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "trafilatura: {message}");
        }
    }
}

impl From<&Options> for DateOptions {
    fn from(options: &Options) -> Self {
        Self {
            enable_fallback: options.enable_fallback,
            html_date_mode: options.html_date_mode,
            html_date_options: options.html_date_options.clone(),
            html_date_override: options.html_date_override.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub document: crate::Document,
    pub root: crate::NodeId,
}

#[derive(Clone, Debug, Default)]
pub struct FallbackCandidates {
    pub readability: Option<Tree>,
    pub distiller: Option<Tree>,
    pub others: Vec<Tree>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub cache_size: i64,
    pub max_duplicate_count: i64,
    pub min_duplicate_check_size: i64,
    pub min_extracted_size: i64,
    pub min_extracted_comment_size: i64,
    pub min_output_size: i64,
    pub min_output_comment_size: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_size: 4096,
            max_duplicate_count: 2,
            min_duplicate_check_size: 100,
            min_extracted_size: 250,
            min_extracted_comment_size: 1,
            min_output_size: 1,
            min_output_comment_size: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_go() {
        assert_eq!(
            Config::default(),
            Config {
                cache_size: 4096,
                max_duplicate_count: 2,
                min_duplicate_check_size: 100,
                min_extracted_size: 250,
                min_extracted_comment_size: 1,
                min_output_size: 1,
                min_output_comment_size: 1,
            }
        );
        assert_eq!(ExtractionFocus::default(), ExtractionFocus::Balanced);
        assert_eq!(HtmlDateMode::default(), HtmlDateMode::Default);
    }
}
