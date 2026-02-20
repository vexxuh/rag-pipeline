use anyhow::{Context, Result};

pub fn extract_text(pdf_bytes: &[u8]) -> Result<String> {
    pdf_extract::extract_text_from_mem(pdf_bytes).context("Failed to extract text from PDF")
}

pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.is_empty() {
        return Vec::new();
    }

    let mut start = 0;
    while start < words.len() {
        let end = (start + chunk_size).min(words.len());
        let chunk = words[start..end].join(" ");

        if !chunk.trim().is_empty() {
            chunks.push(chunk);
        }

        if end >= words.len() {
            break;
        }

        start += chunk_size - overlap;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_text_basic() {
        let text = (0..100).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
        let chunks = chunk_text(&text, 30, 5);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            let word_count = chunk.split_whitespace().count();
            assert!(word_count <= 30);
        }
    }

    #[test]
    fn test_chunk_text_empty() {
        assert!(chunk_text("", 30, 5).is_empty());
        assert!(chunk_text("   ", 30, 5).is_empty());
    }

    #[test]
    fn test_chunk_text_short() {
        let chunks = chunk_text("hello world", 30, 5);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }
}
