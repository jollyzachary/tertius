use regex::{Captures, Regex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritingContext {
    Neutral,
    Message,
    Email,
    Notes,
    Development,
}

impl WritingContext {
    pub fn for_app(app_name: Option<&str>) -> Self {
        let name = app_name.unwrap_or_default().to_ascii_lowercase();
        if contains_any(
            &name,
            &[
                "terminal", "iterm", "warp", "code", "cursor", "codex", "zed", "xcode", "intellij",
            ],
        ) {
            Self::Development
        } else if contains_any(
            &name,
            &[
                "slack", "discord", "messages", "teams", "whatsapp", "telegram", "signal",
            ],
        ) {
            Self::Message
        } else if contains_any(&name, &["mail", "outlook", "thunderbird", "spark"]) {
            Self::Email
        } else if contains_any(
            &name,
            &[
                "notes",
                "notion",
                "obsidian",
                "word",
                "pages",
                "bear",
                "drafts",
                "libreoffice",
            ],
        ) {
            Self::Notes
        } else {
            Self::Neutral
        }
    }
}

fn contains_any(value: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| value.contains(candidate))
}

#[derive(Clone, Debug)]
pub struct CleanupResult {
    pub text: String,
    pub press_enter: bool,
}

pub struct CleanupPipeline {
    filler: Regex,
    whitespace: Regex,
    space_before_punctuation: Regex,
    line_space: Regex,
    spoken_formatting: Vec<(Regex, &'static str)>,
    bullet_marker: Regex,
    numbered_marker: Regex,
}

impl Default for CleanupPipeline {
    fn default() -> Self {
        Self {
            filler: Regex::new(r"(?i)\b(?:um+|uh+|erm+)\b[, \t]*").unwrap(),
            whitespace: Regex::new(r"[ \t]{2,}").unwrap(),
            space_before_punctuation: Regex::new(r"[ \t]+([,.;!?])").unwrap(),
            line_space: Regex::new(r"[ \t]*\n[ \t]*").unwrap(),
            spoken_formatting: vec![
                (Regex::new(r"(?i)\bnew paragraph\b").unwrap(), "\n\n"),
                (Regex::new(r"(?i)\b(?:new line|line break)\b").unwrap(), "\n"),
                (Regex::new(r"(?i)\b(?:insert|type) (?:a )?comma\b").unwrap(), ","),
                (Regex::new(r"(?i)\b(?:insert|type) (?:a )?(?:full stop|period)\b").unwrap(), "."),
                (Regex::new(r"(?i)\b(?:insert|type) (?:a )?question mark\b").unwrap(), "?"),
                (Regex::new(r"(?i)\b(?:insert|type) (?:an )?exclamation (?:mark|point)\b").unwrap(), "!"),
                (Regex::new(r"(?i)\b(?:insert|type) (?:a )?colon\b").unwrap(), ":"),
                (Regex::new(r"(?i)\b(?:insert|type) (?:a )?semicolon\b").unwrap(), ";"),
            ],
            bullet_marker: Regex::new(r"(?i)\b(?:bullet point|next item)\b").unwrap(),
            numbered_marker: Regex::new(
                r"(?i)\b(?:number (?:one|two|three|four|five|six|seven|eight|nine|ten|\d+)|(?:first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth) item)\b",
            )
            .unwrap(),
        }
    }
}

impl CleanupPipeline {
    pub fn run(&self, raw: &str, context: WritingContext) -> CleanupResult {
        let (mut text, press_enter) = strip_press_enter(raw.trim());
        text = self.apply_backtrack(text);

        for (pattern, replacement) in &self.spoken_formatting {
            text = pattern.replace_all(&text, *replacement).into_owned();
        }
        text = self.filler.replace_all(&text, " ").into_owned();
        text = self
            .space_before_punctuation
            .replace_all(&text, "$1")
            .into_owned();
        text = self.whitespace.replace_all(&text, " ").into_owned();
        text = self.line_space.replace_all(&text, "\n").into_owned();
        text = collapse_blank_lines(&text);

        let (formatted, is_list) = self.format_explicit_list(text.trim());
        text = formatted;
        if context != WritingContext::Development {
            text = sentence_case_lines(&text);
        }
        if !is_list && matches!(context, WritingContext::Email | WritingContext::Notes) {
            add_terminal_punctuation(&mut text);
        }

        CleanupResult {
            text: text.trim().to_owned(),
            press_enter,
        }
    }

