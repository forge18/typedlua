use serde::{Deserialize, Serialize};
use typedlua_parser::span::Span;

/// A source map builder following the Source Map v3 specification
/// https://sourcemaps.info/spec.html
#[derive(Debug)]
pub struct SourceMapBuilder {
    file: Option<String>,
    source_root: Option<String>,
    sources: Vec<String>,
    sources_content: Vec<Option<String>>,
    names: Vec<String>,
    mappings: Vec<Mapping>,
    generated_line: usize,
    generated_column: usize,
}

#[derive(Debug, Clone)]
struct Mapping {
    generated_line: usize,
    generated_column: usize,
    source_index: usize,
    source_line: usize,
    source_column: usize,
    name_index: Option<usize>,
}

/// The JSON structure for source maps
#[derive(Debug, Serialize, Deserialize)]
pub struct SourceMap {
    pub version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    pub sources: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sources_content: Vec<Option<String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    pub mappings: String,
}

impl SourceMapBuilder {
    pub fn new(source_file: String) -> Self {
        Self {
            file: None,
            source_root: None,
            sources: vec![source_file],
            sources_content: Vec::new(),
            names: Vec::new(),
            mappings: Vec::new(),
            generated_line: 0,
            generated_column: 0,
        }
    }

    /// Create a new source map builder with multiple source files (for bundle mode)
    pub fn new_multi_source(source_files: Vec<String>) -> Self {
        Self {
            file: None,
            source_root: None,
            sources: source_files,
            sources_content: Vec::new(),
            names: Vec::new(),
            mappings: Vec::new(),
            generated_line: 0,
            generated_column: 0,
        }
    }

    /// Add a source file and return its index
    pub fn add_source(&mut self, source_file: String) -> usize {
        if let Some(idx) = self.sources.iter().position(|s| s == &source_file) {
            idx
        } else {
            self.sources.push(source_file);
            self.sources.len() - 1
        }
    }

    pub fn set_file(&mut self, file: String) {
        self.file = Some(file);
    }

    pub fn set_source_root(&mut self, source_root: String) {
        self.source_root = Some(source_root);
    }

    pub fn add_source_content(&mut self, content: String) {
        self.sources_content.push(Some(content));
    }

    /// Add a mapping from generated position to source position
    pub fn add_mapping(&mut self, source_span: Span, name: Option<String>) {
        self.add_mapping_with_source(source_span, 0, name);
    }

    /// Add a mapping from generated position to source position with explicit source index
    pub fn add_mapping_with_source(
        &mut self,
        source_span: Span,
        source_index: usize,
        name: Option<String>,
    ) {
        let name_index = name.map(|n| {
            if let Some(idx) = self.names.iter().position(|existing| existing == &n) {
                idx
            } else {
                self.names.push(n);
                self.names.len() - 1
            }
        });

        self.mappings.push(Mapping {
            generated_line: self.generated_line,
            generated_column: self.generated_column,
            source_index,
            source_line: source_span.line as usize,
            source_column: source_span.column as usize,
            name_index,
        });
    }

