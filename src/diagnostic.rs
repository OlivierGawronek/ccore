use crate::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            message: message.into(),
            span,
        }
    }

    /// Formats the error rustc-style, with the offending line and a caret underline.
    pub fn render(&self, path: &str, src: &str) -> String {
        let start = floor_char_boundary(src, self.span.start);
        let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
        let end = floor_char_boundary(src, self.span.end.clamp(start, line_end));
        let line_no = src[..start].matches('\n').count() + 1;
        let line = src[line_start..line_end].trim_end_matches('\r');

        // Tabs are printed as four spaces so the carets line up.
        let width = |s: &str| {
            s.chars()
                .map(|c| if c == '\t' { 4 } else { 1 })
                .sum::<usize>()
        };
        let col = src[line_start..start].chars().count() + 1;
        let pad = width(&src[line_start..start]);
        let underline_len = width(&src[start..end]).max(1);
        let gutter = " ".repeat(line_no.to_string().len());

        format!(
            "error: {msg}\n{gutter}--> {path}:{line_no}:{col}\n{gutter} |\n{line_no} | {line}\n{gutter} | {pad}{carets}\n",
            msg = self.message,
            line = line.replace('\t', "    "),
            pad = " ".repeat(pad),
            carets = "^".repeat(underline_len),
        )
    }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_points_at_the_right_column() {
        let src = "fn main() {\n    x = 5;\n}";
        let diag = Diagnostic::new("unknown variable `x`", Span { start: 16, end: 17 });
        let out = diag.render("test.ccr", src);
        assert_eq!(
            out,
            "error: unknown variable `x`\n --> test.ccr:2:5\n  |\n2 |     x = 5;\n  |     ^\n"
        );
    }

    #[test]
    fn render_handles_tabs_and_crlf() {
        let src = "fn main() {\r\n\tx = 5;\r\n}";
        let diag = Diagnostic::new("unknown variable `x`", Span { start: 14, end: 15 });
        let out = diag.render("test.ccr", src);
        assert_eq!(
            out,
            "error: unknown variable `x`\n --> test.ccr:2:2\n  |\n2 |     x = 5;\n  |     ^\n"
        );
    }

    #[test]
    fn render_never_splits_a_character() {
        let src = "żółw";
        let diag = Diagnostic::new("oops", Span { start: 1, end: 3 });
        assert!(diag.render("test.ccr", src).contains("1 | żółw"));
    }
}
