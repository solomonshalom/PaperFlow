use natural::phonetics::soundex;
use once_cell::sync::Lazy;
use regex::Regex;
use strsim::levenshtein;

/// Applies custom word corrections to transcribed text using fuzzy matching
///
/// This function corrects words in the input text by finding the best matches
/// from a list of custom words using a combination of:
/// - Levenshtein distance for string similarity
/// - Soundex phonetic matching for pronunciation similarity
///
/// # Arguments
/// * `text` - The input text to correct
/// * `custom_words` - List of custom words to match against
/// * `threshold` - Maximum similarity score to accept (0.0 = exact match, 1.0 = any match)
///
/// # Returns
/// The corrected text with custom words applied
pub fn apply_custom_words(text: &str, custom_words: &[String], threshold: f64) -> String {
    if custom_words.is_empty() {
        return text.to_string();
    }

    // Pre-compute lowercase versions to avoid repeated allocations
    let custom_words_lower: Vec<String> = custom_words.iter().map(|w| w.to_lowercase()).collect();

    let words: Vec<&str> = text.split_whitespace().collect();
    let mut corrected_words = Vec::new();

    for word in words {
        let cleaned_word = word
            .trim_matches(|c: char| !c.is_alphabetic())
            .to_lowercase();

        if cleaned_word.is_empty() {
            corrected_words.push(word.to_string());
            continue;
        }

        // Skip extremely long words to avoid performance issues
        if cleaned_word.len() > 50 {
            corrected_words.push(word.to_string());
            continue;
        }

        let mut best_match: Option<&String> = None;
        let mut best_score = f64::MAX;

        for (i, custom_word_lower) in custom_words_lower.iter().enumerate() {
            // Skip if lengths are too different (optimization)
            let len_diff = (cleaned_word.len() as i32 - custom_word_lower.len() as i32).abs();
            if len_diff > 5 {
                continue;
            }

            // Calculate Levenshtein distance (normalized by length)
            let levenshtein_dist = levenshtein(&cleaned_word, custom_word_lower);
            let max_len = cleaned_word.len().max(custom_word_lower.len()) as f64;
            let levenshtein_score = if max_len > 0.0 {
                levenshtein_dist as f64 / max_len
            } else {
                1.0
            };

            // Calculate phonetic similarity using Soundex
            let phonetic_match = soundex(&cleaned_word, custom_word_lower);

            // Combine scores: favor phonetic matches, but also consider string similarity
            let combined_score = if phonetic_match {
                levenshtein_score * 0.3 // Give significant boost to phonetic matches
            } else {
                levenshtein_score
            };

            // Accept if the score is good enough (configurable threshold)
            if combined_score < threshold && combined_score < best_score {
                best_match = Some(&custom_words[i]);
                best_score = combined_score;
            }
        }

        if let Some(replacement) = best_match {
            // Preserve the original case pattern as much as possible
            let corrected = preserve_case_pattern(word, replacement);

            // Preserve punctuation from original word
            let (prefix, suffix) = extract_punctuation(word);
            corrected_words.push(format!("{}{}{}", prefix, corrected, suffix));
        } else {
            corrected_words.push(word.to_string());
        }
    }

    corrected_words.join(" ")
}

