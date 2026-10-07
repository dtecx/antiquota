//! Owns conservative shell command-line splitting for permission checks.
//!
//! The tokenizer follows POSIX `shlex` rules with shell operators as punctuation: quotes keep
//! their contents together, a backslash escapes the next character, and newlines separate
//! commands. A line that cannot be read safely yields `None`, so the call still needs a decision.

const PUNCTUATION: &str = "();<>|&\n";
const WHITESPACE: &str = " \t\r";
const SEPARATOR_CHARS: &str = "\n;&|";
/// Output redirections that may only target `/dev/null` in an automatically allowed line.
const FILE_REDIRECTS: [&str; 6] = [">", ">>", ">|", "&>", "&>>", "<>"];
/// Operator runs that stay inside a command: input redirection and grouping.
const INLINE: [&str; 5] = ["<", "<<", "<<<", "(", ")"];

enum Token {
    Word(String),
    Operator(String),
}

fn tokens(line: &str) -> Option<Vec<Token>> {
    let mut out = vec![];
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if WHITESPACE.contains(c) {
            chars.next();
        } else if PUNCTUATION.contains(c) {
            let mut run = String::new();
            while let Some(&p) = chars.peek().filter(|p| PUNCTUATION.contains(**p)) {
                run.push(p);
                chars.next();
            }
            out.push(Token::Operator(run));
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if WHITESPACE.contains(c) || PUNCTUATION.contains(c) {
                    break;
                }
                chars.next();
                match c {
                    '\'' => loop {
                        match chars.next()? {
                            '\'' => break,
                            q => word.push(q),
                        }
                    },
                    '"' => loop {
                        match chars.next()? {
                            '"' => break,
                            '\\' => {
                                let next = chars.next()?;
                                if next != '"' && next != '\\' {
                                    word.push('\\');
                                }
                                word.push(next);
                            }
                            q => word.push(q),
                        }
                    },
                    '\\' => word.push(chars.next()?),
                    c => word.push(c),
                }
            }
            out.push(Token::Word(word));
        }
    }
    Some(out)
}

/// The commands of a shell line, each joined back into one string with quotes removed.
/// `None` when the line hides commands (substitution), writes a file through redirection,
/// has unbalanced quotes, or uses an operator this check does not understand.
pub fn command_parts(line: &str) -> Option<Vec<String>> {
    if line.contains("$(") || line.contains('`') {
        return None;
    }
    let tokens = tokens(line)?;
    let mut parts = vec![];
    let mut current: Vec<String> = vec![];
    for (index, token) in tokens.iter().enumerate() {
        let next_word = match tokens.get(index + 1) {
            Some(Token::Word(word)) => Some(word.as_str()),
            _ => None,
        };
        match token {
            Token::Word(word) => current.push(word.clone()),
            Token::Operator(run) if run == ">&" || run == "<&" => {
                // Descriptor duplication such as 2>&1 or closing with >&-; `>& file` writes a file.
                let target = next_word?;
                if target != "-" && !target.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                current.push(run.clone());
            }
            Token::Operator(run) if FILE_REDIRECTS.contains(&run.as_str()) => {
                if next_word != Some("/dev/null") {
                    return None;
                }
                current.push(run.clone());
            }
            Token::Operator(run) if INLINE.contains(&run.as_str()) => current.push(run.clone()),
            Token::Operator(run)
                if run
                    .chars()
                    .all(|c| SEPARATOR_CHARS.contains(c) || "()".contains(c))
                    && run.chars().any(|c| SEPARATOR_CHARS.contains(c)) =>
            {
                parts.push(std::mem::take(&mut current));
            }
            Token::Operator(_) => return None,
        }
    }
    parts.push(current);
    Some(
        parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .map(|p| p.join(" "))
            .collect(),
    )
}

/// A plain split on shell separators, used only to apply deny rules when the line cannot be
/// parsed: a deny must still win over an unreadable line.
pub fn loose_parts(line: &str) -> Vec<String> {
    let mut parts = vec![];
    let mut rest = line;
    while !rest.is_empty() {
        let cut = ["&&", "||", ";", "|", "\n"]
            .iter()
            .filter_map(|sep| rest.find(sep).map(|at| (at, sep.len())))
            .min();
        let (piece, tail) = match cut {
            Some((at, len)) => (&rest[..at], &rest[at + len..]),
            None => (rest, ""),
        };
        let piece = piece.trim();
        if !piece.is_empty() {
            parts.push(piece.to_owned());
        }
        rest = tail;
    }
    parts
}
