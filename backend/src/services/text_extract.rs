use anyhow::{Context, Result};

/// Supported MIME types for document upload.
pub const SUPPORTED_MIME_TYPES: &[&str] = &[
    "application/pdf",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel",
    "text/xml",
    "application/xml",
    "text/csv",
    "text/plain",
    "text/markdown",
    "application/octet-stream", // fallback — we detect by extension
];

/// Supported file extensions (used as fallback when MIME is generic).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "xlsx", "xls", "xml", "csv", "txt", "md",
];

/// Check if a file is supported by MIME type or extension.
pub fn is_supported(content_type: &str, filename: &str) -> bool {
    if content_type != "application/octet-stream" && SUPPORTED_MIME_TYPES.contains(&content_type) {
        return true;
    }
    extension_from_filename(filename)
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
        .unwrap_or(false)
}

/// Extract text from file bytes, routing to the correct extractor.
///
/// CPU-bound extractors (PDF, DOCX, XLSX) are run on a blocking thread pool
/// via `spawn_blocking` so they don't stall the async runtime.
pub async fn extract_text(bytes: &[u8], content_type: &str, filename: &str) -> Result<String> {
    let ext = extension_from_filename(filename).unwrap_or_default();

    // Determine if this needs blocking extraction
    let needs_blocking = matches!(
        content_type,
        "application/pdf"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            | "application/vnd.ms-excel"
    ) || matches!(ext.as_str(), "pdf" | "docx" | "xlsx" | "xls");

    if needs_blocking {
        let bytes = bytes.to_vec();
        let ct = content_type.to_string();
        let ext = ext.clone();
        let fname = filename.to_string();

        tracing::info!("extract_text: starting blocking extraction for '{fname}' ({ct}, {} bytes)", bytes.len());

        let handle = tokio::task::spawn_blocking(move || {
            tracing::info!("extract_text: spawn_blocking thread started for '{fname}'");
            let result = extract_text_sync(&bytes, &ct, &ext);
            match &result {
                Ok(text) => tracing::info!("extract_text: '{fname}' extraction succeeded, {} chars", text.len()),
                Err(e) => tracing::error!("extract_text: '{fname}' extraction failed: {e:#}"),
            }
            result
        });

        // Time out after 120 seconds to avoid hanging forever on problematic files
        match tokio::time::timeout(std::time::Duration::from_secs(120), handle).await {
            Ok(join_result) => join_result.context("Text extraction task panicked")?,
            Err(_) => anyhow::bail!("Text extraction timed out after 120s for '{filename}'"),
        }
    } else {
        extract_text_sync(bytes, content_type, &ext)
    }
}

/// Synchronous text extraction — called directly for lightweight formats,
/// or via `spawn_blocking` for CPU-heavy ones (PDF, DOCX, XLSX).
fn extract_text_sync(bytes: &[u8], content_type: &str, ext: &str) -> Result<String> {
    match content_type {
        "application/pdf" => extract_pdf(bytes),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            extract_docx(bytes)
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.ms-excel" => extract_xlsx(bytes),
        "text/xml" | "application/xml" => extract_xml(bytes),
        "text/csv" => extract_csv(bytes),
        "text/plain" | "text/markdown" => extract_plaintext(bytes),
        // Fallback: detect by extension
        _ => match ext {
            "pdf" => extract_pdf(bytes),
            "docx" => extract_docx(bytes),
            "xlsx" | "xls" => extract_xlsx(bytes),
            "xml" => extract_xml(bytes),
            "csv" => extract_csv(bytes),
            "txt" | "md" => extract_plaintext(bytes),
            _ => Err(anyhow::anyhow!(
                "Unsupported file type: {content_type} (ext: {ext})"
            )),
        },
    }
}

fn extract_pdf(bytes: &[u8]) -> Result<String> {
    // Try pdftotext (poppler) first — much faster and handles complex PDFs better
    match extract_pdf_pdftotext(bytes) {
        Ok(text) if !text.trim().is_empty() => {
            tracing::info!("PDF extracted via pdftotext ({} chars)", text.len());
            return Ok(text);
        }
        Ok(_) => tracing::warn!("pdftotext returned empty text, falling back to pdf_extract"),
        Err(e) => tracing::warn!("pdftotext failed ({e:#}), falling back to pdf_extract"),
    }

    // Fallback to pure-Rust pdf_extract
    tracing::info!("Extracting PDF via pdf_extract (this may be slow for large files)");
    pdf_extract::extract_text_from_mem(bytes).context("Failed to extract text from PDF")
}