/// Preserves the case pattern of the original word when applying a replacement
fn preserve_case_pattern(original: &str, replacement: &str) -> String {
    if original.chars().all(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if original.chars().next().map_or(false, |c| c.is_uppercase()) {
        let mut chars: Vec<char> = replacement.chars().collect();
        if let Some(first_char) = chars.get_mut(0) {
            *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
        }
        chars.into_iter().collect()
    } else {
        replacement.to_string()
    }
}

/// Extracts punctuation prefix and suffix from a word
fn extract_punctuation(word: &str) -> (&str, &str) {
    let prefix_end = word.chars().take_while(|c| !c.is_alphabetic()).count();
    let suffix_start = word
        .char_indices()
        .rev()
        .take_while(|(_, c)| !c.is_alphabetic())
        .count();

    let prefix = if prefix_end > 0 {
        &word[..prefix_end]
    } else {
        ""
    };

    let suffix = if suffix_start > 0 {
        &word[word.len() - suffix_start..]
    } else {
        ""
    };

    (prefix, suffix)
}

/// Filler words to remove from transcriptions
const FILLER_WORDS: &[&str] = &[
    "uh", "um", "uhm", "umm", "uhh", "uhhh", "ah", "eh", "hmm", "hm", "mmm", "mm", "mh", "ha",
    "ehh",
];

static MULTI_SPACE_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s{2,}").unwrap());

/// Collapses repeated 1-2 letter words (3+ repetitions) to a single instance.
/// E.g., "wh wh wh wh" -> "wh", "I I I I" -> "I"
fn collapse_stutters(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }

    let mut result: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let word = words[i];
        let word_lower = word.to_lowercase();

        // Only process 1-2 letter words
        if word_lower.len() <= 2 && word_lower.chars().all(|c| c.is_alphabetic()) {
            // Count consecutive repetitions (case-insensitive)
            let mut count = 1;
            while i + count < words.len() && words[i + count].to_lowercase() == word_lower {
                count += 1;
            }

            // If 3+ repetitions, collapse to single instance
            if count >= 3 {
                result.push(word);
                i += count;
            } else {
                result.push(word);
                i += 1;
            }
        } else {
            result.push(word);
            i += 1;
        }
    }

    result.join(" ")
}

/// Pre-compiled filler word patterns (built lazily)
static FILLER_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    FILLER_WORDS
        .iter()
        .map(|word| {
            // Match filler word with word boundaries, optionally followed by comma or period
            Regex::new(&format!(r"(?i)\b{}\b[,.]?", regex::escape(word))).unwrap()
        })
        .collect()
});

/// Filters transcription output by removing filler words and stutter artifacts.
///
/// This function cleans up raw transcription text by:
/// 1. Removing filler words (uh, um, hmm, etc.)
/// 2. Collapsing repeated 1-2 letter stutters (e.g., "wh wh wh" -> "wh")
/// 3. Cleaning up excess whitespace
///
/// # Arguments
/// * `text` - The raw transcription text to filter
///
/// # Returns
/// The filtered text with filler words and stutters removed
pub fn filter_transcription_output(text: &str) -> String {
    let mut filtered = text.to_string();

    // Remove filler words
    for pattern in FILLER_PATTERNS.iter() {
        filtered = pattern.replace_all(&filtered, "").to_string();
    }

    // Collapse repeated 1-2 letter words (stutter artifacts like "wh wh wh wh")
    filtered = collapse_stutters(&filtered);

    // Clean up multiple spaces to single space
    filtered = MULTI_SPACE_PATTERN.replace_all(&filtered, " ").to_string();

    // Trim leading/trailing whitespace
    filtered.trim().to_string()
}

// === Real-Time Corrections ===

/// Patterns for detecting verbal corrections
static CORRECTION_ACTUALLY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(\w+)\s+(?:,?\s*)?(?:actually|I mean)\s+(\w+)\b").unwrap());

static CORRECTION_WAIT_NO: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:wait|no|sorry)[,.]?\s+(.+)$").unwrap());

/// Applies verbal corrections to transcribed text
///
/// Handles patterns like:
/// - "at 2 actually 3" → "at 3"
/// - "I mean X" → replaces previous related word with X
///
/// # Arguments
/// * `text` - The transcribed text to process
///
/// # Returns
/// The text with verbal corrections applied
pub fn apply_corrections(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Handle "X actually Y" / "X I mean Y" patterns
    // This replaces the word before "actually" with the word after
    result = CORRECTION_ACTUALLY.replace_all(&result, "$2").to_string();

    // Handle "wait/no/sorry, X" at the end - keep just X
    // This is a simpler case where the user restarts their thought
    if let Some(caps) = CORRECTION_WAIT_NO.captures(&result) {
        if let Some(replacement) = caps.get(1) {
            // Only apply if it's at the end of a sentence/phrase
            let match_start = caps.get(0).unwrap().start();
            // Find the last sentence break before the correction
            let last_break = result[..match_start]
                .rfind(|c: char| c == '.' || c == '!' || c == '?' || c == ',')
                .map(|i| i + 1)
                .unwrap_or(0);

            // Replace from the last break to the correction with just the corrected part
            result = format!(
                "{}{}",
                result[..last_break].trim(),
                if last_break > 0 { " " } else { "" },
            ) + replacement.as_str().trim();
        }
    }

    // Clean up any resulting whitespace issues
    result = MULTI_SPACE_PATTERN.replace_all(&result, " ").to_string();
    result.trim().to_string()
}

