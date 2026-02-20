use crate::model::{analyze_sentiment, Article, FeedSource};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;
use std::time::Duration;

static TICKER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[A-Z]{4}\b").unwrap());

/// Fetch and parse a single RSS feed source
pub async fn fetch_feed(
    client: &reqwest::Client,
    source: &FeedSource,
) -> Result<Vec<Article>, String> {
    let resp = client
        .get(&source.url)
        .send()
        .await
        .map_err(|e| format!("Network error for {}: {}", source.name, e))?;

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Read error for {}: {}", source.name, e))?;

    let feed = feed_rs::parser::parse(&bytes[..])
        .map_err(|e| format!("Parse error for {}: {}", source.name, e))?;

    let now = chrono::Utc::now().timestamp();

    let articles: Vec<Article> = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let title = entry
                .title
                .map(|t| t.content)
                .unwrap_or_default()
                .trim()
                .to_string();

            if title.is_empty() {
                return None;
            }

            let url = entry
                .links
                .first()
                .map(|l| l.href.clone())
                .or_else(|| entry.id.clone().into())
                .unwrap_or_default();

            if url.is_empty() {
                return None;
            }

            let published_at = entry
                .published
                .or(entry.updated)
                .map(|dt| dt.timestamp())
                .unwrap_or(now);

            let tickers = extract_tickers(&title);
            let sentiment = analyze_sentiment(&title);

            Some(Article {
                id: 0, // assigned by DB
                title,
                source: source.name.clone(),
                url,
                tickers,
                published_at,
                fetched_at: now,
                read: false,
                bookmarked: false,
                sentiment,
            })
        })
        .collect();

    Ok(articles)
}

/// Extract potential IDX ticker symbols from text
/// Indonesian tickers are 4 uppercase letters (BBCA, TLKM, BBRI, etc.)
fn extract_tickers(text: &str) -> Vec<String> {
    // Common words to exclude (not tickers)
    let exclude = [
        "DARI", "YANG", "AKAN", "BISA", "JADI", "BARU", "HARI", "JUGA",
        "OLEH", "PADA", "PARA", "LAGI", "BAIK", "BAGI", "KATA", "SAAT",
        "TAPI", "MAKA", "DEMI", "AGAR", "BISA", "JIKA", "SOAL", "THIS",
        "THAT", "WITH", "FROM", "HAVE", "BEEN", "WILL", "THEY", "WHAT",
        "WHEN", "INTO", "THAN", "THEM", "EACH", "JUST", "ONLY", "ALSO",
        "VERY", "MORE", "SOME", "OVER", "SUCH", "BACK", "YEAR", "MOST",
    ];

    TICKER_RE
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .filter(|t| !exclude.contains(&t.as_str()))
        .collect()
}

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
];

/// Fetch article body content from URL with retry and multiple User-Agents
pub async fn fetch_article_content(
    client: &reqwest::Client,
    url: &str,
) -> Result<String, String> {
    let mut last_err = String::new();

    for (attempt, ua) in USER_AGENTS.iter().enumerate() {
        let result = client.get(url).header("User-Agent", *ua).send().await;

        match result {
            Ok(resp) => {
                if let Ok(html_str) = resp.text().await {
                    let content = extract_article_text(&html_str);
                    if !content.starts_with("Could not extract") {
                        return Ok(content);
                    }
                    // Try meta description fallback
                    if let Some(desc) = extract_meta_description(&html_str) {
                        if desc.len() > 50 {
                            return Ok(desc);
                        }
                    }
                    last_err = "Content extraction failed".to_string();
                }
            }
            Err(e) => {
                last_err = format!("Attempt {}: {}", attempt + 1, e);
            }
        }

        if attempt < USER_AGENTS.len() - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    Err(last_err)
}

/// Extract readable text from HTML using common article selectors
fn extract_article_text(html: &str) -> String {
    let document = Html::parse_document(html);

    // Try common article content selectors (most specific first)
    let selectors = [
        // Indonesian news sites
        ".detail__body-text",
        ".read__content",
        ".detail_text",
        ".article__content",
        ".content_detail",
        ".inner-article",
        ".article-content-body__item-content",
        ".TextStory-text",
        ".show-text",
        // General selectors
        "article .content",
        "article .entry-content",
        "article .post-content",
        ".article-content",
        ".article-body",
        ".detail-content",
        ".content-detail",
        "[itemprop=\"articleBody\"]",
        "article p",
        ".entry-content p",
        ".post-content p",
        "main article",
        "article",
        "main .content",
        "main",
    ];

    for sel_str in &selectors {
        if let Ok(selector) = Selector::parse(sel_str) {
            let texts: Vec<String> = document
                .select(&selector)
                .flat_map(|el| {
                    el.text()
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                })
                .collect();

            let combined = texts.join("\n");
            // Only use if we got meaningful content (more than just a title)
            if combined.len() > 100 {
                return clean_article_text(&combined);
            }
        }
    }

    // Fallback: extract all <p> tags
    if let Ok(p_selector) = Selector::parse("p") {
        let paragraphs: Vec<String> = document
            .select(&p_selector)
            .map(|el| {
                el.text()
                    .collect::<Vec<_>>()
                    .join("")
                    .trim()
                    .to_string()
            })
            .filter(|t| t.len() > 20) // skip tiny fragments
            .collect();

        if !paragraphs.is_empty() {
            return clean_article_text(&paragraphs.join("\n\n"));
        }
    }

    "Could not extract article content. Press [o] to open in browser.".to_string()
}

/// Clean up extracted text
fn clean_article_text(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut prev_empty = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                lines.push(String::new());
                prev_empty = true;
            }
        } else {
            lines.push(trimmed.to_string());
            prev_empty = false;
        }
    }

    // Remove trailing empty lines
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }

    lines.join("\n")
}

/// Extract meta description as fallback content
fn extract_meta_description(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    for selector_str in &[
        "meta[property=\"og:description\"]",
        "meta[name=\"description\"]",
    ] {
        if let Ok(selector) = Selector::parse(selector_str) {
            if let Some(el) = document.select(&selector).next() {
                if let Some(content) = el.value().attr("content") {
                    let trimmed = content.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
            }
        }
    }
    None
}

/// Fetch all enabled feeds concurrently
pub async fn fetch_all_feeds(
    client: &reqwest::Client,
    sources: &[FeedSource],
) -> Vec<(String, Result<Vec<Article>, String>)> {
    let mut handles = Vec::new();

    for source in sources.iter().filter(|s| s.enabled) {
        let client = client.clone();
        let source = source.clone();
        handles.push(tokio::spawn(async move {
            let name = source.name.clone();
            let result = fetch_feed(&client, &source).await;
            (name, result)
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    results
}
