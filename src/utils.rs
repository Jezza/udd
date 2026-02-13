pub fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return Err("odd number of hex digits".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

pub fn parse_text_with_escapes(s: &str) -> Vec<u8> {
    let mut result = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('x') | Some('X') => {
                    chars.next();
                    let hex: String = chars.by_ref().take(2).collect();
                    if hex.len() == 2 {
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte);
                            continue;
                        }
                    }
                    result.extend(b"\\x");
                    result.extend(hex.as_bytes());
                }
                Some('n') => {
                    chars.next();
                    result.push(b'\n');
                }
                Some('r') => {
                    chars.next();
                    result.push(b'\r');
                }
                Some('t') => {
                    chars.next();
                    result.push(b'\t');
                }
                Some('0') => {
                    chars.next();
                    result.push(0);
                }
                Some('\\') => {
                    chars.next();
                    result.push(b'\\');
                }
                _ => result.push(b'\\'),
            }
        } else {
            result.push(c as u8);
        }
    }
    result
}