// === Auto-Formatting ===

/// Verbal command patterns for paragraph/line breaks
static VERBAL_PARAGRAPH_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(new paragraph|next paragraph)\b[,.]?").unwrap());
static VERBAL_LINE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(new line|next line)\b[,.]?").unwrap());

/// Verbal command patterns for bullet points
/// Note: "dash" was intentionally excluded as it causes false positives in regular speech
/// (e.g., "I need to dash to the store" would incorrectly become "I need to • to the store")
static VERBAL_BULLET_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(bullet point|bullet)\s+").unwrap());

/// Verbal command patterns for deletion - sentence deletion (checked FIRST, longer pattern)
/// Supports: scratch/delete/erase/undo/cancel [that] last sentence
static VERBAL_DELETE_LAST_SENTENCE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(scratch|delete|erase|undo|cancel)\s+(?:that\s+)?last\s+sentence\b[,.]?")
        .unwrap()
});

/// Verbal command patterns for deletion - line deletion
/// Supports: scratch/delete/erase/undo/cancel [that] last line
static VERBAL_DELETE_LAST_LINE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(scratch|delete|erase|undo|cancel)\s+(?:that\s+)?last\s+line\b[,.]?")
        .unwrap()
});

/// Verbal command patterns for deletion - word deletion (checked after sentence/line patterns)
/// Supports: delete/scratch/erase/undo/cancel/remove that
static VERBAL_DELETE_THAT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(delete|scratch|erase|undo|cancel|remove)\s+that\b[,.]?").unwrap()
});

/// Pattern to detect ordinal list items like "first, ..., second, ..., third, ..."
static ORDINAL_LIST_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth)[,:]?\s+")
        .unwrap()
});

/// Pattern to detect numeric list items like "one, ..., two, ..., three, ..."
static NUMERIC_WORD_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(one|two|three|four|five|six|seven|eight|nine|ten)[,:]?\s+").unwrap()
});

/// Configuration for auto-formatting
#[derive(Debug, Clone, Default)]
pub struct FormattingRules {
    /// Enable automatic list detection and formatting
    pub auto_lists: bool,
    /// Enable verbal commands (new line, new paragraph, bullet point)
    pub verbal_commands: bool,
}

/// Applies auto-formatting rules to transcribed text
///
/// # Arguments
/// * `text` - The transcribed text to format
/// * `rules` - The formatting rules to apply
///
/// # Returns
/// The formatted text with verbal commands processed and lists formatted
pub fn apply_formatting(text: &str, rules: &FormattingRules) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Process verbal commands first (new line, new paragraph, bullet)
    if rules.verbal_commands {
        result = process_verbal_commands(&result);
    }

    // Detect and format lists
    if rules.auto_lists {
        result = detect_and_format_lists(&result);
    }

    // Clean up any resulting formatting issues
    result = clean_formatting(&result);

    result
}

/// Processes verbal commands in the text
fn process_verbal_commands(text: &str) -> String {
    let mut result = text.to_string();

    // Process deletion commands FIRST (before other verbal commands)
    result = process_deletion_commands(&result);

    // Replace "new paragraph" with double newline
    result = VERBAL_PARAGRAPH_PATTERN
        .replace_all(&result, "\n\n")
        .to_string();

    // Replace "new line" with single newline
    result = VERBAL_LINE_PATTERN.replace_all(&result, "\n").to_string();

    // Replace "bullet point X" with "• X"
    result = VERBAL_BULLET_PATTERN.replace_all(&result, "• ").to_string();

    result
}

/// Processes deletion commands in the text
/// Handles "delete that", "scratch that", etc. to remove the last word
/// Handles "scratch last sentence", "delete last sentence" to remove the last sentence
/// Handles "scratch last line", "delete last line" to remove the last line
fn process_deletion_commands(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Process sentence deletions first (longer pattern priority)
    result = process_delete_last_sentence(&result);

    // Process line deletions second
    result = process_delete_last_line(&result);

    // Then process word deletions
    result = process_delete_that(&result);

    result
}

