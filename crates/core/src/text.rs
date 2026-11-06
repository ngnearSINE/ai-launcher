/// Folder-safe slug from a user alias: ASCII letters/digits kept, everything else becomes `-`.
/// Non-ASCII aliases (e.g. Korean) fall back to the instance id so folder names stay portable.
pub fn slug(alias: &str, fallback: &str) -> String {
    let mut out = String::new();
    for c in alias.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.len() < 2 {
        fallback.to_string()
    } else {
        out.chars().take(32).collect()
    }
}

/// Picks `base`, `base-2`, `base-3`… so the result is not in `taken`.
pub fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|c| !taken(c))
        .expect("unbounded range")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_keeps_ascii() {
        assert_eq!(slug("Work Account #2", "x"), "Work-Account-2");
        assert_eq!(slug("업무", "a1b2"), "a1b2");
        assert_eq!(unique("a", |s| s == "a" || s == "a-2"), "a-3");
    }
}
