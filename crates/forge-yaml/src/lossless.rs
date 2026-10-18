use thiserror::Error;

#[derive(Error, Debug)]
pub enum LosslessError {
    #[error("Path not found in YAML document: {0}")]
    PathNotFound(String),

    #[error("Root is not a block mapping")]
    NotAMapping,

    #[error("CST error: {0}")]
    CstError(String),
}

/// A lossless YAML document wrapper that preserves comments, whitespace,
/// line endings (CRLF / LF), and UTF-8 Byte Order Marks (BOM) byte-for-byte.
pub struct LosslessYamlDoc {
    has_bom: bool,
    inner: glaucus_cst::Document,
}

impl LosslessYamlDoc {
    /// Parses a YAML document, detecting and preserving any leading UTF-8 BOM.
    pub fn parse(src: &str) -> Self {
        let has_bom = src.starts_with('\u{FEFF}');
        let clean_src = if has_bom {
            &src['\u{FEFF}'.len_utf8()..]
        } else {
            src
        };
        let inner = glaucus_cst::Document::parse(clean_src);
        Self { has_bom, inner }
    }

    /// Looks up the value at a dotted path (e.g. "status", "metadata.component").
    pub fn get(&self, path: &str) -> Option<&str> {
        self.inner.get(path)
    }

    /// Slices and updates the exact scalar at `path`, preserving all surrounding
    /// comments, formatting, indentation, CRLF/LF line endings, and BOM.
    pub fn set(&mut self, path: &str, value: &str) -> Result<(), LosslessError> {
        self.inner.set(path, value).map_err(|e| match e {
            glaucus_cst::SetError::PathNotFound => LosslessError::PathNotFound(path.to_string()),
            glaucus_cst::SetError::NotAMapping => LosslessError::NotAMapping,
        })
    }
}

impl std::fmt::Display for LosslessYamlDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.has_bom {
            write!(f, "\u{FEFF}{}", self.inner)
        } else {
            write!(f, "{}", self.inner)
        }
    }
}

/// Fallback: targeted source-span replacement directly on raw text.
/// Useful if a document has syntax quirks that a parser rejects or for minimal scalar edits.
pub fn targeted_source_span_replace(src: &str, key: &str, new_val: &str) -> Option<String> {
    let is_crlf = src.contains("\r\n");
    let line_ending = if is_crlf { "\r\n" } else { "\n" };

    let (has_bom, clean_src) = if src.starts_with('\u{FEFF}') {
        (true, &src['\u{FEFF}'.len_utf8()..])
    } else {
        (false, src)
    };

    let mut lines = Vec::new();
    let mut replaced = false;

    for line in clean_src.split(line_ending) {
        if !replaced && line.trim_start().starts_with(key) {
            let trimmed = line.trim_start();
            let indent_len = line.len() - trimmed.len();
            let indent = &line[..indent_len];

            if let Some(rest) = trimmed.strip_prefix(key) {
                let rest_trimmed = rest.trim_start();
                if let Some(after_colon) = rest_trimmed.strip_prefix(':') {
                    let val_and_comment = after_colon;
                    let (val_part, comment_part) = match val_and_comment.find('#') {
                        Some(idx) => (&val_and_comment[..idx], &val_and_comment[idx..]),
                        None => (val_and_comment, ""),
                    };
                    let space_before_val = val_part.len() - val_part.trim_start().len();
                    let spaces = if space_before_val > 0 {
                        &val_part[..space_before_val]
                    } else {
                        " "
                    };
                    let comment_suffix = if comment_part.is_empty() {
                        String::new()
                    } else {
                        let trailing_val_space = val_part.len() - val_part.trim_end().len();
                        let pad = if trailing_val_space > 0 {
                            &val_part[val_part.len() - trailing_val_space..]
                        } else {
                            " "
                        };
                        format!("{pad}{comment_part}")
                    };
                    let new_line = format!("{indent}{key}:{spaces}{new_val}{comment_suffix}");
                    lines.push(new_line);
                    replaced = true;
                    continue;
                }
            }
        }
        lines.push(line.to_string());
    }

    if replaced {
        let joined = lines.join(line_ending);
        if has_bom {
            Some(format!("\u{FEFF}{joined}"))
        } else {
            Some(joined)
        }
    } else {
        None
    }
}