/// Processes "scratch last sentence" / "delete last sentence" commands
/// Removes the last complete sentence before the command
fn process_delete_last_sentence(text: &str) -> String {
    let mut result = text.to_string();

    // Process from left to right, one match at a time
    while let Some(mat) = VERBAL_DELETE_LAST_SENTENCE_PATTERN.find(&result) {
        let before_command = &result[..mat.start()];
        let after_command = &result[mat.end()..];

        let trimmed_before = before_command.trim_end();

        // Find the second-to-last sentence terminator to keep the previous sentence
        // The "last sentence" is the one that ends at or just before the command
        let new_before = find_text_before_last_sentence(trimmed_before);

        result = format!(
            "{}{}",
            new_before.trim_end(),
            if after_command.trim().is_empty() {
                "".to_string()
            } else {
                format!(" {}", after_command.trim_start())
            }
        );
    }

    result.trim().to_string()
}

/// Finds the text before the last sentence in the given string.
/// Returns everything up to and including the second-to-last sentence terminator.
fn find_text_before_last_sentence(text: &str) -> &str {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return "";
    }

    // Find all sentence terminator positions
    let terminators: Vec<usize> = trimmed
        .char_indices()
        .filter(|(_, c)| *c == '.' || *c == '!' || *c == '?')
        .map(|(i, _)| i)
        .collect();

    match terminators.len() {
        0 => "", // No terminators, delete everything
        1 => "", // Only one terminator (one sentence), delete everything
        _ => {
            // Return text up to and including the second-to-last terminator
            let second_to_last = terminators[terminators.len() - 2];
            &trimmed[..=second_to_last]
        }
    }
}

/// Processes "scratch last line" / "delete last line" commands
/// Removes the last line before the command (text after the last newline)
fn process_delete_last_line(text: &str) -> String {
    let mut result = text.to_string();

    // Process from left to right, one match at a time
    while let Some(mat) = VERBAL_DELETE_LAST_LINE_PATTERN.find(&result) {
        let before_command = &result[..mat.start()];
        let after_command = &result[mat.end()..];

        let trimmed_before = before_command.trim_end();

        // Find the last newline to determine what constitutes "the last line"
        let new_before = find_text_before_last_line(trimmed_before);

        result = format!(
            "{}{}",
            new_before.trim_end(),
            if after_command.trim().is_empty() {
                "".to_string()
            } else {
                format!(" {}", after_command.trim_start())
            }
        );
    }

    result.trim().to_string()
}

/// Finds the text before the last line in the given string.
/// Returns everything up to and including the last newline character.
/// If no newlines exist, returns empty string (the entire text is one line).
fn find_text_before_last_line(text: &str) -> &str {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return "";
    }

    // Find the last newline position
    if let Some(last_newline_pos) = trimmed.rfind('\n') {
        // Return text up to and including the newline
        &trimmed[..=last_newline_pos]
    } else {
        // No newlines - the entire text is one line, delete everything
        ""
    }
}

/// Processes "delete that" / "scratch that" commands
/// Removes the last word before the command
fn process_delete_that(text: &str) -> String {
    let mut result = text.to_string();

    // Process from left to right, one match at a time
    while let Some(mat) = VERBAL_DELETE_THAT_PATTERN.find(&result) {
        let before_command = &result[..mat.start()];
        let after_command = &result[mat.end()..];

        // Find the last word before the command
        let trimmed_before = before_command.trim_end();
        let words: Vec<&str> = trimmed_before.split_whitespace().collect();

        let new_before = if words.is_empty() {
            "".to_string()
        } else {
            // Remove the last word
            words[..words.len() - 1].join(" ")
        };

        result = format!(
            "{}{}",
            new_before,
            if after_command.trim().is_empty() {
                "".to_string()
            } else {
                if new_before.is_empty() {
                    after_command.trim_start().to_string()
                } else {
                    format!(" {}", after_command.trim_start())
                }
            }
        );
    }

    result.trim().to_string()
}

/// Maps ordinal words to numbers
fn ordinal_to_number(word: &str) -> Option<u32> {
    match word.to_lowercase().as_str() {
        "first" => Some(1),
        "second" => Some(2),
        "third" => Some(3),
        "fourth" => Some(4),
        "fifth" => Some(5),
        "sixth" => Some(6),
        "seventh" => Some(7),
        "eighth" => Some(8),
        "ninth" => Some(9),
        "tenth" => Some(10),
        _ => None,
    }
}