    /// Advance the generated position by writing text
    pub fn advance(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                self.generated_line += 1;
                self.generated_column = 0;
            } else {
                self.generated_column += 1;
            }
        }
    }

    /// Build the final source map
    pub fn build(self) -> SourceMap {
        let mappings = self.encode_mappings();

        SourceMap {
            version: 3,
            file: self.file,
            source_root: self.source_root,
            sources: self.sources,
            sources_content: self.sources_content,
            names: self.names,
            mappings,
        }
    }

    /// Encode mappings using VLQ (Variable Length Quantity) encoding
    fn encode_mappings(&self) -> String {
        let mut result = String::new();
        let mut prev_generated_line = 0;
        let mut prev_generated_col = 0;
        let mut prev_source_index = 0;
        let mut prev_source_line = 0;
        let mut prev_source_col = 0;
        let mut prev_name_index = 0;

        for mapping in &self.mappings {
            // Add semicolons for new lines
            while prev_generated_line < mapping.generated_line {
                result.push(';');
                prev_generated_line += 1;
                prev_generated_col = 0;
            }

            if !result.is_empty() && !result.ends_with(';') {
                result.push(',');
            }

            // Encode: [generated_col, source_index, source_line, source_col, name_index]
            // All values are relative to previous values (delta encoding)

            // Generated column (always present)
            let generated_col_delta = mapping.generated_column as i32 - prev_generated_col as i32;
            result.push_str(&Self::encode_vlq(generated_col_delta));
            prev_generated_col = mapping.generated_column;

            // Source index
            let source_index_delta = mapping.source_index as i32 - prev_source_index as i32;
            result.push_str(&Self::encode_vlq(source_index_delta));
            prev_source_index = mapping.source_index;

            // Source line
            let source_line_delta = mapping.source_line as i32 - prev_source_line as i32;
            result.push_str(&Self::encode_vlq(source_line_delta));
            prev_source_line = mapping.source_line;

            // Source column
            let source_col_delta = mapping.source_column as i32 - prev_source_col as i32;
            result.push_str(&Self::encode_vlq(source_col_delta));
            prev_source_col = mapping.source_column;

            // Name index (optional)
            if let Some(name_idx) = mapping.name_index {
                let name_index_delta = name_idx as i32 - prev_name_index;
                result.push_str(&Self::encode_vlq(name_index_delta));
                prev_name_index = name_idx as i32;
            }
        }

        result
    }

    /// Encode a single value using VLQ (Variable Length Quantity) Base64 encoding
    fn encode_vlq(value: i32) -> String {
        const BASE64_CHARS: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let mut vlq = if value < 0 {
            ((-value) << 1) | 1
        } else {
            value << 1
        };

        let mut result = String::new();

        loop {
            let mut digit = (vlq & 0x1F) as u8;
            vlq >>= 5;

            if vlq > 0 {
                digit |= 0x20; // Continuation bit
            }

            result.push(BASE64_CHARS[digit as usize] as char);

            if vlq == 0 {
                break;
            }
        }

        result
    }
}

