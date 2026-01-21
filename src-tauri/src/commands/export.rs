use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::io::Cursor;

/// Export format options
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Txt,
    Srt,
    Vtt,
    Json,
    Markdown,
    Csv,
    Html,
    Docx,
    Pdf,
}

/// Segment with timing information for SRT/VTT export
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Option<String>,
}

/// Complete transcript with metadata for export
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TranscriptExport {
    pub title: Option<String>,
    pub source_file: Option<String>,
    pub duration_ms: Option<u64>,
    pub created_at: i64,
    pub text: String,
    pub segments: Option<Vec<TranscriptSegment>>,
}

/// Format milliseconds to SRT timestamp format: HH:MM:SS,mmm
fn format_srt_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let milliseconds = ms % 1000;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        hours, minutes, seconds, milliseconds
    )
}

/// Format milliseconds to VTT timestamp format: HH:MM:SS.mmm
fn format_vtt_timestamp(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let milliseconds = ms % 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        hours, minutes, seconds, milliseconds
    )
}

/// Create segments from plain text (when no timing info is available)
/// Splits text into segments of approximately max_chars_per_segment characters
fn create_segments_from_text(
    text: &str,
    duration_ms: Option<u64>,
    max_chars_per_segment: usize,
) -> Vec<TranscriptSegment> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![];
    }

    let total_duration = duration_ms.unwrap_or(words.len() as u64 * 300); // Estimate 300ms per word if no duration
    let chars_per_ms = if text.len() > 0 && total_duration > 0 {
        total_duration as f64 / text.len() as f64
    } else {
        3.0 // Default: 3ms per character
    };

    let mut segments = Vec::new();
    let mut current_segment = String::new();
    let mut segment_start_char = 0;
    let mut char_count = 0;

    for word in words {
        if !current_segment.is_empty()
            && current_segment.len() + word.len() + 1 > max_chars_per_segment
        {
            // Save current segment
            let start_ms = (segment_start_char as f64 * chars_per_ms) as u64;
            let end_ms = (char_count as f64 * chars_per_ms) as u64;

            segments.push(TranscriptSegment {
                start_ms,
                end_ms: end_ms.min(total_duration),
                text: current_segment.trim().to_string(),
                speaker: None,
            });

            current_segment = String::new();
            segment_start_char = char_count;
        }

        if !current_segment.is_empty() {
            current_segment.push(' ');
            char_count += 1;
        }
        current_segment.push_str(word);
        char_count += word.len();
    }

    // Add remaining text as final segment
    if !current_segment.is_empty() {
        let start_ms = (segment_start_char as f64 * chars_per_ms) as u64;
        segments.push(TranscriptSegment {
            start_ms,
            end_ms: total_duration,
            text: current_segment.trim().to_string(),
            speaker: None,
        });
    }

    segments
}

