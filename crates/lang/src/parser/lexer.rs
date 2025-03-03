use crate::parser::Spanned;
use chumsky::input::MapExtra;
use chumsky::prelude::*;
use rug::Float;
use rug::ops::CompleteRound;
use std::ops::Mul;

type LexerExtra<'src> = extra::Full<Rich<'src, char>, (), ()>;

#[derive(Debug, Clone, PartialEq)]
pub enum LexerToken<'src> {
    Boolean(bool),
    RealInteger(i32),
    RealNumber(Float),
    ImaginaryInteger(i32),
    ImaginaryNumber(Float),
    I,
    Op(&'src str),
    Delim(char),
    Lifetime(&'src str),
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

impl LexerToken<'_> {
    fn spanned<'a, 'b, 'src>(
        self,
        map_extra: &'a mut MapExtra<'src, 'b, &'src str, LexerExtra<'src>>,
    ) -> Spanned<Self> {
        Spanned(self, map_extra.span())
    }
}

pub fn lexer<'src>(
    prec: u32,
) -> impl Parser<'src, &'src str, Vec<Spanned<LexerToken<'src>>>, LexerExtra<'src>> {
    let dec_int = text::digits(10).to_slice().from_str().unwrapped();

    let hex_int = just("0x").ignore_then(
        text::digits(16)
            .to_slice()
            .map(|s| i32::from_str_radix(s, 16))
            .unwrapped(),
    );

    let oct_int = just("0o").ignore_then(
        text::digits(8)
            .to_slice()
            .map(|s| i32::from_str_radix(s, 8))
            .unwrapped(),
    );

    let bin_int = just("0b").ignore_then(
        text::digits(2)
            .to_slice()
            .map(|s| i32::from_str_radix(s, 2))
            .unwrapped(),
    );

    let int = choice((hex_int, oct_int, bin_int, dec_int));
    let real_int = int.map(LexerToken::RealInteger);
    let imag_int = int.then_ignore(just('i')).map(LexerToken::ImaginaryInteger);

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
        .map(move |i| i.complete(prec));

    let hex_num_exp =
        just('p').ignore_then(one_of("+-").or_not().then(text::digits(10)).to_slice());
    let hex_parse_float = move |s: &str| Float::parse_radix(s, 16).unwrap().complete(prec);
    let hex_num = just("0x").ignore_then(
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
    );

    let num = hex_num.or(dec_num);
    let real_num = num.map(LexerToken::RealNumber);
    let imag_num = num.then_ignore(just('i')).map(LexerToken::ImaginaryNumber);

    let op = one_of("+-*/^!=|&")
        .repeated()
        .at_least(1)
        .to_slice()
        .map(LexerToken::Op);

    let delim = one_of("()[]{}:,").map(LexerToken::Delim);

    let term = just(';').to(LexerToken::Terminator);

    let lifetime = just('\'')
        .ignore_then(text::ascii::ident())
        .map(LexerToken::Lifetime);

    let ident = text::ascii::ident().map(|ident: &str| match ident {
        "i" => LexerToken::I,
        "fn" => LexerToken::Fn,
        "let" => LexerToken::Let,
        "mut" => LexerToken::Mut,
        "if" => LexerToken::If,
        "else" => LexerToken::Else,
        "while" => LexerToken::While,
        "for" => LexerToken::For,
        "return" => LexerToken::Return,
        "break" => LexerToken::Break,
        "continue" => LexerToken::Continue,
        _ => LexerToken::Ident(ident),
    });

    let token = choice((
        imag_num, real_num, imag_int, real_int, op, delim, term, lifetime, ident,
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
        .map_with(LexerToken::spanned)
        .padded_by(multi_comment.or(line_comment).repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::parser::Spanned;
    use crate::parser::lexer::{LexerToken, lexer};
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
                vec![Spanned(
                    LexerToken::RealInteger(i),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_dec_imag_int() {
        let vec = lexer(24).parse("32i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryInteger(32),
                SimpleSpan::new(0, 3)
            )],
            vec,
            "Attempted to parse 32i",
        );
    }

    #[test]
    fn test_hex_int() {
        for i in 0..1000 {
            let s = format!("0x{i:x}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![Spanned(
                    LexerToken::RealInteger(i),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_hex_imag_int() {
        let vec = lexer(24).parse("0x20i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryInteger(0x20),
                SimpleSpan::new(0, 5)
            )],
            vec,
            "Attempted to parse 0x20i",
        );
    }

    #[test]
    fn test_oct_int() {
        for i in 0..1000 {
            let s = format!("0o{i:o}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![Spanned(
                    LexerToken::RealInteger(i),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_oct_imag_int() {
        let vec = lexer(24).parse("0o40i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryInteger(0o40),
                SimpleSpan::new(0, 5)
            )],
            vec,
            "Attempted to parse 0o40i",
        );
    }

    #[test]
    fn test_bin_int() {
        for i in 0..1000 {
            let s = format!("0b{i:b}");
            let vec = lexer(24).parse(&s).unwrap();
            assert_eq!(
                vec![Spanned(
                    LexerToken::RealInteger(i),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            );
        }
    }

    #[test]
    fn test_bin_imag_int() {
        let vec = lexer(24).parse("0b100i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryInteger(0b100),
                SimpleSpan::new(0, 6)
            )],
            vec,
            "Attempted to parse 0b100i",
        );
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
                vec![Spanned(
                    LexerToken::RealNumber(Float::with_val(24, f)),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            )
        }
    }

    #[test]
    fn test_dec_imag_num() {
        let vec = lexer(24).parse("3.2i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryNumber(Float::with_val(24, 3.2f32)),
                SimpleSpan::new(0, 4)
            )],
            vec,
            "Attempted to parse 3.2i",
        );
    }

    #[test]
    fn test_dec_num_exp() {
        let f = 2.3e-6f32;
        let s = f.to_hex();
        let vec = lexer(24).parse(&s).unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::RealNumber(Float::with_val(24, f)),
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
                vec![Spanned(
                    LexerToken::RealNumber(Float::with_val(24, f)),
                    SimpleSpan::new(0, s.len())
                )],
                vec,
                "Attempted to parse {}",
                &s
            )
        }
    }

    #[test]
    fn test_hex_imag_num() {
        let vec = lexer(24).parse("0x3.2i").unwrap();
        assert_eq!(
            vec![Spanned(
                LexerToken::ImaginaryNumber(Float::with_val(24, 3.125f32)),
                SimpleSpan::new(0, 6)
            )],
            vec,
            "Attempted to parse 0x3.2i",
        );
    }

    #[parameterized::parameterized(input = {
        "hello",
        "1 + 2",
        r#"// a comment
        let hello = c + 2;
        hello /*+ 3*/ * 2.0
        "#
    }, expected = {
        vec![Spanned(LexerToken::Ident("hello"), SimpleSpan::new(0, 5))],
        vec![Spanned(LexerToken::RealInteger(1), SimpleSpan::new(0, 1)), Spanned(LexerToken::Op("+"), SimpleSpan::new(2, 3)), Spanned(LexerToken::RealInteger(2), SimpleSpan::new(4, 5))],
        vec![
            Spanned(LexerToken::Let, SimpleSpan::new(21, 24)),
            Spanned(LexerToken::Ident("hello"), SimpleSpan::new(25, 30)),
            Spanned(LexerToken::Op("="), SimpleSpan::new(31, 32)),
            Spanned(LexerToken::Ident("c"), SimpleSpan::new(33, 34)),
            Spanned(LexerToken::Op("+"), SimpleSpan::new(35, 36)),
            Spanned(LexerToken::RealInteger(2), SimpleSpan::new(37, 38)),
            Spanned(LexerToken::Terminator, SimpleSpan::new(38, 39)),
            Spanned(LexerToken::Ident("hello"), SimpleSpan::new(48, 53)),
            Spanned(LexerToken::Op("*"), SimpleSpan::new(62, 63)),
            Spanned(LexerToken::RealNumber(Float::with_val(24, 2.0)), SimpleSpan::new(64, 67))
        ]
    })]
    fn test_tokens(input: &str, expected: Vec<Spanned<LexerToken>>) {
        let vec = lexer(24).parse(input).unwrap();
        assert_eq!(expected, vec, "Attempted to parse '{input}'");
    }
}
