#![deny(warnings)]

mod bpe;
mod normalizer;
mod special;
mod vocab_txt;

/// `utok` for token id.
#[allow(non_camel_case_types)]
pub type utok = u32;

pub trait Tokenize {
    fn vocab_size(&self) -> usize;
    fn encode(&self, text: &str) -> Vec<utok>;
    fn decode(&self, token: utok) -> &str;
}

pub trait Method {
    fn unk_token(&self) -> utok;
    fn vocab_size(&self) -> usize;
    fn internal_special(&self) -> impl IntoIterator<Item = (&str, utok)>;
    fn encode<'a>(&'a self, text: &'a str) -> impl IntoIterator<Item = utok> + 'a;
    fn decode(&self, token: utok) -> &[u8];
}

pub use bpe::BPE;
pub use normalizer::{BPECommonNormalizer, Normalizer};
pub use special::Tokenizer;
pub use vocab_txt::VocabTxt;

const fn as_byte_token(piece: &[u8]) -> Option<u8> {
    // 按结构分解并转换
    match piece {
        &[b'<', b'0', b'x', a, b, b'>'] if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() => {
            // ascii 转数字
            #[inline(always)]
            const fn to_num(c: u8) -> u8 {
                match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => unreachable!(),
                }
            }

            Some(to_num(a) * 16 + to_num(b))
        }
        _ => None,
    }
}

const fn decode_with_ascii(piece: &str) -> &str {
    match as_byte_token(piece.as_bytes()) {
        Some(b) => {
            // 预填充 ASCII 码表的所有字符
            const BYTES: [u8; 256] = {
                let mut ans = [0; 256];
                let mut i = 0;
                while i < 256 {
                    ans[i] = i as _;
                    i += 1;
                }
                ans
            };

            use std::{slice::from_ref, str::from_utf8_unchecked};
            let byte = from_ref(&BYTES[b as usize]);
            unsafe { from_utf8_unchecked(byte) }
        }
        None => piece,
    }
}

#[test]
fn test_decode_with_byte() {
    assert_eq!(decode_with_ascii("<0x0A>"), "\n");
    assert_eq!(decode_with_ascii("<0x20>"), " ");
    assert_eq!(decode_with_ascii("<0x2E>"), ".");
    assert_eq!(decode_with_ascii("<0x30>"), "0");
    assert_eq!(decode_with_ascii("<0x39>"), "9");
    assert_eq!(decode_with_ascii("<0x41>"), "A");
    assert_eq!(decode_with_ascii("<0x5A>"), "Z");
    assert_eq!(decode_with_ascii("<0x61>"), "a");
    assert_eq!(decode_with_ascii("<0x7A>"), "z");
}
