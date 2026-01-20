use regex::Regex;
use serde::{Deserialize, Serialize};
use specta::Type;

/// A voice snippet that expands a trigger phrase into full text
#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Snippet {
    /// Unique identifier for the snippet
    pub id: String,
    /// The trigger phrase (e.g., "my email")
    pub trigger: String,
    /// The expanded text (e.g., "john@example.com")
    pub expansion: String,
    /// Whether to match case-sensitively
    #[serde(default)]
    pub case_sensitive: bool,
    /// Whether to only match whole words (not partial matches)
    #[serde(default = "default_whole_word")]
    pub whole_word: bool,
}

fn default_whole_word() -> bool {
    true
}

/// Applies snippet expansions to transcribed text
///
/// Replaces trigger phrases with their corresponding expansions.
/// By default, matching is case-insensitive and only matches whole words.
///
/// # Arguments
/// * `text` - The transcribed text to process
/// * `snippets` - List of snippets to apply
///
/// # Returns
/// The text with all matching triggers replaced by their expansions
pub fn apply_snippets(text: &str, snippets: &[Snippet]) -> String {
    if snippets.is_empty() || text.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    for snippet in snippets {
        if snippet.trigger.is_empty() {
            continue;
        }

        // Escape special regex characters in the trigger
        let escaped_trigger = regex::escape(&snippet.trigger);

        // Build the pattern based on options
        let pattern_str = if snippet.whole_word {
            // Use word boundaries for whole word matching
            format!(r"(?i)\b{}\b", escaped_trigger)
        } else {
            format!(r"(?i){}", escaped_trigger)
        };

        // Adjust pattern for case sensitivity
        let final_pattern = if snippet.case_sensitive {
            pattern_str.replace("(?i)", "")
        } else {
            pattern_str
        };

        if let Ok(re) = Regex::new(&final_pattern) {
            result = re.replace_all(&result, &snippet.expansion).to_string();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_snippet(trigger: &str, expansion: &str) -> Snippet {
        Snippet {
            id: uuid::Uuid::new_v4().to_string(),
            trigger: trigger.to_string(),
            expansion: expansion.to_string(),
            case_sensitive: false,
            whole_word: true,
        }
    }

    #[test]
    fn test_simple_expansion() {
        let text = "Send it to my email please";
        let snippets = vec![create_snippet("my email", "john@example.com")];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "Send it to john@example.com please");
    }

    #[test]
    fn test_case_insensitive() {
        let text = "Send it to MY EMAIL please";
        let snippets = vec![create_snippet("my email", "john@example.com")];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "Send it to john@example.com please");
    }

    #[test]
    fn test_multiple_snippets() {
        let text = "my email and my phone";
        let snippets = vec![
            create_snippet("my email", "john@example.com"),
            create_snippet("my phone", "+1-555-0123"),
        ];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "john@example.com and +1-555-0123");
    }

    #[test]
    fn test_multiple_occurrences() {
        let text = "my email is my email";
        let snippets = vec![create_snippet("my email", "test@test.com")];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "test@test.com is test@test.com");
    }

    #[test]
    fn test_whole_word_only() {
        let text = "emails are great";
        let snippets = vec![create_snippet("email", "test@test.com")];
        let result = apply_snippets(text, &snippets);
        // Should not match "emails" because it's not a whole word match
        assert_eq!(result, "emails are great");
    }

    #[test]
    fn test_partial_match_disabled() {
        let text = "myemail is broken";
        let snippets = vec![create_snippet("my email", "test@test.com")];
        let result = apply_snippets(text, &snippets);
        // Should not match because the trigger has a space
        assert_eq!(result, "myemail is broken");
    }

    #[test]
    fn test_case_sensitive_snippet() {
        let text = "MY EMAIL and my email";
        let snippets = vec![Snippet {
            id: "1".to_string(),
            trigger: "my email".to_string(),
            expansion: "test@test.com".to_string(),
            case_sensitive: true,
            whole_word: true,
        }];
        let result = apply_snippets(text, &snippets);
        // Should only match lowercase "my email"
        assert_eq!(result, "MY EMAIL and test@test.com");
    }

    #[test]
    fn test_empty_input() {
        let text = "";
        let snippets = vec![create_snippet("my email", "test@test.com")];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "");
    }

    #[test]
    fn test_empty_snippets() {
        let text = "my email";
        let snippets: Vec<Snippet> = vec![];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "my email");
    }

    #[test]
    fn test_special_chars_in_trigger() {
        let text = "Call me at (home)";
        let snippets = vec![create_snippet("(home)", "+1-555-HOME")];
        let result = apply_snippets(text, &snippets);
        assert_eq!(result, "Call me at +1-555-HOME");
    }
}
