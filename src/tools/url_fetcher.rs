//! URLFetcherTool - Fetches and analyzes web content
//!
//! This tool fetches content from a URL, converts HTML to markdown,
//! and uses an AI model to analyze the content based on a user's prompt.

use crate::{
    error::{KodeError, Result},
    tools::{Tool, ToolContext, ToolStream, ToolStreamItem, ValidationResult},
};
use async_stream::try_stream;
use async_trait::async_trait;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, UPGRADE_INSECURE_REQUESTS, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Cache entry with content and timestamp
#[derive(Clone)]
struct CacheEntry {
    content: String,
    timestamp: SystemTime,
}

/// URL cache with 15-minute expiration
struct UrlCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl UrlCache {
    const CACHE_DURATION: Duration = Duration::from_secs(15 * 60); // 15 minutes

    fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, url: &str) -> Option<String> {
        let cache = self.cache.lock();
        let entry = cache.get(url)?;

        // Check if entry has expired
        if SystemTime::now()
            .duration_since(entry.timestamp)
            .ok()?
            > Self::CACHE_DURATION
        {
            return None;
        }

        Some(entry.content.clone())
    }

    fn set(&self, url: String, content: String) {
        let mut cache = self.cache.lock();
        cache.insert(
            url,
            CacheEntry {
                content,
                timestamp: SystemTime::now(),
            },
        );
    }

    fn clean_expired(&self) {
        let mut cache = self.cache.lock();
        let now = SystemTime::now();
        cache.retain(|_, entry| {
            now.duration_since(entry.timestamp)
                .map(|d| d < Self::CACHE_DURATION)
                .unwrap_or(false)
        });
    }
}

// Global URL cache singleton
static URL_CACHE: Lazy<UrlCache> = Lazy::new(UrlCache::new);

/// Input schema for URLFetcherTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlFetcherInput {
    /// The URL to fetch content from
    pub url: String,

    /// The prompt to run on the fetched content
    pub prompt: String,
}

/// Output from URLFetcherTool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlFetcherOutput {
    /// The normalized URL
    pub url: String,

    /// Whether content was from cache
    pub from_cache: bool,

    /// AI analysis of the content
    pub ai_analysis: String,
}

/// URLFetcherTool implementation
pub struct UrlFetcherTool;

impl UrlFetcherTool {
    /// Normalize URL (auto-upgrade HTTP to HTTPS)
    fn normalize_url(url: &str) -> String {
        if url.starts_with("http://") {
            url.replace("http://", "https://")
        } else {
            url.to_string()
        }
    }