/// Export transcript as plain text
fn export_as_txt(transcript: &TranscriptExport) -> String {
    let mut output = String::new();

    // Add title if present
    if let Some(title) = &transcript.title {
        output.push_str(&format!("# {}\n\n", title));
    }

    // Add metadata
    if let Some(source) = &transcript.source_file {
        output.push_str(&format!("Source: {}\n", source));
    }
    if let Some(duration) = transcript.duration_ms {
        let seconds = duration / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        output.push_str(&format!("Duration: {}:{:02}\n", minutes, remaining_seconds));
    }
    output.push_str(&format!(
        "Created: {}\n",
        chrono::DateTime::from_timestamp(transcript.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    ));
    output.push_str("\n---\n\n");

    // Add transcript text
    if let Some(segments) = &transcript.segments {
        for segment in segments {
            if let Some(speaker) = &segment.speaker {
                output.push_str(&format!("[{}]: {}\n\n", speaker, segment.text));
            } else {
                output.push_str(&format!("{}\n\n", segment.text));
            }
        }
    } else {
        output.push_str(&transcript.text);
    }

    output
}

/// Export transcript as SRT (SubRip Subtitle) format
fn export_as_srt(transcript: &TranscriptExport) -> String {
    let segments = transcript
        .segments
        .clone()
        .unwrap_or_else(|| create_segments_from_text(&transcript.text, transcript.duration_ms, 80));

    // Handle empty transcript
    if segments.is_empty() {
        return String::new();
    }

    let mut output = String::new();

    for (i, segment) in segments.iter().enumerate() {
        // Sequence number (1-based)
        output.push_str(&format!("{}\n", i + 1));

        // Timestamp
        output.push_str(&format!(
            "{} --> {}\n",
            format_srt_timestamp(segment.start_ms),
            format_srt_timestamp(segment.end_ms)
        ));

        // Text (with optional speaker)
        if let Some(speaker) = &segment.speaker {
            output.push_str(&format!("[{}] {}\n", speaker, segment.text));
        } else {
            output.push_str(&format!("{}\n", segment.text));
        }

        // Blank line between entries
        output.push('\n');
    }

    output
}

/// Export transcript as WebVTT format
fn export_as_vtt(transcript: &TranscriptExport) -> String {
    let segments = transcript
        .segments
        .clone()
        .unwrap_or_else(|| create_segments_from_text(&transcript.text, transcript.duration_ms, 80));

    // VTT header is always required, even for empty content
    let mut output = String::from("WEBVTT\n\n");

    // Handle empty transcript - return valid empty VTT
    if segments.is_empty() {
        return output;
    }

    // Add metadata as NOTE
    if transcript.title.is_some() || transcript.source_file.is_some() {
        output.push_str("NOTE\n");
        if let Some(title) = &transcript.title {
            output.push_str(&format!("Title: {}\n", title));
        }
        if let Some(source) = &transcript.source_file {
            output.push_str(&format!("Source: {}\n", source));
        }
        output.push_str("\n");
    }

    for (i, segment) in segments.iter().enumerate() {
        // Optional cue identifier
        output.push_str(&format!("cue-{}\n", i + 1));

        // Timestamp
        output.push_str(&format!(
            "{} --> {}\n",
            format_vtt_timestamp(segment.start_ms),
            format_vtt_timestamp(segment.end_ms)
        ));

        // Text (with optional speaker using <v> tag)
        if let Some(speaker) = &segment.speaker {
            output.push_str(&format!("<v {}>{}\n", speaker, segment.text));
        } else {
            output.push_str(&format!("{}\n", segment.text));
        }

        // Blank line between entries
        output.push('\n');
    }

    output
}

/// Export transcript as JSON
fn export_as_json(transcript: &TranscriptExport) -> Result<String> {
    serde_json::to_string_pretty(transcript).map_err(|e| anyhow!("Failed to serialize JSON: {}", e))
}

/// Escape special characters for CSV
fn csv_escape(text: &str) -> String {
    // If text contains commas, quotes, or newlines, wrap in quotes and escape internal quotes
    if text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// Escape special characters for HTML
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Export transcript as Markdown
fn export_as_markdown(transcript: &TranscriptExport) -> String {
    let mut output = String::new();

    // Add title as H1
    if let Some(title) = &transcript.title {
        output.push_str(&format!("# {}\n\n", title));
    } else {
        output.push_str("# Transcript\n\n");
    }

    // Add metadata as list
    output.push_str("## Metadata\n\n");
    if let Some(source) = &transcript.source_file {
        output.push_str(&format!("- **Source:** {}\n", source));
    }
    if let Some(duration) = transcript.duration_ms {
        let seconds = duration / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        output.push_str(&format!(
            "- **Duration:** {}:{:02}\n",
            minutes, remaining_seconds
        ));
    }
    output.push_str(&format!(
        "- **Created:** {}\n",
        chrono::DateTime::from_timestamp(transcript.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    ));
    output.push_str("\n---\n\n");

    // Add transcript content
    output.push_str("## Content\n\n");
    if let Some(segments) = &transcript.segments {
        for segment in segments {
            if let Some(speaker) = &segment.speaker {
                output.push_str(&format!("**{}:** {}\n\n", speaker, segment.text));
            } else {
                output.push_str(&format!("{}\n\n", segment.text));
            }
        }
    } else {
        output.push_str(&transcript.text);
        output.push('\n');
    }

    output
}

/// Export transcript as CSV
fn export_as_csv(transcript: &TranscriptExport) -> String {
    let mut output = String::new();

    // CSV Header
    output.push_str("Start (ms),End (ms),Duration (ms),Speaker,Text\n");

    let segments = transcript
        .segments
        .clone()
        .unwrap_or_else(|| create_segments_from_text(&transcript.text, transcript.duration_ms, 80));

    // Handle empty transcript - return just the header
    if segments.is_empty() {
        return output;
    }

    for segment in segments {
        let duration = segment.end_ms.saturating_sub(segment.start_ms);
        let speaker = segment.speaker.as_deref().unwrap_or("");
        output.push_str(&format!(
            "{},{},{},{},{}\n",
            segment.start_ms,
            segment.end_ms,
            duration,
            csv_escape(speaker),
            csv_escape(&segment.text)
        ));
    }

    output
}

/// Export transcript as HTML
fn export_as_html(transcript: &TranscriptExport) -> String {
    let mut output = String::new();

    // PaperFlow pink accent color
    let accent_color = "#da5893";

    // HTML document start
    output.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    output.push_str("  <meta charset=\"UTF-8\">\n");
    output
        .push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    output.push_str(&format!(
        "  <title>{}</title>\n",
        html_escape(transcript.title.as_deref().unwrap_or("Transcript"))
    ));
    output.push_str("  <style>\n");
    output.push_str("    :root {\n");
    output.push_str(&format!("      --accent: {};\n", accent_color));
    output.push_str("    }\n");
    output.push_str("    body {\n");
    output.push_str("      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;\n");
    output.push_str("      max-width: 800px;\n");
    output.push_str("      margin: 0 auto;\n");
    output.push_str("      padding: 2rem;\n");
    output.push_str("      line-height: 1.6;\n");
    output.push_str("      color: #333;\n");
    output.push_str("    }\n");
    output.push_str("    h1 {\n");
    output.push_str("      color: var(--accent);\n");
    output.push_str("      border-bottom: 2px solid var(--accent);\n");
    output.push_str("      padding-bottom: 0.5rem;\n");
    output.push_str("    }\n");
    output.push_str("    .metadata {\n");
    output.push_str("      background: #f5f5f5;\n");
    output.push_str("      padding: 1rem;\n");
    output.push_str("      border-radius: 8px;\n");
    output.push_str("      margin-bottom: 2rem;\n");
    output.push_str("    }\n");
    output.push_str("    .metadata p {\n");
    output.push_str("      margin: 0.25rem 0;\n");
    output.push_str("    }\n");
    output.push_str("    .segment {\n");
    output.push_str("      margin-bottom: 1rem;\n");
    output.push_str("      padding: 0.5rem 0;\n");
    output.push_str("    }\n");
    output.push_str("    .speaker {\n");
    output.push_str("      font-weight: bold;\n");
    output.push_str("      color: var(--accent);\n");
    output.push_str("    }\n");
    output.push_str("    .timestamp {\n");
    output.push_str("      font-size: 0.85em;\n");
    output.push_str("      color: #666;\n");
    output.push_str("    }\n");
    output.push_str("    .footer {\n");
    output.push_str("      margin-top: 2rem;\n");
    output.push_str("      padding-top: 1rem;\n");
    output.push_str("      border-top: 1px solid #ddd;\n");
    output.push_str("      font-size: 0.85em;\n");
    output.push_str("      color: #666;\n");
    output.push_str("    }\n");
    output.push_str("  </style>\n");
    output.push_str("</head>\n<body>\n");

    // Title
    output.push_str(&format!(
        "  <h1>{}</h1>\n",
        html_escape(transcript.title.as_deref().unwrap_or("Transcript"))
    ));

    // Metadata section
    output.push_str("  <div class=\"metadata\">\n");
    if let Some(source) = &transcript.source_file {
        output.push_str(&format!(
            "    <p><strong>Source:</strong> {}</p>\n",
            html_escape(source)
        ));
    }
    if let Some(duration) = transcript.duration_ms {
        let seconds = duration / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        output.push_str(&format!(
            "    <p><strong>Duration:</strong> {}:{:02}</p>\n",
            minutes, remaining_seconds
        ));
    }
    output.push_str(&format!(
        "    <p><strong>Created:</strong> {}</p>\n",
        chrono::DateTime::from_timestamp(transcript.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    ));
    output.push_str("  </div>\n");

    // Content
    output.push_str("  <div class=\"content\">\n");
    if let Some(segments) = &transcript.segments {
        for segment in segments {
            output.push_str("    <div class=\"segment\">\n");
            output.push_str(&format!(
                "      <span class=\"timestamp\">[{}]</span>\n",
                format_vtt_timestamp(segment.start_ms)
            ));
            if let Some(speaker) = &segment.speaker {
                output.push_str(&format!(
                    "      <span class=\"speaker\">{}</span>: {}\n",
                    html_escape(speaker),
                    html_escape(&segment.text)
                ));
            } else {
                output.push_str(&format!("      {}\n", html_escape(&segment.text)));
            }
            output.push_str("    </div>\n");
        }
    } else {
        output.push_str(&format!("    <p>{}</p>\n", html_escape(&transcript.text)));
    }
    output.push_str("  </div>\n");

    // Footer
    output.push_str("  <div class=\"footer\">\n");
    output.push_str("    <p>Generated by PaperFlow</p>\n");
    output.push_str("  </div>\n");

    output.push_str("</body>\n</html>\n");

    output
}

/// Export transcript as DOCX (Microsoft Word)
fn export_as_docx(transcript: &TranscriptExport) -> Result<Vec<u8>> {
    use docx_rs::*;

    let accent_color = "DA5893"; // PaperFlow pink

    let mut docx = Docx::new();

    // Title
    let title_text = transcript.title.as_deref().unwrap_or("Transcript");
    let title_para = Paragraph::new().add_run(
        Run::new()
            .add_text(title_text)
            .bold()
            .size(48)
            .color(accent_color),
    );
    docx = docx.add_paragraph(title_para);

    // Add blank line after title
    docx = docx.add_paragraph(Paragraph::new());

    // Metadata section
    if let Some(source) = &transcript.source_file {
        let meta_para = Paragraph::new()
            .add_run(Run::new().add_text("Source: ").bold())
            .add_run(Run::new().add_text(source));
        docx = docx.add_paragraph(meta_para);
    }

    if let Some(duration) = transcript.duration_ms {
        let seconds = duration / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        let meta_para = Paragraph::new()
            .add_run(Run::new().add_text("Duration: ").bold())
            .add_run(Run::new().add_text(&format!("{}:{:02}", minutes, remaining_seconds)));
        docx = docx.add_paragraph(meta_para);
    }

    let created_text = chrono::DateTime::from_timestamp(transcript.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let meta_para = Paragraph::new()
        .add_run(Run::new().add_text("Created: ").bold())
        .add_run(Run::new().add_text(&created_text));
    docx = docx.add_paragraph(meta_para);

    // Add separator
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_text("───────────────────────────────────────")),
    );
    docx = docx.add_paragraph(Paragraph::new());

    // Content
    if let Some(segments) = &transcript.segments {
        for segment in segments {
            let mut para = Paragraph::new();

            // Timestamp in gray
            let timestamp = format_vtt_timestamp(segment.start_ms);
            para = para.add_run(
                Run::new()
                    .add_text(&format!("[{}] ", timestamp))
                    .color("666666"),
            );

            // Speaker in pink bold (if present)
            if let Some(speaker) = &segment.speaker {
                para = para.add_run(
                    Run::new()
                        .add_text(&format!("{}: ", speaker))
                        .bold()
                        .color(accent_color),
                );
            }

            // Text
            para = para.add_run(Run::new().add_text(&segment.text));
            docx = docx.add_paragraph(para);
        }
    } else {
        // Plain text without segments
        let para = Paragraph::new().add_run(Run::new().add_text(&transcript.text));
        docx = docx.add_paragraph(para);
    }

    // Footer
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(
        Paragraph::new().add_run(
            Run::new()
                .add_text("Generated by PaperFlow")
                .color("666666")
                .size(18),
        ),
    );

    // Build to bytes
    let mut buffer = Cursor::new(Vec::new());
    docx.build()
        .pack(&mut buffer)
        .map_err(|e| anyhow!("Failed to build DOCX: {}", e))?;

    Ok(buffer.into_inner())
}

/// Export transcript as PDF
/// Note: Requires Liberation Sans fonts installed on the system, or falls back to simpler output
fn export_as_pdf(transcript: &TranscriptExport) -> Result<Vec<u8>> {
    use genpdf::elements::{Break, Paragraph as PdfParagraph};
    use genpdf::fonts;
    use genpdf::style::{Color, Style};
    use genpdf::{Document, Element, SimplePageDecorator};

    let accent_color = Color::Rgb(218, 88, 147); // PaperFlow pink
    let gray_color = Color::Rgb(102, 102, 102);

    // Try to load Liberation Sans fonts from common system paths
    let font_paths = [
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/liberation-sans",
        "/Library/Fonts",
        "/System/Library/Fonts",
        "C:\\Windows\\Fonts",
    ];

    let font_family = font_paths.iter()
        .find_map(|path| fonts::from_files(path, "LiberationSans", None).ok())
        .or_else(|| fonts::from_files("/System/Library/Fonts", "Helvetica", None).ok())
        .ok_or_else(|| anyhow!("PDF export requires Liberation Sans or Helvetica fonts. Please use HTML or DOCX export instead."))?;

    let mut doc = Document::new(font_family);
    doc.set_title(transcript.title.as_deref().unwrap_or("Transcript"));

    // Set page decorator for margins
    let decorator = SimplePageDecorator::new();
    doc.set_page_decorator(decorator);

    // Title
    let title_text = transcript.title.as_deref().unwrap_or("Transcript");
    let title_style = Style::new()
        .with_font_size(24)
        .bold()
        .with_color(accent_color);
    doc.push(PdfParagraph::new(title_text).styled(title_style));
    doc.push(Break::new(1));

    // Metadata
    if let Some(source) = &transcript.source_file {
        let label_style = Style::new().bold();
        let mut para = PdfParagraph::default();
        para.push_styled("Source: ", label_style);
        para.push(source.as_str());
        doc.push(para);
    }

    if let Some(duration) = transcript.duration_ms {
        let seconds = duration / 1000;
        let minutes = seconds / 60;
        let remaining_seconds = seconds % 60;
        let label_style = Style::new().bold();
        let mut para = PdfParagraph::default();
        para.push_styled("Duration: ", label_style);
        para.push(format!("{}:{:02}", minutes, remaining_seconds));
        doc.push(para);
    }

    let created_text = chrono::DateTime::from_timestamp(transcript.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let label_style = Style::new().bold();
    let mut para = PdfParagraph::default();
    para.push_styled("Created: ", label_style);
    para.push(created_text);
    doc.push(para);

    // Separator
    doc.push(Break::new(1));
    doc.push(PdfParagraph::new("───────────────────────────────────────"));
    doc.push(Break::new(1));

    // Content
    if let Some(segments) = &transcript.segments {
        for segment in segments {
            let timestamp = format_vtt_timestamp(segment.start_ms);
            let timestamp_style = Style::new().with_color(gray_color);
            let speaker_style = Style::new().bold().with_color(accent_color);

            let mut para = PdfParagraph::default();
            para.push_styled(format!("[{}] ", timestamp), timestamp_style);

            if let Some(speaker) = &segment.speaker {
                para.push_styled(format!("{}: ", speaker), speaker_style);
            }

            para.push(&segment.text);
            doc.push(para);
        }
    } else {
        doc.push(PdfParagraph::new(&transcript.text));
    }

    // Footer
    doc.push(Break::new(2));
    let footer_style = Style::new().with_font_size(9).with_color(gray_color);
    doc.push(PdfParagraph::new("Generated by PaperFlow").styled(footer_style));

    // Render to bytes
    let mut buffer = Vec::new();
    doc.render(&mut buffer)
        .map_err(|e| anyhow!("Failed to render PDF: {}", e))?;

    Ok(buffer)
}

/// Export a transcript in the specified format
#[tauri::command]
#[specta::specta]
pub fn export_transcript(
    text: String,
    format: ExportFormat,
    title: Option<String>,
    source_file: Option<String>,
    duration_ms: Option<u64>,
    segments: Option<Vec<TranscriptSegment>>,
) -> Result<String, String> {
    let transcript = TranscriptExport {
        title,
        source_file,
        duration_ms,
        created_at: chrono::Utc::now().timestamp(),
        text,
        segments,
    };

    match format {
        ExportFormat::Txt => Ok(export_as_txt(&transcript)),
        ExportFormat::Srt => Ok(export_as_srt(&transcript)),
        ExportFormat::Vtt => Ok(export_as_vtt(&transcript)),
        ExportFormat::Json => export_as_json(&transcript).map_err(|e| e.to_string()),
        ExportFormat::Markdown => Ok(export_as_markdown(&transcript)),
        ExportFormat::Csv => Ok(export_as_csv(&transcript)),
        ExportFormat::Html => Ok(export_as_html(&transcript)),
        ExportFormat::Docx | ExportFormat::Pdf => {
            Err("Binary formats (DOCX, PDF) must use export_transcript_binary or export_transcript_to_file".to_string())
        }
    }
}

/// Export a transcript and save to file
#[tauri::command]
#[specta::specta]
pub fn export_transcript_to_file(
    text: String,
    format: ExportFormat,
    file_path: String,
    title: Option<String>,
    source_file: Option<String>,
    duration_ms: Option<u64>,
    segments: Option<Vec<TranscriptSegment>>,
) -> Result<(), String> {
    // Validate file path
    if file_path.is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    // Ensure parent directory exists
    let path = std::path::Path::new(&file_path);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }
    }

    let transcript = TranscriptExport {
        title,
        source_file,
        duration_ms,
        created_at: chrono::Utc::now().timestamp(),
        text,
        segments,
    };

    // Handle binary formats separately
    match format {
        ExportFormat::Docx => {
            let bytes = export_as_docx(&transcript).map_err(|e| e.to_string())?;
            std::fs::write(&file_path, bytes)
                .map_err(|e| format!("Failed to write file {}: {}", file_path, e))
        }
        ExportFormat::Pdf => {
            let bytes = export_as_pdf(&transcript).map_err(|e| e.to_string())?;
            std::fs::write(&file_path, bytes)
                .map_err(|e| format!("Failed to write file {}: {}", file_path, e))
        }
        _ => {
            let content = match format {
                ExportFormat::Txt => export_as_txt(&transcript),
                ExportFormat::Srt => export_as_srt(&transcript),
                ExportFormat::Vtt => export_as_vtt(&transcript),
                ExportFormat::Json => export_as_json(&transcript).map_err(|e| e.to_string())?,
                ExportFormat::Markdown => export_as_markdown(&transcript),
                ExportFormat::Csv => export_as_csv(&transcript),
                ExportFormat::Html => export_as_html(&transcript),
                ExportFormat::Docx | ExportFormat::Pdf => unreachable!(),
            };
            std::fs::write(&file_path, content)
                .map_err(|e| format!("Failed to write file {}: {}", file_path, e))
        }
    }
}

/// Get the appropriate file extension for a format
#[tauri::command]
#[specta::specta]
pub fn get_export_file_extension(format: ExportFormat) -> String {
    match format {
        ExportFormat::Txt => "txt".to_string(),
        ExportFormat::Srt => "srt".to_string(),
        ExportFormat::Vtt => "vtt".to_string(),
        ExportFormat::Json => "json".to_string(),
        ExportFormat::Markdown => "md".to_string(),
        ExportFormat::Csv => "csv".to_string(),
        ExportFormat::Html => "html".to_string(),
        ExportFormat::Docx => "docx".to_string(),
        ExportFormat::Pdf => "pdf".to_string(),
    }
}

/// Get all available export formats
#[tauri::command]
#[specta::specta]
pub fn get_available_export_formats() -> Vec<ExportFormat> {
    vec![
        ExportFormat::Txt,
        ExportFormat::Srt,
        ExportFormat::Vtt,
        ExportFormat::Json,
        ExportFormat::Markdown,
        ExportFormat::Csv,
        ExportFormat::Html,
        ExportFormat::Docx,
        ExportFormat::Pdf,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_timestamp_format() {
        assert_eq!(format_srt_timestamp(0), "00:00:00,000");
        assert_eq!(format_srt_timestamp(1500), "00:00:01,500");
        assert_eq!(format_srt_timestamp(61000), "00:01:01,000");
        assert_eq!(format_srt_timestamp(3661500), "01:01:01,500");
    }

    #[test]
    fn test_vtt_timestamp_format() {
        assert_eq!(format_vtt_timestamp(0), "00:00:00.000");
        assert_eq!(format_vtt_timestamp(1500), "00:00:01.500");
        assert_eq!(format_vtt_timestamp(61000), "00:01:01.000");
        assert_eq!(format_vtt_timestamp(3661500), "01:01:01.500");
    }

    #[test]
    fn test_create_segments_from_text() {
        let text = "Hello world this is a test transcript.";
        let segments = create_segments_from_text(text, Some(10000), 20);

        assert!(!segments.is_empty());
        // Verify segments don't overlap
        for i in 1..segments.len() {
            assert!(segments[i].start_ms >= segments[i - 1].end_ms);
        }
    }

    #[test]
    fn test_create_segments_from_empty_text() {
        let segments = create_segments_from_text("", Some(10000), 80);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_create_segments_from_whitespace_only() {
        let segments = create_segments_from_text("   \n\t  ", Some(10000), 80);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_export_as_srt() {
        let transcript = TranscriptExport {
            title: Some("Test".to_string()),
            source_file: None,
            duration_ms: Some(5000),
            created_at: 0,
            text: "Hello world.".to_string(),
            segments: Some(vec![TranscriptSegment {
                start_ms: 0,
                end_ms: 2000,
                text: "Hello world.".to_string(),
                speaker: None,
            }]),
        };

        let srt = export_as_srt(&transcript);
        assert!(srt.contains("1\n"));
        assert!(srt.contains("00:00:00,000 --> 00:00:02,000"));
        assert!(srt.contains("Hello world."));
    }

    #[test]
    fn test_export_as_srt_empty() {
        let transcript = TranscriptExport {
            title: None,
            source_file: None,
            duration_ms: None,
            created_at: 0,
            text: "".to_string(),
            segments: None,
        };

        let srt = export_as_srt(&transcript);
        assert!(srt.is_empty());
    }

    #[test]
    fn test_export_as_vtt() {
        let transcript = TranscriptExport {
            title: Some("Test".to_string()),
            source_file: None,
            duration_ms: Some(5000),
            created_at: 0,
            text: "Hello world.".to_string(),
            segments: Some(vec![TranscriptSegment {
                start_ms: 0,
                end_ms: 2000,
                text: "Hello world.".to_string(),
                speaker: Some("Speaker 1".to_string()),
            }]),
        };

        let vtt = export_as_vtt(&transcript);
        assert!(vtt.starts_with("WEBVTT"));
        assert!(vtt.contains("00:00:00.000 --> 00:00:02.000"));
        assert!(vtt.contains("<v Speaker 1>Hello world."));
    }

    #[test]
    fn test_export_as_vtt_empty() {
        let transcript = TranscriptExport {
            title: None,
            source_file: None,
            duration_ms: None,
            created_at: 0,
            text: "".to_string(),
            segments: None,
        };

        let vtt = export_as_vtt(&transcript);
        assert!(vtt.starts_with("WEBVTT"));
        // Empty VTT should just be the header
        assert_eq!(vtt.trim(), "WEBVTT");
    }

    #[test]
    fn test_export_as_txt_empty() {
        let transcript = TranscriptExport {
            title: None,
            source_file: None,
            duration_ms: None,
            created_at: 0,
            text: "".to_string(),
            segments: None,
        };

        let txt = export_as_txt(&transcript);
        // Should still have created timestamp
        assert!(txt.contains("Created:"));
    }
}
