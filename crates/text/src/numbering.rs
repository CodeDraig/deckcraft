//! Text labels for automatic paragraph numbering.

pub(super) fn roman(mut n: u32, upper: bool) -> String {
    let table = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut s = String::new();
    if n == 0 || n > 3999 {
        return n.to_string();
    }
    for (v, r) in table {
        while n >= v {
            s.push_str(r);
            n -= v;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

pub(super) fn alpha_num(n: u32, upper: bool) -> String {
    if n == 0 {
        return String::new();
    }
    let mut n = n - 1;
    let c = (b'a' + (n % 26) as u8) as char;
    let reps = n / 26 + 1;
    n = reps;
    let s: String = std::iter::repeat_n(c, n as usize).collect();
    if upper { s.to_uppercase() } else { s }
}

/// Text of an auto-number bullet: `arabicPeriod` 3 → "3.".
pub fn autonum(scheme: &str, n: u32) -> String {
    let core = if scheme.starts_with("romanUc") {
        roman(n, true)
    } else if scheme.starts_with("romanLc") {
        roman(n, false)
    } else if scheme.starts_with("alphaUc") {
        alpha_num(n, true)
    } else if scheme.starts_with("alphaLc") {
        alpha_num(n, false)
    } else if scheme.starts_with("circleNum") {
        let c = char::from_u32(0x2460 + n.saturating_sub(1).min(19)).unwrap_or('•');
        return c.to_string();
    } else {
        n.to_string()
    };
    if scheme.ends_with("ParenBoth") {
        format!("({core})")
    } else if scheme.ends_with("ParenR") {
        format!("{core})")
    } else if scheme.ends_with("Period") {
        format!("{core}.")
    } else if scheme.ends_with("Minus") {
        format!("- {core} -")
    } else {
        core
    }
}
