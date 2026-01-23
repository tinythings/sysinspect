/// Renders simple markup to ANSI escape codes for terminal output.
/// Markup syntax:
/// - `[fg:bg:attrs]` where:
///   - `fg` is a single character for foreground color (e.g. 'r' for red)
///   - `bg` is a single character for background color (e.g. 'b' for blue)
///   - `attrs` is a string of characters for attributes (e.g. 'bu' for bold and underline)
/// - `[N]` resets all attributes
/// - Unknown tags are rendered literally
/// # Supported colors:
/// - k: black
/// - r: red
/// - g: green
/// - y: yellow
/// - b: blue
/// - m: magenta
/// - c: cyan
/// - w: white
/// - K: bright black (gray)
/// - R: bright red
/// - G: bright green
/// - Y: bright yellow
/// - B: bright blue
/// - M: bright magenta
/// - C: bright cyan
/// - W: bright white
/// # Supported attributes:
/// - b: bold
/// - d: dim
/// - u: underline
/// - i: inverse
/// - s: strikethrough
/// # Arguments
/// * `input` - Input string with markup
/// # Returns
/// * `String` - Output string with ANSI escape codes
/// # Example
/// ```no_run
/// let rendered = render_markup("This is [r::b]red text on default background[N] and this is [::bu]bold underlined text[N].");
/// println!("{}", rendered);
/// ```
pub fn render_markup(input: &str) -> String {
    fn fg_code(c: char) -> Option<&'static str> {
        Some(match c {
            'k' => "\x1b[30m",
            'r' => "\x1b[31m",
            'g' => "\x1b[32m",
            'y' => "\x1b[33m",
            'b' => "\x1b[34m",
            'm' => "\x1b[35m",
            'c' => "\x1b[36m",
            'w' => "\x1b[37m",
            'K' => "\x1b[90m",
            'R' => "\x1b[91m",
            'G' => "\x1b[92m",
            'Y' => "\x1b[93m",
            'B' => "\x1b[94m",
            'M' => "\x1b[95m",
            'C' => "\x1b[96m",
            'W' => "\x1b[97m",
            _ => return None,
        })
    }

    fn bg_code(c: char) -> Option<&'static str> {
        Some(match c {
            'k' => "\x1b[40m",
            'r' => "\x1b[41m",
            'g' => "\x1b[42m",
            'y' => "\x1b[43m",
            'b' => "\x1b[44m",
            'm' => "\x1b[45m",
            'c' => "\x1b[46m",
            'w' => "\x1b[47m",
            'K' => "\x1b[100m",
            'R' => "\x1b[101m",
            'G' => "\x1b[102m",
            'Y' => "\x1b[103m",
            'B' => "\x1b[104m",
            'M' => "\x1b[105m",
            'C' => "\x1b[106m",
            'W' => "\x1b[107m",
            _ => return None,
        })
    }

    fn attr_code(c: char) -> Option<&'static str> {
        Some(match c {
            'b' => "\x1b[1m", // bold
            'd' => "\x1b[2m", // dim
            'u' => "\x1b[4m", // underline
            'i' => "\x1b[7m", // inverse
            's' => "\x1b[9m", // strikethrough
            _ => return None,
        })
    }

    let mut out = String::with_capacity(input.len() + 16);
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '[' {
            out.push(ch);
            continue;
        }

        // Collect tag until ']'
        let mut tag = String::new();
        while let Some(&c) = chars.peek() {
            chars.next();
            if c == ']' {
                break;
            }
            tag.push(c);
        }

        // If we didn't end with ']', treat as literal
        if !tag.ends_with([]) && !input.contains(']') {
            out.push('[');
            out.push_str(&tag);
            continue;
        }

        // Special reset tag
        if tag == "N" {
            out.push_str("\x1b[0m");
            continue;
        }

        // Parse fg:bg:attrs
        // - fg and bg are optional (empty allowed)
        // - attrs can be multiple letters like "bu"
        let mut parts = tag.splitn(3, ':');
        let fg = parts.next().unwrap_or("");
        let bg = parts.next().unwrap_or("");
        let attrs = parts.next().unwrap_or("");

        // If no ':' at all, treat literal (avoid eating user's text)
        if !tag.contains(':') {
            out.push('[');
            out.push_str(&tag);
            out.push(']');
            continue;
        }

        // Apply: attributes first or last doesn't really matter with ANSI,
        // but we’ll do attrs, then fg, then bg.
        let mut applied = false;

        for a in attrs.chars() {
            if let Some(code) = attr_code(a) {
                out.push_str(code);
                applied = true;
            }
        }

        if let Some(c) = fg.chars().next()
            && let Some(code) = fg_code(c)
        {
            out.push_str(code);
            applied = true;
        }

        if let Some(c) = bg.chars().next()
            && let Some(code) = bg_code(c)
        {
            out.push_str(code);
            applied = true;
        }

        // If nothing applied (unknown tag), render literally
        if !applied {
            out.push('[');
            out.push_str(&tag);
            out.push(']');
        }
    }

    out
}

/// Indent each line of the given string with the specified prefix.
/// # Arguments
/// * `s` - Input string to indent
/// * `prefix` - Prefix string to add to each line
/// # Returns
/// * `String` - Indented string
/// # Example
/// ```no_run
/// let indented = indent_block("Line 1\nLine 2\nLine 3", ">> ");
/// println!("{}", indented);
/// ```
pub fn indent_block(s: &str, prefix: &str) -> String {
    s.lines().map(|line| if line.is_empty() { prefix.to_string() } else { format!("{prefix}{line}") }).collect::<Vec<_>>().join("\n")
}
