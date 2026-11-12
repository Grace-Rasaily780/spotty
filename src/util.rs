use std::process::Command;
pub(crate) fn mmss(s: u64) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

pub(crate) fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Channel name with " - Topic" / "VEVO" removed, normalised, so all channels of one artist share a key.
pub(crate) fn key(s: &str) -> String {
    let l = s.to_lowercase();
    norm(l.trim_end_matches(" - topic").trim_end_matches("vevo"))
}

pub(crate) fn fix_url(u: &str) -> String {
    if u.starts_with("//") {
        format!("https:{u}")
    } else {
        u.to_string()
    }
}

pub(crate) fn base_title(s: &str) -> &str {
    s.split(" (")
        .next()
        .unwrap_or(s)
        .split(" - ")
        .next()
        .unwrap_or(s)
        .trim()
}

pub(crate) fn ell(s: &str, max_w: f32, size: f32) -> String {
    let n = (max_w / (size * 0.56)).max(3.0) as usize;
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>().trim_end())
    }
}

pub(crate) fn short_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

/// Spawn a helper program without flashing a console window on Windows.
pub(crate) fn quiet(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_helpers() {
        assert_eq!(mmss(326), "5:26");
        assert_eq!(short_num(608_776), "608.8K");
        assert_eq!(urlenc("a b&c"), "a+b%26c");
        // all channels of one artist share a key
        assert_eq!(key("The 1975 - Topic"), key("The1975VEVO"));
        assert_eq!(norm("About You!"), "aboutyou");
        assert_eq!(
            base_title("About You (Live from MSG) - Remastered"),
            "About You"
        );
        assert_eq!(ell("short", 200.0, 14.0), "short");
        assert!(ell(&"x".repeat(80), 100.0, 14.0).ends_with('…'));
    }
}