    /// Convert HTML to Markdown (simplified regex-based approach)
    fn html_to_markdown(html: &str) -> Result<String> {
        // Remove scripts and styles
        let re_script = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let re_style = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let re_comment = Regex::new(r"(?is)<!--.*?-->").unwrap();

        let mut text = html.to_string();
        text = re_script.replace_all(&text, "").to_string();
        text = re_style.replace_all(&text, "").to_string();
        text = re_comment.replace_all(&text, "").to_string();

        // Convert headings
        for i in (1..=6).rev() {
            let re = Regex::new(&format!(r"(?is)<h{}[^>]*>(.*?)</h{}>", i, i)).unwrap();
            let replacement = format!("\n\n{} $1\n\n", "#".repeat(i));
            text = re.replace_all(&text, replacement.as_str()).to_string();
        }

        // Convert paragraphs
        let re_p = Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap();
        text = re_p.replace_all(&text, "\n\n$1\n\n").to_string();

        // Convert line breaks
        let re_br = Regex::new(r"(?is)<br\s*/?>").unwrap();
        text = re_br.replace_all(&text, "\n").to_string();

        // Convert links
        let re_link = Regex::new(r#"(?is)<a[^>]*href=["']([^"']*)["'][^>]*>(.*?)</a>"#).unwrap();
        text = re_link.replace_all(&text, "[$2]($1)").to_string();

        // Convert bold
        let re_strong = Regex::new(r"(?is)<strong>(.*?)</strong>").unwrap();
        let re_b = Regex::new(r"(?is)<b>(.*?)</b>").unwrap();
        text = re_strong.replace_all(&text, "**$1**").to_string();
        text = re_b.replace_all(&text, "**$1**").to_string();

        // Convert italic
        let re_em = Regex::new(r"(?is)<em>(.*?)</em>").unwrap();
        let re_i = Regex::new(r"(?is)<i>(.*?)</i>").unwrap();
        text = re_em.replace_all(&text, "_$1_").to_string();
        text = re_i.replace_all(&text, "_$1_").to_string();

        // Convert code
        let re_code = Regex::new(r"(?is)<code>(.*?)</code>").unwrap();
        text = re_code.replace_all(&text, "`$1`").to_string();

        // Convert list items
        let re_li = Regex::new(r"(?is)<li[^>]*>(.*?)</li>").unwrap();
        text = re_li.replace_all(&text, "\n- $1").to_string();

        // Convert horizontal rules
        let re_hr = Regex::new(r"(?is)<hr\s*/?>").unwrap();
        text = re_hr.replace_all(&text, "\n\n---\n\n").to_string();

        // Remove remaining HTML tags
        let re_tags = Regex::new(r"<[^>]+>").unwrap();
        text = re_tags.replace_all(&text, "").to_string();

        // Decode HTML entities (basic ones)
        text = text
            .replace("&nbsp;", " ")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'");

        // Clean up whitespace
        let re_whitespace = Regex::new(r" +").unwrap();
        text = re_whitespace.replace_all(&text, " ").to_string();

        // Clean up excessive newlines
        let re_newlines = Regex::new(r"\n{3,}").unwrap();
        text = re_newlines.replace_all(&text, "\n\n").to_string();

        // Trim lines
        let cleaned = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(cleaned)
    }

    /// Fetch content from URL
    async fn fetch_url(url: &str) -> Result<String> {
        // Create HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| KodeError::ToolExecution(format!("Failed to create HTTP client: {}", e)))?;

        // Set headers
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (compatible; URLFetcher/1.0)"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));

        // Make request
        let response = client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| KodeError::ToolExecution(format!("Failed to fetch URL: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            return Err(KodeError::ToolExecution(format!(
                "HTTP {}: {}",
                response.status().as_u16(),
                response.status().canonical_reason().unwrap_or("Unknown")
            )));
        }

        // Check content type
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.contains("text/") && !content_type.contains("application/") {
            return Err(KodeError::ToolExecution(format!(
                "Unsupported content type: {}",
                content_type
            )));
        }

        // Get body as text
        let html = response
            .text()
            .await
            .map_err(|e| KodeError::ToolExecution(format!("Failed to read response body: {}", e)))?;

        Ok(html)
    }
}

#[async_trait]
impl Tool for UrlFetcherTool {
    type Input = UrlFetcherInput;
    type Output = UrlFetcherOutput;

    fn name(&self) -> &str {
        "WebFetch"
    }

