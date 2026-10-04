//! Punycode (RFC 3492) and markdown-it-py's domain mapping: labels with non-ASCII characters are
//! encoded as `xn--...`, nothing else is touched (no case mapping or nameprep).

const BASE: u32 = 36;
const TMIN: u32 = 1;
const TMAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;

fn adapt(mut delta: u32, numpoints: u32, first: bool) -> u32 {
    delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / numpoints;
    let mut k = 0;
    while delta > ((BASE - TMIN) * TMAX) / 2 {
        delta /= BASE - TMIN;
        k += BASE;
    }
    k + (BASE - TMIN + 1) * delta / (delta + SKEW)
}

fn digit(d: u32) -> char {
    (if d < 26 {
        b'a' + d as u8
    } else {
        b'0' + (d - 26) as u8
    }) as char
}

pub fn encode(input: &str) -> Option<String> {
    let cps: Vec<u32> = input.chars().map(|c| c as u32).collect();
    let mut out: String = input.chars().filter(|c| (*c as u32) < 0x80).collect();
    let b = out.len() as u32;
    let mut h = b;
    if b > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias) = (INITIAL_N, 0u32, INITIAL_BIAS);
    while (h as usize) < cps.len() {
        let m = *cps.iter().filter(|c| **c >= n).min()?;
        delta = delta.checked_add((m - n).checked_mul(h + 1)?)?;
        n = m;
        for &c in &cps {
            if c < n {
                delta = delta.checked_add(1)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let t = if k <= bias {
                        TMIN
                    } else if k >= bias + TMAX {
                        TMAX
                    } else {
                        k - bias
                    };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta, h + 1, h == b);
                delta = 0;
                h += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(out)
}

pub fn decode(input: &str) -> Option<String> {
    let (basic, rest) = match input.rfind('-') {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => ("", input),
    };
    let mut out: Vec<u32> = basic.chars().map(|c| c as u32).collect();
    if out.iter().any(|c| *c >= 0x80) {
        return None;
    }
    let (mut n, mut i, mut bias) = (INITIAL_N, 0u32, INITIAL_BIAS);
    let digits: Vec<char> = rest.chars().collect();
    let mut pos = 0;
    while pos < digits.len() {
        let oldi = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            let c = *digits.get(pos)?;
            pos += 1;
            let d = match c {
                'a'..='z' => c as u32 - 'a' as u32,
                'A'..='Z' => c as u32 - 'A' as u32,
                '0'..='9' => c as u32 - '0' as u32 + 26,
                _ => return None,
            };
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = if k <= bias {
                TMIN
            } else if k >= bias + TMAX {
                TMAX
            } else {
                k - bias
            };
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k += BASE;
        }
        let len = out.len() as u32 + 1;
        bias = adapt(i - oldi, len, oldi == 0);
        n = n.checked_add(i / len)?;
        i %= len;
        out.insert(i as usize, n);
        i += 1;
    }
    out.into_iter().map(char::from_u32).collect()
}

fn is_sep(c: char) -> bool {
    matches!(c, '\u{2e}' | '\u{3002}' | '\u{ff0e}' | '\u{ff61}')
}

fn map_domain(s: &str, f: impl Fn(&str) -> String) -> String {
    let (prefix, host) = match s.split_once('@') {
        Some((a, b)) if !b.contains('@') => (format!("{a}@"), b.to_string()),
        Some((a, b)) => {
            // Python: parts = s.split('@'); result = parts[0] + '@'; string = parts[1]
            (
                format!("{a}@"),
                b.split('@').next().unwrap_or("").to_string(),
            )
        }
        None => (String::new(), s.to_string()),
    };
    let labels: Vec<String> = host.split(is_sep).map(&f).collect();
    format!("{prefix}{}", labels.join("."))
}

pub fn to_ascii(s: &str) -> String {
    map_domain(s, |l| {
        if l.chars().any(|c| (c as u32) > 0x7e) {
            encode(l)
                .map(|e| format!("xn--{e}"))
                .unwrap_or_else(|| l.to_string())
        } else {
            l.to_string()
        }
    })
}

pub fn to_unicode(s: &str) -> String {
    map_domain(s, |l| {
        if let Some(rest) = l.strip_prefix("xn--") {
            decode(&rest.to_lowercase()).unwrap_or_else(|| l.to_string())
        } else {
            l.to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        assert_eq!(encode("münchen").unwrap(), "mnchen-3ya");
        assert_eq!(decode("mnchen-3ya").unwrap(), "münchen");
        assert_eq!(to_ascii("bücher.example"), "xn--bcher-kva.example");
        assert_eq!(to_unicode("xn--bcher-kva.example"), "bücher.example");
        assert_eq!(to_ascii("example.com"), "example.com");
    }
}
