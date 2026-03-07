//! type declarations

#[allow(unused_imports)]
pub use crate::mnemonics::{M as Mnemonic,P as Prefix,O as Operand,Instruction,ALLOWED_PATTERNS,Register,OpPattern};

#[allow(non_camel_case_types)]
pub type uptr = u32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Section {
    None,
    Conf,
    Data,
    Code,
    Indx
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Keyword {
    Purpose = 0,
    Name,
    Invar,
    Fstr,
    Some,
    None,
    Str,
    S32,
    F64,
    Res,
    Section,
    Cutoff,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    U8,U16,U32,U64,U128,
    S8,S16,S32,S64,S128,
    Void,
    Sstr,
    Lstr,
    Sarr(u8,Box<Type>),
    Uarr(Box<Type>),
    Struct,
    Any,
    Invalid,
    Ptr(Box<Type>),
    Unset,
    Auto,
}
impl Type {
    fn add_bytes(&self, bytes: &mut Vec<u8>) -> () {
        bytes.push(match self {
            Self::U8 => 0x81,
            Type::U16 => 0x82,
            Type::U32 => 0x84,
            Type::U64 => 0x88,
            Type::U128 => 0x90,
            Type::S8 => 0xc1,
            Type::S16 => 0xc2,
            Type::S32 => 0xc4,
            Type::S64 => 0xc8,
            Type::S128 => 0xd0,
            Type::Void => 0x00,
            Type::Sstr => 0x02,
            Type::Lstr => 0x03,
            Type::Sarr(s, _) => 0x40 | *s,
            Type::Uarr(_) => 0x04,
            Type::Struct => 0x06,
            Type::Any => 0x05,
            Type::Invalid => 0xff,
            Type::Ptr(_) => 0x01,
            Type::Unset|Type::Auto => {panic!("Unset and Auto do not have bit representations");}
        });
        match self {
            Type::Sarr(_,t) | Type::Uarr(t) | Type::Ptr(t) => {t.as_ref().add_bytes(bytes);}
            _ => {}
        }
    }
    pub fn to_bytes(&self) -> Box<[u8]> {
        let mut b = Vec::new();
        self.add_bytes(&mut b);
        return b.into_boxed_slice();
    }
    pub fn auto_eq(&self, other: &Self) -> bool {
        return self == other || self == &Self::Auto || other == &Self::Auto;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelTo {
    None,
    Invar,
    Data,
    Reg(Register),
}
impl RelTo {
    pub fn qualified_eq(&self, other: &Self) -> bool {
        if *self == RelTo::None || *other == RelTo::None {
            return true;
        }
        return *self == *other;
    }
}

#[derive(Debug, Clone)]
pub enum Tok<'a> {
    String(&'a str),
    Word(&'a str),
    Label(&'a str),
    Keyword(Keyword),
    Section(Section),
    KwDecl(Keyword, Box<Tok<'a>>),
    Param(&'a str, Type),
    Signature(&'a str, Box<[Tok<'a>]>, Type),
    Type(Type),
    Symbol(u8),
    Newline,
    Addr(uptr),
    UInt(u64),
    SInt(i64),
    Float(f64),
    Prefix(Prefix),
    Instruction(Mnemonic,Vec<Prefix>,Vec<(Operand, Token<'a>)>,RelTo),
    Sized(u64, Box<Tok<'a>>),
    Deref(Box<[Token<'a>]>),
    Reg(Register),
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub tok: Tok<'a>,
    pub line: u32,
    pub column: u32,
}

pub fn min_size(value: &Tok) -> u64 {
    let val = match value {
        Tok::Addr(v) => 4-(v.leading_zeros() >> 3),
        Tok::Float(_) => 8,
        Tok::UInt(v) => 8-(v.leading_zeros() >> 3),
        Tok::SInt(v) => match *v < 0 {
            true => (v.trailing_zeros()+1) >> 3,
            _ => 8-(v.leading_zeros() >> 3)
        }
        _ => 0
    };
    if val == 0 {
        return 1;
    }
    return val as u64;
}
