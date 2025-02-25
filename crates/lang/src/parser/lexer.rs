use chumsky::input::MapExtra;
use chumsky::prelude::*;
use rug::Float;
use rug::ops::CompleteRound;
use std::ops::Mul;

type LexerExtra<'src> = extra::Full<Rich<'src, char>, (), ()>;

#[derive(Debug, Clone, PartialEq)]
pub enum LangToken<'src> {
    Boolean(bool),
    Integer(i32),
    Number(Float),
    Op(&'src str),
    Delim(char),
    Ident(&'src str),
    Terminator,
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    For,
    Return,
    Break,
    Continue,
}

impl LangToken<'_> {
    fn spanned<'a, 'b, 'src>(
        self,
        map_extra: &'a mut MapExtra<'src, 'b, &'src str, LexerExtra<'src>>,
    ) -> (Self, SimpleSpan) {
        (self, map_extra.span())
    }
}

pub fn lexer<'src>(
    prec: u32,
) -> impl Parser<'src, &'src str, Vec<(LangToken<'src>, SimpleSpan)>, LexerExtra<'src>> {
    let dec_int = text::digits(10)
        .to_slice()
        .from_str()
        .unwrapped()
        .map(LangToken::Integer);

    let hex_int = just("0x")
        .ignore_then(
            text::digits(16)
                .to_slice()
                .map(|s| i32::from_str_radix(s, 16))
                .unwrapped(),
        )
        .map(LangToken::Integer);

    let oct_int = just("0o")
        .ignore_then(
            text::digits(8)
                .to_slice()
                .map(|s| i32::from_str_radix(s, 8))
                .unwrapped(),
        )
        .map(LangToken::Integer);

    let bin_int = just("0b")
        .ignore_then(
            text::digits(2)
                .to_slice()
                .map(|s| i32::from_str_radix(s, 2))
                .unwrapped(),
        )
        .map(LangToken::Integer);

    let dec_num_exp = just('e').then(one_of("+-").or_not()).then(text::digits(10));
    let dec_num = text::digits(10)
        .then(just('.').then(text::digits(10).or_not()))
        .to_slice()
        .or(just('.').then(text::digits(10)).to_slice())
        .then(dec_num_exp.or_not())
        .to_slice()
        .or(text::digits(10).then(dec_num_exp).to_slice())
        .map(Float::parse)
        .unwrapped()
        .map(move |i| i.complete(prec))
        .map(LangToken::Number);

    let hex_num_exp =
        just('p').ignore_then(one_of("+-").or_not().then(text::digits(10)).to_slice());
    let hex_parse_float = move |s: &str| Float::parse_radix(s, 16).unwrap().complete(prec);
    let hex_num = just("0x")
        .ignore_then(
            text::digits(16)
                .then(just('.').then(text::digits(16).or_not()))
                .to_slice()
                .or(just('.').then(text::digits(16)).to_slice())
                .map(hex_parse_float)
                .then(hex_num_exp.or_not())
                .map(move |(f, e)| {
                    if let Some(e) = e {
                        f.mul(Float::parse(e).unwrap().complete(prec).exp2())
                    } else {
                        f
                    }
                })
                .or(text::digits(16)
                    .to_slice()
                    .map(hex_parse_float)
                    .then(hex_num_exp)
                    .map(move |(f, e)| f.mul(Float::parse(e).unwrap().complete(prec).exp2()))),
        )
        .map(LangToken::Number);

    let op = one_of("+-*/^!=|&")
        .repeated()
        .at_least(1)
        .to_slice()
        .map(LangToken::Op);

    let delim = one_of("()[]{}:,").map(LangToken::Delim);

    let term = just(';').to(LangToken::Terminator);

    let ident = text::ascii::ident().map(|ident: &str| match ident {
        "fn" => LangToken::Fn,
        "let" => LangToken::Let,
        "mut" => LangToken::Mut,
        "if" => LangToken::If,
        "else" => LangToken::Else,
        "while" => LangToken::While,
        "for" => LangToken::For,
        "return" => LangToken::Return,
        "break" => LangToken::Break,
        "continue" => LangToken::Continue,
        _ => LangToken::Ident(ident),
    });

    let token = choice((
        hex_num, hex_int, oct_int, bin_int, dec_num, dec_int, op, delim, term, ident,
    ));

    // let token = hex_num;

    let line_comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded()
        .ignored();
    let multi_comment = just("/*")
        .then(any().and_is(just("*/").not()).repeated())
        .then(just("*/"))
        .padded()
        .ignored();

    token
        .map_with(LangToken::spanned)
        .padded_by(multi_comment.or(line_comment).repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::parser::lexer::{LangToken, lexer};
    use chumsky::Parser;
    use chumsky::span::SimpleSpan;
    use fhex::ToHex;
    use rug::Float;

    #[test]
    fn test_dec_int() {
        for i in 0..1000 {
            let s = format!("{i}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(LangToken::Integer(i), SimpleSpan::new(0, s.len()))],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_hex_int() {
        for i in 0..1000 {
            let s = format!("0x{i:x}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(LangToken::Integer(i), SimpleSpan::new(0, s.len()))],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_oct_int() {
        for i in 0..1000 {
            let s = format!("0o{i:o}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(LangToken::Integer(i), SimpleSpan::new(0, s.len()))],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_bin_int() {
        for i in 0..1000 {
            let s = format!("0b{i:b}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(LangToken::Integer(i), SimpleSpan::new(0, s.len()))],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_dec_num() {
        for i in 0..1000 {
            let f = i as f32 / 100f32;
            let mut s = format!("{f}");
            if !s.contains('.') {
                s.push('.');
            }
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(
                    LangToken::Number(Float::with_val(24, f)),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            )
        }
    }

    #[test]
    fn test_dec_num_exp() {
        let f = 2.3e-6f32;
        let s = f.to_hex();
        let vec = lexer(24).parse(&s).unwrap();
        assert_eq!(
            vec![(
                LangToken::Number(Float::with_val(24, f)),
                SimpleSpan::new(0, s.len())
            )],
            vec,
            "Attempted to parse {}",
            &s
        )
    }

    #[test]
    fn test_hex_num() {
        for i in 0..1000 {
            let f = i as f32 / 100f32;
            let s = f.to_hex();
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![(
                    LangToken::Number(Float::with_val(24, f)),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            )
        }
    }

    #[parameterized::parameterized(input = {
        "hello",
        "1 + 2",
        r#"// a comment
        let hello = c + 2;
        hello /*+ 3*/ * 2.0
        "#
    }, expected = {
        vec![(LangToken::Ident("hello"), SimpleSpan::new(0, 5))],
        vec![(LangToken::Integer(1), SimpleSpan::new(0, 1)), (LangToken::Op("+"), SimpleSpan::new(2, 3)), (LangToken::Integer(2), SimpleSpan::new(4, 5))],
        vec![
            (LangToken::Let, SimpleSpan::new(21, 24)),
            (LangToken::Ident("hello"), SimpleSpan::new(25, 30)),
            (LangToken::Op("="), SimpleSpan::new(31, 32)),
            (LangToken::Ident("c"), SimpleSpan::new(33, 34)),
            (LangToken::Op("+"), SimpleSpan::new(35, 36)),
            (LangToken::Integer(2), SimpleSpan::new(37, 38)),
            (LangToken::Terminator, SimpleSpan::new(38, 39)),
            (LangToken::Ident("hello"), SimpleSpan::new(48, 53)),
            (LangToken::Op("*"), SimpleSpan::new(62, 63)),
            (LangToken::Number(Float::with_val(24, 2.0)), SimpleSpan::new(64, 67))
        ]
    })]
    fn test_tokens(input: &str, expected: Vec<(LangToken, SimpleSpan)>) {
        let vec = lexer(24).parse(input).unwrap();
        assert_eq!(expected, vec, "Attempted to parse '{input}'");
    }
}
