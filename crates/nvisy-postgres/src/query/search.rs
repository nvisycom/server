//! Helpers for name search filters.
//!
//! Name search is a hybrid: a case-insensitive substring match (`ILIKE
//! '%term%'`) OR a trigram similarity match (`display_name % term`). The `ILIKE`
//! half gives predictable substring/prefix matching that works for short queries
//! (`"sa"` finds `"sample.txt"`), which trigram similarity alone misses because a
//! two-character query produces too few trigrams to clear the similarity
//! threshold. The trigram half adds typo tolerance (`"smaple"` still finds
//! `"sample"`). Both halves are served by the same `gin_trgm_ops` index.

/// Wraps a user search term as an `ILIKE` "contains" pattern (`%term%`), with the
/// term's `LIKE` metacharacters neutralized so they match literally.
///
/// A raw `%` or `_` in a search term would otherwise act as a wildcard — `"%"`
/// would match every row, `"a_b"` any three-char run — so each `%`, `_`, and the
/// `\` escape character itself is prefixed with `\` (Postgres's default `LIKE`
/// escape). The pattern is a bound parameter, never concatenated into SQL, so
/// this guards against wildcard injection, not SQL injection.
///
/// Escaping is done in one pass — a `\` is emitted immediately before each
/// metacharacter — so there is no order-dependence to get wrong (unlike chained
/// `replace` calls, where the `\`-doubling step must run first).
pub(crate) fn ilike_contains(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for ch in term.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests {
    use super::ilike_contains;

    #[test]
    fn wraps_in_contains_wildcards() {
        assert_eq!(ilike_contains("sa"), "%sa%");
    }

    #[test]
    fn escapes_like_metacharacters() {
        assert_eq!(ilike_contains("50%_off"), "%50\\%\\_off%");
        assert_eq!(ilike_contains("a\\b"), "%a\\\\b%");
    }

    #[test]
    fn empty_term_matches_everything() {
        assert_eq!(ilike_contains(""), "%%");
    }
}