/// Maps number words to numbers
fn number_word_to_number(word: &str) -> Option<u32> {
    match word.to_lowercase().as_str() {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => None,
    }
}

/// Detects list patterns and formats them appropriately
fn detect_and_format_lists(text: &str) -> String {
    // Check if text contains potential list markers
    let ordinal_count = ORDINAL_LIST_PATTERN.find_iter(text).count();
    let numeric_count = NUMERIC_WORD_PATTERN.find_iter(text).count();

    // Only format if we have at least 2 list items
    if ordinal_count >= 2 {
        return format_ordinal_list(text);
    }

    if numeric_count >= 2 {
        return format_numeric_word_list(text);
    }

    text.to_string()
}

/// Formats text with ordinal words (first, second, third) into a numbered list
fn format_ordinal_list(text: &str) -> String {
    let mut result = String::new();
    let mut last_end = 0;
    let mut is_first_match = true;

    for cap in ORDINAL_LIST_PATTERN.find_iter(text) {
        // Extract the ordinal word
        let word = cap.as_str().trim_end_matches(|c: char| !c.is_alphabetic());

        if let Some(num) = ordinal_to_number(word) {
            // Add text before this match
            let before = &text[last_end..cap.start()];
            if is_first_match {
                // For first match, preserve the prefix text (including newlines)
                // Only trim trailing spaces, NOT newlines
                let trimmed = before.trim_end_matches(' ');
                if !trimmed.is_empty() {
                    result.push_str(trimmed);
                    // Only add newline if prefix doesn't already end with one
                    if !trimmed.ends_with('\n') {
                        result.push('\n');
                    }
                }
                is_first_match = false;
            } else {
                // For subsequent matches, add text between matches
                let between = before.trim();
                if !between.is_empty() {
                    result.push_str(between);
                }
                result.push('\n');
            }

            // Add the numbered list item
            result.push_str(&format!("{}. ", num));
            last_end = cap.end();
        }
    }

    // Add remaining text
    if last_end < text.len() {
        result.push_str(text[last_end..].trim());
    }

    result
}

/// Formats text with number words (one, two, three) into a numbered list
fn format_numeric_word_list(text: &str) -> String {
    let mut result = String::new();
    let mut last_end = 0;
    let mut is_first_match = true;

    for cap in NUMERIC_WORD_PATTERN.find_iter(text) {
        // Extract the number word
        let word = cap.as_str().trim_end_matches(|c: char| !c.is_alphabetic());

        if let Some(num) = number_word_to_number(word) {
            // Add text before this match
            let before = &text[last_end..cap.start()];
            if is_first_match {
                // For first match, preserve the prefix text (including newlines)
                // Only trim trailing spaces, NOT newlines
                let trimmed = before.trim_end_matches(' ');
                if !trimmed.is_empty() {
                    result.push_str(trimmed);
                    // Only add newline if prefix doesn't already end with one
                    if !trimmed.ends_with('\n') {
                        result.push('\n');
                    }
                }
                is_first_match = false;
            } else {
                // For subsequent matches, add text between matches
                let between = before.trim();
                if !between.is_empty() {
                    result.push_str(between);
                }
                result.push('\n');
            }

            // Add the numbered list item
            result.push_str(&format!("{}. ", num));
            last_end = cap.end();
        }
    }

    // Add remaining text
    if last_end < text.len() {
        result.push_str(text[last_end..].trim());
    }

    result
}