impl SourceMap {
    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate the inline source map data URI
    pub fn to_data_uri(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, json.as_bytes());
        Ok(format!(
            "data:application/json;charset=utf-8;base64,{}",
            encoded
        ))
    }

    /// Generate the source mapping URL comment for Lua
    pub fn to_comment(&self) -> Result<String, serde_json::Error> {
        let data_uri = self.to_data_uri()?;
        Ok(format!("--# sourceMappingURL={}", data_uri))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map_builder() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());
        builder.set_file("output.lua".to_string());

        // Add a simple mapping
        builder.add_mapping(Span::new(0, 0, 0, 5), Some("foo".to_string()));
        builder.advance("local");

        let source_map = builder.build();

        assert_eq!(source_map.version, 3);
        assert_eq!(source_map.file, Some("output.lua".to_string()));
        assert_eq!(source_map.sources, vec!["input.tl".to_string()]);
        assert!(source_map.names.contains(&"foo".to_string()));
    }

    #[test]
    fn test_vlq_encoding() {
        assert_eq!(SourceMapBuilder::encode_vlq(0), "A");
        assert_eq!(SourceMapBuilder::encode_vlq(1), "C");
        assert_eq!(SourceMapBuilder::encode_vlq(-1), "D");
        assert_eq!(SourceMapBuilder::encode_vlq(123), "2H");
    }

    #[test]
    fn test_source_map_to_json() {
        let source_map = SourceMap {
            version: 3,
            file: Some("output.lua".to_string()),
            source_root: None,
            sources: vec!["input.tl".to_string()],
            sources_content: vec![],
            names: vec!["foo".to_string()],
            mappings: "AAAA".to_string(),
        };

        let json = source_map.to_json().unwrap();
        assert!(json.contains("\"version\": 3"));
        assert!(json.contains("\"file\": \"output.lua\""));
    }

    #[test]
    fn test_source_map_data_uri() {
        let source_map = SourceMap {
            version: 3,
            file: Some("output.lua".to_string()),
            source_root: None,
            sources: vec!["input.tl".to_string()],
            sources_content: vec![],
            names: vec![],
            mappings: "AAAA".to_string(),
        };

        let data_uri = source_map.to_data_uri().unwrap();
        assert!(data_uri.starts_with("data:application/json;charset=utf-8;base64,"));
    }

    #[test]
    fn test_source_map_comment() {
        let source_map = SourceMap {
            version: 3,
            file: Some("output.lua".to_string()),
            source_root: None,
            sources: vec!["input.tl".to_string()],
            sources_content: vec![],
            names: vec![],
            mappings: "AAAA".to_string(),
        };

        let comment = source_map.to_comment().unwrap();
        assert!(comment.starts_with("--# sourceMappingURL=data:application/json"));
    }

    #[test]
    fn test_multiple_mappings() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());

        // Add multiple mappings
        builder.add_mapping(Span::new(0, 5, 1, 1), Some("foo".to_string()));
        builder.advance("local");
        builder.advance(" ");

        builder.add_mapping(Span::new(6, 9, 1, 7), Some("foo".to_string()));
        builder.advance("foo");
        builder.advance(" ");

        builder.add_mapping(Span::new(10, 11, 1, 11), None);
        builder.advance("=");

        let source_map = builder.build();

        assert_eq!(source_map.version, 3);
        assert_eq!(source_map.names.len(), 1); // Only "foo" should be deduplicated
        assert!(source_map.names.contains(&"foo".to_string()));
        assert!(!source_map.mappings.is_empty());
    }

    #[test]
    fn test_multiline_mappings() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());

        // First line
        builder.add_mapping(Span::new(0, 5, 1, 1), None);
        builder.advance("local");
        builder.advance("\n");

        // Second line
        builder.add_mapping(Span::new(6, 8, 2, 1), None);
        builder.advance("if");
        builder.advance("\n");

        let source_map = builder.build();

        // Mappings string should contain semicolons for line breaks
        assert!(source_map.mappings.contains(';'));
    }

    #[test]
    fn test_advance_tracking() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());

        assert_eq!(builder.generated_line, 0);
        assert_eq!(builder.generated_column, 0);

        builder.advance("hello");
        assert_eq!(builder.generated_line, 0);
        assert_eq!(builder.generated_column, 5);

        builder.advance("\n");
        assert_eq!(builder.generated_line, 1);
        assert_eq!(builder.generated_column, 0);

        builder.advance("world");
        assert_eq!(builder.generated_line, 1);
        assert_eq!(builder.generated_column, 5);
    }

    #[test]
    fn test_source_content() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());
        builder.add_source_content("const x = 42".to_string());

        let source_map = builder.build();

        assert_eq!(source_map.sources_content.len(), 1);
        assert_eq!(
            source_map.sources_content[0],
            Some("const x = 42".to_string())
        );
    }

    #[test]
    fn test_name_deduplication() {
        let mut builder = SourceMapBuilder::new("input.tl".to_string());

        // Add same name multiple times
        builder.add_mapping(Span::new(0, 3, 1, 1), Some("foo".to_string()));
        builder.advance("foo");

        builder.add_mapping(Span::new(4, 7, 1, 5), Some("foo".to_string()));
        builder.advance("foo");

        builder.add_mapping(Span::new(8, 11, 1, 9), Some("bar".to_string()));
        builder.advance("bar");

        let source_map = builder.build();

        // Should only have 2 unique names
        assert_eq!(source_map.names.len(), 2);
        assert!(source_map.names.contains(&"foo".to_string()));
        assert!(source_map.names.contains(&"bar".to_string()));
    }

    #[test]
    fn test_vlq_encoding_edge_cases() {
        // Test various edge cases
        assert_eq!(SourceMapBuilder::encode_vlq(0), "A");
        assert_eq!(SourceMapBuilder::encode_vlq(1), "C");
        assert_eq!(SourceMapBuilder::encode_vlq(-1), "D");
        assert_eq!(SourceMapBuilder::encode_vlq(15), "e");
        assert_eq!(SourceMapBuilder::encode_vlq(-15), "f");
        assert_eq!(SourceMapBuilder::encode_vlq(16), "gB");
        assert_eq!(SourceMapBuilder::encode_vlq(-16), "hB");
    }
}
