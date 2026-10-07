//! Owns `@path` mentions in user messages and the reference list appended for agy.

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Paths mentioned as `@path`: an `@` that does not follow a word character or backtick,
/// then at least two path characters ending in a word character or `/`.
pub fn mention_candidates(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = vec![];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' || (i > 0 && (word(chars[i - 1]) || chars[i - 1] == '`')) {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < chars.len() && (word(chars[end]) || "./~-".contains(chars[end])) {
            end += 1;
        }
        let mut last = end;
        while last > start && !(word(chars[last - 1]) || chars[last - 1] == '/') {
            last -= 1;
        }
        if last - start >= 2 {
            found.push(chars[start..last].iter().collect());
        }
        i = end.max(i + 1);
    }
    found
}

/// The message with the absolute paths of mentioned files or folders listed after it.
pub fn with_references(text: &str, found: &[String]) -> String {
    if found.is_empty() {
        return text.into();
    }
    let noun = if found.len() > 1 { "files" } else { "file" };
    let list: Vec<String> = found.iter().map(|p| format!("- {p}")).collect();
    format!("{text}\n\nReferenced {noun}:\n{}", list.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mentions_follow_the_legacy_pattern() {
        assert_eq!(
            mention_candidates("see @src/main.rs and @docs/ but not a@b.c or `@x`"),
            ["src/main.rs", "docs/"]
        );
        assert_eq!(
            mention_candidates("@a @ab. @~/notes.md"),
            ["ab", "~/notes.md"]
        );
        assert_eq!(
            with_references("hi", &["/p/a".into()]),
            "hi\n\nReferenced file:\n- /p/a"
        );
        assert_eq!(
            with_references("hi", &["/a".into(), "/b".into()]),
            "hi\n\nReferenced files:\n- /a\n- /b"
        );
        assert_eq!(with_references("hi", &[]), "hi");
    }
}
