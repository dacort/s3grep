/// Utility functions for s3grep

/**
    Returns true if the given line contains the pattern, respecting case sensitivity.

    # Arguments

    * `line` - The line of text to search.
    * `pattern` - The pattern to search for.
    * `case_sensitive` - If true, the search is case sensitive.

    # Examples

    ```
    use s3grep::line_matches;
    assert!(line_matches("Error: something failed", "Error", true));
    assert!(!line_matches("Error: something failed", "error", true));
    assert!(line_matches("Error: something failed", "error", false));
    ```
*/
pub fn line_matches(line: &str, pattern: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        line.contains(pattern)
    } else {
        line.to_lowercase().contains(&pattern.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_matches_case_sensitive() {
        assert!(line_matches("Error: something failed", "Error", true));
        assert!(!line_matches("Error: something failed", "error", true));
    }

    #[test]
    fn test_line_matches_case_insensitive() {
        assert!(line_matches("Error: something failed", "error", false));
        assert!(line_matches("error: something failed", "Error", false));
    }
}