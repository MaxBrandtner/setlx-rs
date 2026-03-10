/* Based on unescape-rs:
 *
 * https://github.com/saghm/unescape-rs.git
 * 
 * Modified to fall back to inserting `\` instead of returning None to comply with the upstream
 * behavior in setlx
 */
use ariadne::ReportKind;
use std::char;
use std::iter::Peekable;
use std::str::CharIndices;

use crate::cst::passes::pass_string::StrCtx;
use crate::diagnostics::report;

// Takes in a string with backslash escapes written out with literal backslash characters and
// converts it to a string with the proper escaped characters.
pub fn unescape(s: &str, ctx: &StrCtx, err_str: &mut String) -> String {
    let mut iter = s.char_indices().peekable();
    let mut s = String::new();

    while let Some((pos, c)) = iter.next() {
        if c != '\\' {
            s.push(c);
            continue;
        }

        let Some((escaped_pos, next)) = iter.next() else {
            if ctx.warn_invalid_backslash {
                report(
                    ReportKind::Warning,
                    "parse error",
                    "trailing backslash",
                    pos + ctx.lhs,
                    pos + ctx.lhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
            }
            s.push('\\');
            break;
        };

        if let Some(res) = match next {
            'b' => Some('\u{0008}'),
            'f' => Some('\u{000C}'),
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            '\'' => Some('\''),
            '\"' => Some('\"'),
            '\\' => Some('\\'),
            'u' => unescape_unicode(&mut iter),
            'x' => unescape_byte(&mut iter),
            c if c.is_digit(8) => unescape_octal(c, &mut iter),
            _ => None,
        } {
            s.push(res);
        } else {
            if ctx.warn_invalid_backslash && next != '$' {
                report(
                    ReportKind::Warning,
                    "parse error",
                    "invalid escape sequence",
                    pos + ctx.lhs,
                    escaped_pos + ctx.lhs,
                    ctx.src,
                    ctx.srcname,
                    err_str,
                );
            }
            if next == '$' {
                s.push('\\');
            }

            s.push(next);
        };
    }

    s
}

fn unescape_unicode(iter: &mut Peekable<CharIndices<'_>>) -> Option<char> {
    let mut clone = iter.clone();
    let mut val: u32 = 0;

    for _ in 0..4 {
        let (_, c) = clone.next()?;
        let d = c.to_digit(16)?;
        val = (val << 4) | d;
    }

    let u = char::from_u32(val)?;
    *iter = clone;

    Some(u)
}

fn unescape_byte(iter: &mut Peekable<CharIndices<'_>>) -> Option<char> {
    let mut clone = iter.clone();
    let mut val: u32 = 0;

    for _ in 0..2 {
        let (_, c) = clone.next()?;
        let d = c.to_digit(16)?;
        val = (val << 4) | d;
    }

    if val > 0xff {
        return None;
    }

    let u = char::from_u32(val)?;
    *iter = clone;

    Some(u)
}

fn unescape_octal(first: char, iter: &mut Peekable<CharIndices<'_>>) -> Option<char> {
    if !first.is_digit(8) {
        return None;
    }

    let mut clone = iter.clone();
    let mut value = first.to_digit(8)?; // first digit already consumed

    // True C: consume up to 2 more octal digits (max 3 total)
    for _ in 0..2 {
        match clone.peek() {
            Some(&(_, c)) if c.is_digit(8) => {
                let (_, c) = clone.next().unwrap();
                value = (value << 3) | c.to_digit(8)?;
            }
            _ => break,
        }
    }

    // Truncate to a single byte, like C
    value &= 0xFF;

    let ch = char::from_u32(value)?;

    // commit only if fully valid
    *iter = clone;

    Some(ch)
}