/// Cleans up formatting artifacts like multiple newlines, trailing spaces, etc.
fn clean_formatting(text: &str) -> String {
    let mut result = text.to_string();

    // Replace 3+ newlines with 2
    let excessive_newlines = Regex::new(r"\n{3,}").unwrap();
    result = excessive_newlines.replace_all(&result, "\n\n").to_string();

    // Trim whitespace from each line
    result = result
        .lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n");

    // Trim overall
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_custom_words_exact_match() {
        let text = "hello world";
        let custom_words = vec!["Hello".to_string(), "World".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_apply_custom_words_fuzzy_match() {
        let text = "helo wrold";
        let custom_words = vec!["hello".to_string(), "world".to_string()];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_preserve_case_pattern() {
        assert_eq!(preserve_case_pattern("HELLO", "world"), "WORLD");
        assert_eq!(preserve_case_pattern("Hello", "world"), "World");
        assert_eq!(preserve_case_pattern("hello", "WORLD"), "WORLD");
    }

    #[test]
    fn test_extract_punctuation() {
        assert_eq!(extract_punctuation("hello"), ("", ""));
        assert_eq!(extract_punctuation("!hello?"), ("!", "?"));
        assert_eq!(extract_punctuation("...hello..."), ("...", "..."));
    }

    #[test]
    fn test_empty_custom_words() {
        let text = "hello world";
        let custom_words = vec![];
        let result = apply_custom_words(text, &custom_words, 0.5);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_filter_filler_words() {
        let text = "So um I was thinking uh about this";
        let result = filter_transcription_output(text);
        assert_eq!(result, "So I was thinking about this");
    }

    #[test]
    fn test_filter_filler_words_case_insensitive() {
        let text = "UM this is UH a test";
        let result = filter_transcription_output(text);
        assert_eq!(result, "this is a test");
    }

    #[test]
    fn test_filter_filler_words_with_punctuation() {
        let text = "Well, um, I think, uh. that's right";
        let result = filter_transcription_output(text);
        assert_eq!(result, "Well, I think, that's right");
    }

    #[test]
    fn test_filter_cleans_whitespace() {
        let text = "Hello    world   test";
        let result = filter_transcription_output(text);
        assert_eq!(result, "Hello world test");
    }

    #[test]
    fn test_filter_trims() {
        let text = "  Hello world  ";
        let result = filter_transcription_output(text);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_filter_combined() {
        let text = "  Um, so I was, uh, thinking about this  ";
        let result = filter_transcription_output(text);
        assert_eq!(result, "so I was, thinking about this");
    }

    #[test]
    fn test_filter_preserves_valid_text() {
        let text = "This is a completely normal sentence.";
        let result = filter_transcription_output(text);
        assert_eq!(result, "This is a completely normal sentence.");
    }

    #[test]
    fn test_filter_stutter_collapse() {
        let text = "w wh wh wh wh wh wh wh wh wh why";
        let result = filter_transcription_output(text);
        assert_eq!(result, "w wh why");
    }

    #[test]
    fn test_filter_stutter_short_words() {
        let text = "I I I I think so so so so";
        let result = filter_transcription_output(text);
        assert_eq!(result, "I think so");
    }

    #[test]
    fn test_filter_stutter_mixed_case() {
        let text = "No NO no NO no";
        let result = filter_transcription_output(text);
        assert_eq!(result, "No");
    }

    #[test]
    fn test_filter_stutter_preserves_two_repetitions() {
        let text = "no no is fine";
        let result = filter_transcription_output(text);
        assert_eq!(result, "no no is fine");
    }

    // === Auto-Formatting Tests ===

    #[test]
    fn test_verbal_new_paragraph() {
        let rules = FormattingRules {
            verbal_commands: true,
            auto_lists: false,
        };
        let text = "First paragraph new paragraph second paragraph";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("\n\n"));
        assert!(result.contains("First paragraph"));
        assert!(result.contains("second paragraph"));
    }

    #[test]
    fn test_verbal_new_line() {
        let rules = FormattingRules {
            verbal_commands: true,
            auto_lists: false,
        };
        let text = "Line one new line line two";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("Line one"));
        assert!(result.contains("\n"));
        assert!(result.contains("line two"));
    }

    #[test]
    fn test_verbal_bullet_point() {
        let rules = FormattingRules {
            verbal_commands: true,
            auto_lists: false,
        };
        let text = "Items: bullet point item one, bullet point item two";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("• item one"));
        assert!(result.contains("• item two"));
    }

    #[test]
    fn test_ordinal_list_detection() {
        let rules = FormattingRules {
            verbal_commands: false,
            auto_lists: true,
        };
        let text = "First get the data, second process it, third display results";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("1. "));
        assert!(result.contains("2. "));
        assert!(result.contains("3. "));
    }

    #[test]
    fn test_numeric_word_list_detection() {
        let rules = FormattingRules {
            verbal_commands: false,
            auto_lists: true,
        };
        let text = "One open the app, two click the button, three submit the form";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("1. "));
        assert!(result.contains("2. "));
        assert!(result.contains("3. "));
    }

    #[test]
    fn test_formatting_disabled() {
        let rules = FormattingRules {
            verbal_commands: false,
            auto_lists: false,
        };
        let text = "First thing new paragraph second thing";
        let result = apply_formatting(text, &rules);
        assert_eq!(result, text);
    }

    #[test]
    fn test_formatting_empty_text() {
        let rules = FormattingRules {
            verbal_commands: true,
            auto_lists: true,
        };
        let text = "";
        let result = apply_formatting(text, &rules);
        assert_eq!(result, "");
    }

    #[test]
    fn test_formatting_combined() {
        let rules = FormattingRules {
            verbal_commands: true,
            auto_lists: true,
        };
        let text = "Introduction new paragraph first step one, second step two";
        let result = apply_formatting(text, &rules);
        assert!(result.contains("\n\n")); // new paragraph
        assert!(result.contains("1. ")); // first
        assert!(result.contains("2. ")); // second
    }

    // === Correction Tests ===

    #[test]
    fn test_correction_actually() {
        let text = "Let's meet at 2 actually 3";
        let result = apply_corrections(text);
        assert!(result.contains("3"));
        assert!(!result.contains("2 actually"));
    }

    #[test]
    fn test_correction_i_mean() {
        let text = "Send it to John I mean Jane";
        let result = apply_corrections(text);
        assert!(result.contains("Jane"));
        assert!(!result.contains("John I mean"));
    }

    #[test]
    fn test_correction_no_change() {
        let text = "This is a normal sentence";
        let result = apply_corrections(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_correction_empty() {
        let text = "";
        let result = apply_corrections(text);
        assert_eq!(result, "");
    }

    // === Deletion Command Tests ===

    mod deletion_tests {
        use super::*;

        // --- Delete That (word deletion) tests ---

        #[test]
        fn test_delete_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello world delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello");
        }

        #[test]
        fn test_scratch_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Send report scratch that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Send");
        }

        #[test]
        fn test_erase_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Quick test erase that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Quick");
        }

        #[test]
        fn test_undo_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello there undo that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello");
        }

        #[test]
        fn test_cancel_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Message sent cancel that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Message");
        }

        #[test]
        fn test_delete_that_case_insensitive() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello DELETE THAT";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        #[test]
        fn test_delete_that_preserves_text_after() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "delete that more text";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "more text");
        }

        #[test]
        fn test_delete_that_at_start() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "delete that hello world";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "hello world");
        }

        #[test]
        fn test_delete_that_multiple_commands() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "one two delete that three four delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "one three");
        }

        #[test]
        fn test_delete_that_empty_text() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        #[test]
        fn test_delete_that_only_command() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        #[test]
        fn test_delete_that_with_trailing_punctuation() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello world delete that, more text";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello more text");
        }

        // --- Delete Last Sentence tests ---

        #[test]
        fn test_scratch_last_sentence_removes_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First. Second. scratch last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First.");
        }

        #[test]
        fn test_delete_last_sentence_removes_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello! World! delete last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello!");
        }

        #[test]
        fn test_erase_last_sentence_removes_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A? B? erase last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A?");
        }

        #[test]
        fn test_scratch_that_last_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A. B. scratch that last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A.");
        }

        #[test]
        fn test_delete_last_sentence_no_terminator() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Just words delete last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        #[test]
        fn test_delete_last_sentence_case_insensitive() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First. Second. DELETE LAST SENTENCE";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First.");
        }

        #[test]
        fn test_delete_last_sentence_preserves_text_after() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A. B. delete last sentence more text";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A. more text");
        }

        #[test]
        fn test_delete_last_sentence_only_command() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "scratch last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        // --- Disabled state tests ---

        #[test]
        fn test_delete_that_disabled() {
            let rules = FormattingRules {
                verbal_commands: false,
                auto_lists: false,
            };
            let text = "Hello delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello delete that");
        }

        #[test]
        fn test_delete_last_sentence_disabled() {
            let rules = FormattingRules {
                verbal_commands: false,
                auto_lists: false,
            };
            let text = "A. B. delete last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A. B. delete last sentence");
        }

        // --- Combined with other verbal commands ---

        #[test]
        fn test_delete_that_with_new_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello world delete that new line more text";
            let result = apply_formatting(text, &rules);
            assert!(result.contains("Hello"));
            assert!(result.contains("\n"));
            assert!(result.contains("more text"));
        }

        // --- Edge case tests ---

        #[test]
        fn test_consecutive_delete_that() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "a b c delete that delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "a");
        }

        #[test]
        fn test_delete_that_with_comma_before() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            // Word with trailing comma - should delete "world," as a unit
            let text = "Hello, world, delete that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello,");
        }

        #[test]
        fn test_sentence_with_abbreviation() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            // Abbreviations like "Dr." have periods but aren't sentence endings
            // This is a known limitation - we treat all periods as sentence endings
            let text = "Dr. Smith arrived. He said hello. delete last sentence";
            let result = apply_formatting(text, &rules);
            // Should ideally keep "Dr. Smith arrived." but this is a known edge case
            assert!(result.contains("Dr."));
        }

        #[test]
        fn test_three_sentences_delete_last() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A. B. C. delete last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A. B.");
        }

        // --- Delete Last Line tests ---

        #[test]
        fn test_delete_last_line_removes_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First line\nSecond line delete last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First line");
        }

        #[test]
        fn test_scratch_last_line_removes_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Line one\nLine two scratch last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Line one");
        }

        #[test]
        fn test_erase_last_line_removes_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A\nB erase last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A");
        }

        #[test]
        fn test_undo_last_line_removes_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First\nSecond undo last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First");
        }

        #[test]
        fn test_cancel_last_line_removes_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Top\nBottom cancel last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Top");
        }

        #[test]
        fn test_delete_that_last_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Line A\nLine B delete that last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Line A");
        }

        #[test]
        fn test_delete_last_line_no_newline() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            // No newlines - entire text is one line, should delete everything
            let text = "Just one line delete last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "");
        }

        #[test]
        fn test_delete_last_line_multiple_lines() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Line 1\nLine 2\nLine 3 delete last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Line 1\nLine 2");
        }

        #[test]
        fn test_delete_last_line_preserves_text_after() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A\nB delete last line more text";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A more text");
        }

        #[test]
        fn test_delete_last_line_case_insensitive() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First\nSecond DELETE LAST LINE";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First");
        }

        #[test]
        fn test_delete_last_line_disabled() {
            let rules = FormattingRules {
                verbal_commands: false,
                auto_lists: false,
            };
            let text = "A\nB delete last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A\nB delete last line");
        }

        // --- New synonym tests ---

        #[test]
        fn test_remove_that_removes_last_word() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Hello world remove that";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "Hello");
        }

        #[test]
        fn test_undo_last_sentence_removes_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "First. Second. undo last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "First.");
        }

        #[test]
        fn test_cancel_last_sentence_removes_sentence() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A! B! cancel last sentence";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A!");
        }

        // --- Bullet point dash removal verification ---

        #[test]
        fn test_dash_no_longer_triggers_bullet() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            // "dash" should NOT be converted to bullet anymore
            let text = "I need to dash to the store";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "I need to dash to the store");
        }

        #[test]
        fn test_bullet_still_works() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Items: bullet item one, bullet item two";
            let result = apply_formatting(text, &rules);
            assert!(result.contains("• item one"));
            assert!(result.contains("• item two"));
        }

        #[test]
        fn test_bullet_point_still_works() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "List: bullet point first, bullet point second";
            let result = apply_formatting(text, &rules);
            assert!(result.contains("• first"));
            assert!(result.contains("• second"));
        }

        // --- Combined operations ---

        #[test]
        fn test_delete_line_then_new_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "Line A\nLine B delete last line new line Line C";
            let result = apply_formatting(text, &rules);
            assert!(result.contains("Line A"));
            assert!(result.contains("\n"));
            assert!(result.contains("Line C"));
        }

        #[test]
        fn test_consecutive_delete_last_line() {
            let rules = FormattingRules {
                verbal_commands: true,
                auto_lists: false,
            };
            let text = "A\nB\nC delete last line delete last line";
            let result = apply_formatting(text, &rules);
            assert_eq!(result, "A");
        }
    }
}