    fn apply_backtrack(&self, text: String) -> String {
        let lower = text.to_lowercase();
        let Some(marker) = lower.rfind("scratch that") else {
            return text;
        };
        let before = text[..marker].trim_end();
        let after = text[marker + "scratch that".len()..].trim_start();
        let boundary = before
            .rfind(['.', '!', '?', '\n'])
            .map(|index| index + 1)
            .unwrap_or(0);
        let mut result = before[..boundary].trim_end().to_owned();
        if !result.is_empty() && !after.is_empty() {
            result.push(' ');
        }
        result.push_str(after);
        result
    }

    fn format_explicit_list(&self, text: &str) -> (String, bool) {
        if let Some(items) = split_marked_items(text, &self.bullet_marker) {
            return (
                items
                    .into_iter()
                    .map(|item| {
                        format!(
                            "• {}",
                            item.trim_matches(|c: char| c == ',' || c.is_whitespace())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                true,
            );
        }
        if let Some(items) = split_marked_items(text, &self.numbered_marker) {
            return (
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| {
                        format!(
                            "{}. {}",
                            index + 1,
                            item.trim_matches(|c: char| c == ',' || c.is_whitespace())
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                true,
            );
        }
        (text.to_owned(), false)
    }
}

fn split_marked_items<'a>(text: &'a str, marker: &Regex) -> Option<Vec<&'a str>> {
    let matches = marker.find_iter(text).collect::<Vec<_>>();
    if matches.len() < 2 {
        return None;
    }
    let items = matches
        .iter()
        .enumerate()
        .filter_map(|(index, current)| {
            let end = matches
                .get(index + 1)
                .map_or(text.len(), |next| next.start());
            let item = text[current.end()..end].trim();
            (!item.is_empty()).then_some(item)
        })
        .collect::<Vec<_>>();
    (items.len() >= 2).then_some(items)
}

fn strip_press_enter(text: &str) -> (String, bool) {
    let pattern = Regex::new(r"(?i)(?:[.!?]\s*)?\bpress enter\b[.!?]?\s*$").unwrap();
    if !pattern.is_match(text) {
        return (text.to_owned(), false);
    }
    let stripped = pattern.replace(text, |caps: &Captures| {
        caps.get(0)
            .and_then(|item| {
                item.as_str()
                    .chars()
                    .find(|character| matches!(character, '.' | '!' | '?'))
            })
            .map(|character| character.to_string())
            .unwrap_or_default()
    });
    (stripped.trim().to_owned(), true)
}

fn sentence_case_lines(text: &str) -> String {
    text.lines()
        .map(sentence_case)
        .collect::<Vec<_>>()
        .join("\n")
}

fn sentence_case(text: &str) -> String {
    let Some((index, first)) = text
        .char_indices()
        .find(|(_, character)| character.is_alphabetic())
    else {
        return text.to_owned();
    };
    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..index]);
    result.extend(first.to_uppercase());
    result.push_str(&text[index + first.len_utf8()..]);
    result
}

fn add_terminal_punctuation(text: &mut String) {
    if text.split_whitespace().count() > 3
        && !text
            .chars()
            .last()
            .is_some_and(|character| matches!(character, '.' | '!' | '?' | ':' | ';' | '\n'))
    {
        text.push('.');
    }
}

fn collapse_blank_lines(text: &str) -> String {
    let mut output = Vec::new();
    let mut previous_blank = false;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if !blank || !previous_blank {
            output.push(line);
        }
        previous_blank = blank;
    }
    output.join("\n")
}