fn extract_pdf_pdftotext(bytes: &[u8]) -> Result<String> {
    use std::io::Write;
    use std::process::Command;

    // Write bytes to a temp file (pdftotext reads from file)
    let mut tmp = tempfile::NamedTempFile::new().context("Failed to create temp file")?;
    tmp.write_all(bytes).context("Failed to write PDF to temp file")?;
    tmp.flush()?;

    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg(tmp.path())
        .arg("-") // output to stdout
        .output()
        .context("Failed to run pdftotext — is poppler-utils installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pdftotext exited with {}: {stderr}", output.status);
    }

    String::from_utf8(output.stdout).context("pdftotext output is not valid UTF-8")
}

fn extract_docx(bytes: &[u8]) -> Result<String> {
    let doc = docx_rs::read_docx(bytes).map_err(|e| anyhow::anyhow!("Failed to read DOCX: {e}"))?;

    let mut text = String::new();
    for child in doc.document.children.iter() {
        collect_docx_text(child, &mut text);
    }
    Ok(text)
}

fn collect_docx_text(child: &docx_rs::DocumentChild, out: &mut String) {
    match child {
        docx_rs::DocumentChild::Paragraph(p) => {
            for run_child in &p.children {
                if let docx_rs::ParagraphChild::Run(run) = run_child {
                    for rc in &run.children {
                        if let docx_rs::RunChild::Text(t) = rc {
                            out.push_str(&t.text);
                        }
                    }
                }
            }
            out.push('\n');
        }
        docx_rs::DocumentChild::Table(table) => {
            for row in &table.rows {
                let docx_rs::TableChild::TableRow(tr) = row;
                for cell in &tr.cells {
                    let docx_rs::TableRowChild::TableCell(tc) = cell;
                    for tc_child in &tc.children {
                        if let docx_rs::TableCellContent::Paragraph(p) = tc_child {
                            for run_child in &p.children {
                                if let docx_rs::ParagraphChild::Run(run) = run_child {
                                    for rc in &run.children {
                                        if let docx_rs::RunChild::Text(t) = rc {
                                            out.push_str(&t.text);
                                        }
                                    }
                                }
                            }
                            out.push('\t');
                        }
                    }
                }
                out.push('\n');
            }
        }
        _ => {}
    }
}

fn extract_xlsx(bytes: &[u8]) -> Result<String> {
    use calamine::{Reader, open_workbook_auto_from_rs};
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|e| anyhow::anyhow!("Failed to read spreadsheet: {e}"))?;

    let mut text = String::new();
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

    for name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&name) {
            for row in range.rows() {
                let cells: Vec<String> = row
                    .iter()
                    .map(|cell| format!("{cell}"))
                    .collect();
                text.push_str(&cells.join("\t"));
                text.push('\n');
            }
            text.push('\n');
        }
    }

    Ok(text)
}

fn extract_xml(bytes: &[u8]) -> Result<String> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_reader(bytes);
    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                let t = e.unescape().unwrap_or_default();
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    text.push_str(trimmed);
                    text.push(' ');
                }
            }
            Ok(Event::CData(e)) => {
                let t = String::from_utf8_lossy(e.as_ref());
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    text.push_str(trimmed);
                    text.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(text)
}

fn extract_csv(bytes: &[u8]) -> Result<String> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(bytes);

    let mut text = String::new();

    for result in reader.records() {
        let record = result.context("Failed to parse CSV row")?;
        let row: Vec<&str> = record.iter().collect();
        text.push_str(&row.join(" "));
        text.push('\n');
    }

    Ok(text)
}

fn extract_plaintext(bytes: &[u8]) -> Result<String> {
    String::from_utf8(bytes.to_vec()).context("File is not valid UTF-8 text")
}

fn extension_from_filename(filename: &str) -> Option<String> {
    filename
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
}

/// Split text into overlapping word chunks for embedding.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
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
    fn test_is_supported() {
        assert!(is_supported("application/pdf", "test.pdf"));
        assert!(is_supported("text/plain", "readme.txt"));
        assert!(is_supported("application/octet-stream", "doc.docx"));
        assert!(!is_supported("application/octet-stream", "image.png"));
    }

    #[tokio::test]
    async fn test_extract_plaintext() {
        let bytes = b"Hello world\nThis is a test";
        let result = extract_text(bytes, "text/plain", "test.txt").await.unwrap();
        assert_eq!(result, "Hello world\nThis is a test");
    }

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
}
