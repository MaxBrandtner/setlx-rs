/* Based on unescape-rs:
 *
 * https://github.com/saghm/unescape-rs.git
 * 
 * Modified to fall back to inserting `\` instead of returning None to comply with the upstream
 * behavior in setlx
 */

use std::collections::VecDeque;

use std::char;

macro_rules! try_option {
    ($o:expr) => {
        match $o {
            Some(s) => s,
            None => return None,
        }
    }
}

// Takes in a string with backslash escapes written out with literal backslash characters and
// converts it to a string with the proper escaped characters.
pub fn unescape(s: &str) -> Option<String> {
    let mut queue : VecDeque<_> = String::from(s).chars().collect();
    let mut s = String::new();

    while let Some(c) = queue.pop_front() {
        if c != '\\' {
            s.push(c);
            continue;
        }

        if let Some(res) = match queue.pop_front() {
            Some('b') => Some('\u{0008}'),
            Some('f') => Some('\u{000C}'),
            Some('n') => Some('\n'),
            Some('r') => Some('\r'),
            Some('t') => Some('\t'),
            Some('\'') => Some('\''),
            Some('\"') => Some('\"'),
            Some('\\') => Some('\\'),
            Some('u') => unescape_unicode(&mut queue),
            Some('x') => unescape_byte(&mut queue),
            Some(c) if c.is_digit(8) => unescape_octal(c, &mut queue),
            _ => None
        } {
            s.push(res);
        } else {
            s.push('\\');
        };
    }

    Some(s)
}

fn unescape_unicode(queue: &VecDeque<char>) -> Option<char> {
    let mut s = String::new();

    for i in 0..4 {
        s.push(*try_option!(queue.get(i)));
    }

    let u = try_option!(u32::from_str_radix(&s, 16).ok());
    char::from_u32(u)
}

fn unescape_byte(queue: &VecDeque<char>) -> Option<char> {
    let mut s = String::new();

    for i in 0..2 {
        s.push(*try_option!(queue.get(i)));
    }

    let u = try_option!(u32::from_str_radix(&s, 16).ok());
    char::from_u32(u)
}

fn unescape_octal(c: char, queue: &VecDeque<char>) -> Option<char> {
    match unescape_octal_leading(c, queue) {
        Some(ch) => {
            Some(ch)
        }
        None => unescape_octal_no_leading(c, queue)
    }
}

fn unescape_octal_leading(c: char, queue: &VecDeque<char>) -> Option<char> {
    if c != '0' && c != '1' && c != '2' && c != '3' {
        return None;
    }

    let mut s = String::new();
    s.push(c);
    s.push(*try_option!(queue.get(0)));
    s.push(*try_option!(queue.get(1)));

    let u = try_option!(u32::from_str_radix(&s, 8).ok());
    char::from_u32(u)
}

fn unescape_octal_no_leading(c: char, queue: &VecDeque<char>) -> Option<char> {
    let mut s = String::new();
    s.push(c);
    s.push(*try_option!(queue.get(0)));

    let u = try_option!(u32::from_str_radix(&s, 8).ok());
    char::from_u32(u)
}
