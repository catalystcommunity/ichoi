//! Minimal m3u playlist read/write. Entries are server-root-relative paths (§7), which is
//! what makes a playlist portable across servers that share a collection layout.

/// Parse an m3u into its ordered list of entry paths, ignoring blank lines and `#` directives.
pub fn parse(contents: &str) -> Vec<String> {
    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Serialize entries into an `#EXTM3U` document.
pub fn write(entries: &[String]) -> String {
    let mut out = String::from("#EXTM3U\n");
    for entry in entries {
        out.push_str(entry);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_entries() {
        let entries = vec![
            "Artist/Album/01 One.flac".to_string(),
            "Artist/Album/02 Two.flac".to_string(),
        ];
        let text = write(&entries);
        assert_eq!(parse(&text), entries);
    }

    #[test]
    fn skips_directives_and_blanks() {
        let text = "#EXTM3U\n\n#EXTINF:123,Song\nA/B.mp3\n";
        assert_eq!(parse(text), vec!["A/B.mp3".to_string()]);
    }
}
