//! The `preg_*` calls of the `SQLite` schema introspection, over
//! php2rust-regex (PHP's patterns translated to fancy-regex).
//!
//! The subjects are SQL text that `SQLite` returns as UTF-8; the patterns
//! have no `/u` modifier, so they match bytes, as in PHP. A pattern that
//! the translation refuses, or a match that fails (the backtracking limit,
//! PHP's `false`), is reported as an exception rather than as PHP's
//! warning and `false`.
//!
//! Upstream: none

use php_regex::{Groups, Pcre};

use crate::exception::Exception;

fn compile(pattern: &str) -> Result<Pcre, Exception> {
    Pcre::new(pattern.as_bytes()).map_err(|e| Exception::new(Exception::UNSUPPORTED, e.to_string()))
}

fn captures(pcre: &Pcre, subject: &str, offset: usize) -> Result<Option<Groups>, Exception> {
    pcre.captures_from(subject.as_bytes(), offset)
        .map_err(|e| Exception::new(Exception::UNSUPPORTED, e.0))
}

fn group_text(subject: &str, group: Option<(usize, usize)>) -> Option<String> {
    group.map(|(start, end)| String::from_utf8_lossy(&subject.as_bytes()[start..end]).into_owned())
}

/// `preg_match($pattern, $subject, $matches)`: the groups of the first
/// match (`None` for a group that did not take part), or `None` for no
/// match. PHP leaves out trailing groups that did not take part and gives
/// `""` for the others; [`group`] reads them as `isset()` does.
pub(crate) fn preg_match(
    pattern: &str,
    subject: &str,
) -> Result<Option<Vec<Option<String>>>, Exception> {
    let pcre = compile(pattern)?;
    Ok(captures(&pcre, subject, 0)?.map(|groups| {
        groups
            .into_iter()
            .map(|group| group_text(subject, group))
            .collect()
    }))
}

/// `isset($matches[$n])` and `$matches[$n]` after [`preg_match`]: a group
/// that did not take part is `""` when a later group did, and unset
/// otherwise.
pub(crate) fn group(matches: &[Option<String>], n: usize) -> Option<&str> {
    match matches.get(n) {
        Some(Some(text)) => Some(text),
        Some(None) if matches[n + 1..].iter().any(Option::is_some) => Some(""),
        _ => None,
    }
}

/// `preg_match_all($pattern, $subject, $matches)` (`PREG_PATTERN_ORDER`):
/// `$matches[$group][$match]`, `""` for a group that did not take part.
pub(crate) fn preg_match_all(pattern: &str, subject: &str) -> Result<Vec<Vec<String>>, Exception> {
    let pcre = compile(pattern)?;
    let mut matches: Vec<Vec<String>> = vec![Vec::new(); pcre.group_count()];
    let mut offset = 0;
    while offset <= subject.len() {
        let Some(groups) = captures(&pcre, subject, offset)? else {
            break;
        };
        let Some((start, end)) = groups[0] else {
            break;
        };
        for (n, group) in groups.into_iter().enumerate() {
            matches[n].push(group_text(subject, group).unwrap_or_default());
        }
        offset = if end == start { end + 1 } else { end };
    }
    Ok(matches)
}

/// `preg_replace($pattern, $replacement, $subject)`.
pub(crate) fn preg_replace(
    pattern: &str,
    replacement: &str,
    subject: &str,
) -> Result<String, Exception> {
    let pcre = compile(pattern)?;
    let replaced = pcre
        .replace(subject.as_bytes(), replacement.as_bytes())
        .map_err(|e| Exception::new(Exception::UNSUPPORTED, e.0))?;
    Ok(String::from_utf8_lossy(&replaced).into_owned())
}

/// `preg_quote($str, $delimiter)`.
pub(crate) fn preg_quote(s: &str, delimiter: char) -> String {
    let delimiter = u8::try_from(delimiter).ok();
    String::from_utf8_lossy(&php_regex::preg_quote(s.as_bytes(), delimiter)).into_owned()
}

/// `rtrim($s, "\n")`.
pub(crate) fn rtrim_newlines(s: &str) -> &str {
    s.trim_end_matches('\n')
}