    async fn description(&self) -> String {
        "Fetches content from a URL and processes it using an AI model".to_string()
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from",
                    "format": "uri"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to run on the fetched content"
                }
            },
            "required": ["url", "prompt"]
        })
    }

    async fn prompt(&self, _safe_mode: bool) -> String {
        r#"
- Fetches content from a specified URL and processes it using an AI model
- Takes a URL and a prompt as input
- Fetches the URL content, converts HTML to markdown
- Processes the content with the prompt using a small, fast model
- Returns the model's response about the content
- Use this tool when you need to retrieve and analyze web content

Usage notes:
  - IMPORTANT: If an MCP-provided web fetch tool is available, prefer using that tool instead of this one, as it may have fewer restrictions. All MCP-provided tools start with "mcp__".
  - The URL must be a fully-formed valid URL
  - HTTP URLs will be automatically upgraded to HTTPS
  - The prompt should describe what information you want to extract from the page
  - This tool is read-only and does not modify any files
  - Results may be summarized if the content is very large
  - Includes a self-cleaning 15-minute cache for faster responses when repeatedly accessing the same URL
  - When a URL redirects to a different host, the tool will inform you and provide the redirect URL in a special format. You should then make a new WebFetch request with the redirect URL to fetch the content.
"#.trim().to_string()
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    fn needs_permissions(&self, _input: &Self::Input) -> bool {
        false
    }

    async fn validate_input(
        &self,
        input: &Self::Input,
        _context: &ToolContext,
    ) -> ValidationResult {
        // Validate URL format
        if input.url.trim().is_empty() {
            return ValidationResult::error("URL is required");
        }

        if !input.url.starts_with("http://") && !input.url.starts_with("https://") {
            return ValidationResult::error("URL must start with http:// or https://");
        }

        // Validate prompt
        if input.prompt.trim().is_empty() {
            return ValidationResult::error("Prompt is required");
        }

        ValidationResult::ok()
    }

    fn render_result(&self, output: &Self::Output) -> Result<String> {
        if output.ai_analysis.trim().is_empty() {
            Ok(format!("No content could be analyzed from URL: {}", output.url))
        } else {
            Ok(output.ai_analysis.clone())
        }
    }

    fn render_tool_use(&self, input: &Self::Input, _verbose: bool) -> String {
        format!(
            "Fetching content from {} and analyzing with prompt: \"{}\"",
            input.url, input.prompt
        )
    }

    async fn call(
        &self,
        input: Self::Input,
        _context: ToolContext,
    ) -> Result<ToolStream<Self::Output>> {
        let normalized_url = Self::normalize_url(&input.url);

        Ok(Box::pin(try_stream! {
            // Clean expired cache entries periodically
            URL_CACHE.clean_expired();

            let mut from_cache = false;
            let content: String;

            // Check cache first
            if let Some(cached) = URL_CACHE.get(&normalized_url) {
                content = cached;
                from_cache = true;
            } else {
                // Fetch from URL
                let html = Self::fetch_url(&normalized_url).await?;

                // Convert HTML to markdown
                content = Self::html_to_markdown(&html)?;

                // Cache the result
                URL_CACHE.set(normalized_url.clone(), content.clone());
            }

            // Truncate content if too large
            const MAX_CONTENT_LENGTH: usize = 50000; // ~15k tokens approximately
            let truncated_content = if content.len() > MAX_CONTENT_LENGTH {
                format!(
                    "{}\n\n[Content truncated due to length]",
                    &content[..MAX_CONTENT_LENGTH]
                )
            } else {
                content
            };

            // TODO: AI Analysis - For now, just return the markdown content
            // In the full implementation, this would call a "quick" model
            // to analyze the content based on the prompt
            let ai_analysis = format!(
                "Content from {}:\n\n{}",
                normalized_url,
                truncated_content
            );

            let output = UrlFetcherOutput {
                url: normalized_url,
                from_cache,
                ai_analysis,
            };

            yield ToolStreamItem::Result {
                data: output,
                result_for_assistant: None,
            };
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(
            UrlFetcherTool::normalize_url("http://example.com"),
            "https://example.com"
        );
        assert_eq!(
            UrlFetcherTool::normalize_url("https://example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn test_html_to_markdown() {
        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <h1>Hello World</h1>
                    <p>This is a <strong>test</strong> paragraph.</p>
                    <a href="https://example.com">Link</a>
                </body>
            </html>
        "#;

        let markdown = UrlFetcherTool::html_to_markdown(html).unwrap();
        assert!(markdown.contains("# Hello World"));
        assert!(markdown.contains("**test**"));
        assert!(markdown.contains("[Link](https://example.com)"));
    }

    #[tokio::test]
    async fn test_validation() {
        let tool = UrlFetcherTool;
        let ctx = ToolContext::default();

        // Valid input
        let input = UrlFetcherInput {
            url: "https://example.com".to_string(),
            prompt: "What is this page about?".to_string(),
        };
        let result = tool.validate_input(&input, &ctx).await;
        assert!(result.is_valid);

        // Empty URL
        let input = UrlFetcherInput {
            url: "".to_string(),
            prompt: "test".to_string(),
        };
        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);

        // Invalid URL scheme
        let input = UrlFetcherInput {
            url: "ftp://example.com".to_string(),
            prompt: "test".to_string(),
        };
        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);

        // Empty prompt
        let input = UrlFetcherInput {
            url: "https://example.com".to_string(),
            prompt: "".to_string(),
        };
        let result = tool.validate_input(&input, &ctx).await;
        assert!(!result.is_valid);
    }

    #[test]
    fn test_cache() {
        let cache = UrlCache::new();

        // Set and get
        cache.set("https://example.com".to_string(), "test content".to_string());
        assert_eq!(cache.get("https://example.com"), Some("test content".to_string()));

        // Non-existent key
        assert_eq!(cache.get("https://other.com"), None);
    }
}
