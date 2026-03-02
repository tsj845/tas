//! type declarations

#[allow(unused_imports)]
pub use crate::mnemonics::{M as Mnemonic,P as Prefix,O as Operand,Instruction,ALLOWED_PATTERNS,Register,OpPattern};

#[allow(non_camel_case_types)]
pub type uptr = u32;

#[derive(Debug, Clone, Copy)]
pub enum Section {
    None,
    Conf,
    Data,
    Code,
    Indx
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone)]
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
    Instruction(Mnemonic,Vec<Prefix>,Vec<(Operand, Token<'a>)>),
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

#[derive(Debug)]
/// tokens directly writeable to output
pub struct WriteToken {
    //
}
