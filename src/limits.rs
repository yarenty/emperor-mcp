//! Output and input caps every tool should apply: characters returned, results per call, and
//! bytes downloaded or read from a file.
//!
//! A tool result lands in the model's context. An unbounded one (a 5 MB page, a 2 000-row
//! search) costs the whole conversation and pushes the user's own words out. [`Limits`] holds the
//! caps with safe defaults; every setter clamps to a hard ceiling, so a configuration value or a
//! model-supplied argument can lower a cap but never lift it past the ceiling.
//!
//! ```
//! use emperor_mcp::limits::{Limits, truncate_chars};
//! let limits = Limits::default().with_max_results(50);
//! assert_eq!(limits.clamp_results(Some(500)), 50); // the model asked for 500
//! assert_eq!(limits.clamp_results(None), 50);
//! let text = truncate_chars("abcdef", 3);
//! assert!(text.starts_with("abc") && text.contains("truncated"));
//! ```

use std::borrow::Cow;

/// Default for characters of text a tool returns.
pub const DEFAULT_MAX_CHARS: usize = 50_000;
/// Hard ceiling for characters of text a tool returns.
pub const MAX_CHARS_CEILING: usize = 200_000;
/// Default for results (rows, hits, messages) a tool returns per call.
pub const DEFAULT_MAX_RESULTS: usize = 20;
/// Hard ceiling for results a tool returns per call.
pub const MAX_RESULTS_CEILING: usize = 100;
/// Default for bytes a tool downloads or reads from one file: 8 MiB.
pub const DEFAULT_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// Hard ceiling for bytes a tool downloads or reads from one file: 64 MiB.
pub const MAX_BYTES_CEILING: u64 = 64 * 1024 * 1024;

/// The caps one server applies. Construct with [`Limits::default`] and adjust with the `with_*`
/// setters, which clamp to `1..=ceiling`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    max_chars: usize,
    max_results: usize,
    max_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_chars: DEFAULT_MAX_CHARS,
            max_results: DEFAULT_MAX_RESULTS,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

impl Limits {
    /// Set the character cap, clamped to `1..=`[`MAX_CHARS_CEILING`].
    pub fn with_max_chars(mut self, n: usize) -> Self {
        self.max_chars = n.clamp(1, MAX_CHARS_CEILING);
        self
    }

    /// Set the results-per-call cap, clamped to `1..=`[`MAX_RESULTS_CEILING`].
    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n.clamp(1, MAX_RESULTS_CEILING);
        self
    }

    /// Set the byte cap, clamped to `1..=`[`MAX_BYTES_CEILING`].
    pub fn with_max_bytes(mut self, n: u64) -> Self {
        self.max_bytes = n.clamp(1, MAX_BYTES_CEILING);
        self
    }

    /// The character cap.
    pub fn max_chars(&self) -> usize {
        self.max_chars
    }

    /// The results-per-call cap.
    pub fn max_results(&self) -> usize {
        self.max_results
    }

    /// The byte cap.
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// How many results to return for a requested count (e.g. a `limit` tool argument):
    /// the cap when none was asked for, otherwise the request clamped to `1..=max_results`.
    pub fn clamp_results(&self, requested: Option<usize>) -> usize {
        requested.map_or(self.max_results, |n| n.clamp(1, self.max_results))
    }

    /// How many characters to return for a requested count, like [`Self::clamp_results`].
    pub fn clamp_chars(&self, requested: Option<usize>) -> usize {
        requested.map_or(self.max_chars, |n| n.clamp(1, self.max_chars))
    }

    /// True when `len` bytes fit under the byte cap. Check the declared length (e.g.
    /// `Content-Length`) before downloading, and count while streaming.
    pub fn bytes_allowed(&self, len: u64) -> bool {
        len <= self.max_bytes
    }

    /// [`truncate_chars`] with this cap.
    pub fn truncate<'a>(&self, text: &'a str) -> Cow<'a, str> {
        truncate_chars(text, self.max_chars)
    }
}

/// Cut `text` to its first `max_chars` characters (Unicode scalar values, never mid-character)
/// and append a note saying how much was left out, so the model knows the content is
/// incomplete. Text that fits is returned unchanged and unallocated. The note is added after the
/// cut, so the result is at most `max_chars` plus the note's length.
pub fn truncate_chars(text: &str, max_chars: usize) -> Cow<'_, str> {
    match text.char_indices().nth(max_chars) {
        None => Cow::Borrowed(text),
        Some((cut, _)) => {
            let total = text.chars().count();
            Cow::Owned(format!(
                "{}\n\n[truncated: showing the first {max_chars} of {total} characters]",
                &text[..cut]
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let l = Limits::default();
        assert_eq!(l.max_chars(), 50_000);
        assert_eq!(l.max_results(), 20);
        assert_eq!(l.max_bytes(), 8 * 1024 * 1024);
    }

    #[test]
    fn setters_clamp_to_ceilings() {
        let l = Limits::default()
            .with_max_chars(1_000_000)
            .with_max_results(10_000)
            .with_max_bytes(u64::MAX);
        assert_eq!(l.max_chars(), MAX_CHARS_CEILING);
        assert_eq!(l.max_results(), MAX_RESULTS_CEILING);
        assert_eq!(l.max_bytes(), MAX_BYTES_CEILING);
        let z = Limits::default()
            .with_max_chars(0)
            .with_max_results(0)
            .with_max_bytes(0);
        assert_eq!((z.max_chars(), z.max_results(), z.max_bytes()), (1, 1, 1));
    }

    #[test]
    fn requested_counts_are_clamped() {
        let l = Limits::default();
        assert_eq!(l.clamp_results(None), 20);
        assert_eq!(l.clamp_results(Some(5)), 5);
        assert_eq!(l.clamp_results(Some(0)), 1);
        assert_eq!(l.clamp_results(Some(1_000)), 20);
        assert_eq!(l.clamp_chars(Some(10)), 10);
        assert_eq!(l.clamp_chars(Some(usize::MAX)), 50_000);
    }

    #[test]
    fn byte_cap() {
        let l = Limits::default().with_max_bytes(10);
        assert!(l.bytes_allowed(10));
        assert!(!l.bytes_allowed(11));
    }

    #[test]
    fn short_text_is_borrowed_unchanged() {
        assert!(matches!(truncate_chars("hello", 5), Cow::Borrowed("hello")));
        assert!(matches!(truncate_chars("", 0), Cow::Borrowed("")));
    }

    #[test]
    fn long_text_is_cut_with_a_note() {
        let out = truncate_chars("hello world", 5);
        assert!(out.starts_with("hello\n\n[truncated:"), "{out}");
        assert!(out.contains("first 5 of 11 characters"), "{out}");
        assert!(!out.contains("world"));
    }

    #[test]
    fn cut_respects_multibyte_characters() {
        let out = truncate_chars("żółw🐢abc", 4);
        assert!(out.starts_with("żółw\n"), "{out}");
        assert!(out.contains("first 4 of 8 characters"), "{out}");
        let limited = Limits::default().with_max_chars(5).truncate("żółw🐢abc");
        assert!(limited.starts_with("żółw🐢\n"), "{limited}");
    }
}
