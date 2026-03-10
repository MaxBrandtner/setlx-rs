use ariadne::{Color, Config, Label, Report, ReportKind, Source};
use lalrpop_util::ParseError;
use std::io::{self, IsTerminal};

pub fn report(
    kind: ReportKind,
    cat_msg: &str,
    msg: &str,
    lhs: usize,
    rhs: usize,
    src: &str,
    srcname: &str,
    output: &mut String,
) {
    let mut buf = Vec::new();

    Report::build(kind, (srcname, lhs..rhs))
        .with_config(Config::default().with_color(io::stdout().is_terminal()))
        .with_message(cat_msg)
        .with_label(
            Label::new((srcname, lhs..rhs))
                .with_message(msg)
                .with_color(match kind {
                    ReportKind::Error => Color::Red,
                    ReportKind::Warning => Color::Yellow,
                    ReportKind::Advice => Color::Cyan,
                    ReportKind::Custom(_, c) => c,
                }),
        )
        .finish()
        .write((srcname, Source::from(src)), &mut buf)
        .unwrap();

    output.push_str(&String::from_utf8(buf).unwrap());
}

pub fn report_parse_error<Tok, E>(
    err: ParseError<usize, Tok, E>,
    src: &str,
    srcname: &str,
    output: &mut String,
) where
    Tok: std::fmt::Debug,
    E: std::fmt::Display,
{
    let mut buf = Vec::new();

    match err {
        ParseError::InvalidToken { location } => {
            Report::build(ReportKind::Error, (srcname, location..location))
                .with_config(Config::default().with_color(io::stdout().is_terminal()))
                .with_message("Invalid token")
                .with_label(
                    Label::new((srcname, location..location + 1))
                        .with_message("Unexpected token here")
                        .with_color(Color::Red),
                )
                .finish()
                .write((srcname, Source::from(src)), &mut buf)
                .unwrap();
        }

        ParseError::UnrecognizedEof { location, expected } => {
            Report::build(ReportKind::Error, (srcname, location..location))
                .with_config(Config::default().with_color(io::stdout().is_terminal()))
                .with_message("Unexpected end of input")
                .with_label(
                    Label::new((srcname, location..location))
                        .with_message(format!("Expected {}", expected.join(", ")))
                        .with_color(Color::Red),
                )
                .finish()
                .write((srcname, Source::from(src)), &mut buf)
                .unwrap();
        }

        ParseError::UnrecognizedToken {
            token: (start, tok, end),
            expected,
        } => {
            Report::build(ReportKind::Error, (srcname, start..end))
                .with_config(Config::default().with_color(io::stdout().is_terminal()))
                .with_message("Unexpected token")
                .with_label(
                    Label::new((srcname, start..end))
                        .with_message(format!("Found {:?}, expected {}", tok, expected.join(", ")))
                        .with_color(Color::Red),
                )
                .finish()
                .write((srcname, Source::from(src)), &mut buf)
                .unwrap();
        }

        ParseError::ExtraToken {
            token: (start, tok, end),
        } => {
            Report::build(ReportKind::Error, (srcname, start..end))
                .with_config(Config::default().with_color(io::stdout().is_terminal()))
                .with_message("Extra token")
                .with_label(
                    Label::new((srcname, start..end))
                        .with_message(format!("Unexpected {:?}", tok))
                        .with_color(Color::Red),
                )
                .finish()
                .write((srcname, Source::from(src)), &mut buf)
                .unwrap();
        }

        ParseError::User { error } => {
            Report::build(ReportKind::Error, (srcname, 0..0))
                .with_config(Config::default().with_color(io::stdout().is_terminal()))
                .with_message(format!("Parser error: {}", error))
                .finish()
                .write((srcname, Source::from(src)), &mut buf)
                .unwrap();
        }
    }

    output.push_str(&String::from_utf8(buf).unwrap());
}

pub fn parse_err_add_offset<Tok, E>(
    err: &mut ParseError<usize, Tok, E>,
    offset: usize,
) {
    match err {
        ParseError::InvalidToken { location } => {
            *location += offset;
        }
        ParseError::UnrecognizedEof { location, .. } => {
            *location += offset;
        }
        ParseError::UnrecognizedToken { token: (start, _, end), .. } => {
            *start += offset;
            *end += offset;
        }
        ParseError::ExtraToken { token: (start, _, end) } => {
            *start += offset;
            *end += offset;
        }
        _ => (),
    }
}
