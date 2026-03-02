//! handles parsing tasm source files into tokens

use std::mem;

use crate::types::*;
use crate::errors::*;

pub type TokenVec<'a> = Vec<Token<'a>>;

static SYMCHARS: &'static [u8] = b"![](),:*$&";
static N_ALNUM_WCHARS: &'static [u8] = b"@_.";

fn is_wordchar(c: u8) -> bool {
    c.is_ascii_alphanumeric() || N_ALNUM_WCHARS.contains(&c)
}

enum ParseState {
    Scan = 0,
    Comment,
    String,
    Other,
}
enum CanBe {
    UInt = 1,
    SInt = 2,
    Float = 4,
    Word = 8,
}

fn lex_other<'a>(source: &'a [u8], sourcestr: &'a str, tokstart: usize, tokend: usize) -> Tok<'a> {
    let mut canbe = CanBe::UInt as u8 | CanBe::SInt as u8 | CanBe::Float as u8 | CanBe::Word as u8;
    let mut tokstartoffset = 0;
    if source[tokstart] == b'#' {
        canbe = CanBe::Word as u8;
        tokstartoffset += 1;
    }
    let mut dotfound = false;
    match source[tokstart] {
        b'-' => {canbe &= !(CanBe::UInt as u8);tokstartoffset += 1;}
        b'.' => {canbe &= !(CanBe::UInt as u8 | CanBe::SInt as u8);dotfound = true;tokstartoffset += 1;},
        _ => {}
    }
    if tokstart >= tokend {
        return Tok::Invalid;
    }
    for c in &source[tokstart+tokstartoffset..tokend] {
        // if canbe.count_ones() < 2 {
        //     break;
        // }
        // println!("{}, {:b}", *c as char, canbe);
        if !is_wordchar(*c) {
            canbe &= !(CanBe::Word as u8);
        }
        if *c == b'.' {
            if dotfound {
                canbe = 0;
            } else {
                canbe = CanBe::Float as u8;
                dotfound = true;
                // tokstart += 1;
                continue;
            }
        }
        if !c.is_ascii_digit() {
            canbe &= CanBe::Word as u8;
        }
    }
    if canbe & CanBe::UInt as u8 != 0 {
        return Tok::UInt((&sourcestr[tokstart..tokend]).parse::<u64>().unwrap());
    }
    if canbe & CanBe::SInt as u8 != 0 {
        return Tok::SInt((&sourcestr[tokstart..tokend]).parse::<i64>().unwrap());
    }
    // if dotfound {
    //     tokstart -= 1;
    // }
    if canbe & CanBe::Float as u8 != 0 {
        return Tok::Float((&sourcestr[tokstart..tokend]).parse::<f64>().unwrap());
    }
    if canbe & CanBe::Word as u8 != 0 {
        return Tok::Word(&sourcestr[tokstart..tokend]);
    }
    return Tok::Invalid;
}

/// convert a string into a stream of lexographic tokens
pub fn lex<'a>(sourcestr: &'a str) -> Result<TokenVec<'a>, TokenVec<'a>> {
    let mut haderr = false;
    let mut toks = Vec::new();
    let mut state = ParseState::Scan;
    let mut tokstart = 0usize;let mut startcol = 1u32;
    let mut i = 0usize;
    let sl = sourcestr.len();
    let mut line = 1u32;let mut column = 1u32;
    let source = sourcestr.as_bytes();
    while i < sl {
        loop {
            if source[i] == b'\n' {
                match state {
                    ParseState::String => {
                        haderr = true;
                        error(AsmErr { message: "unterminated string literal", line, column:startcol, context: None });
                    },
                    ParseState::Other => {
                        let tok = lex_other(source, sourcestr, tokstart, i);
                        match tok {
                            Tok::Invalid => {
                                haderr = true;
                                error(AsmErr { message: "invalid token", line, column:startcol, context: None });
                            },
                            _ => {toks.push(Token { tok, line, column:startcol });}
                        }
                    },
                    _ => {}
                }
                state = ParseState::Scan;
                toks.push(Token{tok:Tok::Newline,line,column});
                column = 0;
                line += 1;
                break;
            }
            match state {
                ParseState::Scan => {
                    if source[i] == b';' {
                        state = ParseState::Comment;
                    } else if source[i] == b'"' {
                        state = ParseState::String;
                        tokstart = i+1;
                        startcol = column;
                    } else if SYMCHARS.contains(&source[i]) {
                        toks.push(Token { tok: Tok::Symbol(source[i]), line, column });
                    } else if !source[i].is_ascii_whitespace() {
                        state = ParseState::Other;
                        tokstart = i;
                        startcol = column;
                    }
                },
                ParseState::String => {
                    if source[i] == b'\\' {
                        match source[i+1] {
                            b'x' => {i += 3;},
                            _ => {i += 1;}
                        }
                    } else if source[i] == b'"' {
                        toks.push(Token { tok: Tok::String(&sourcestr[tokstart..i]), line, column:startcol });
                        state = ParseState::Scan;
                    }
                },
                ParseState::Other => {
                    if !is_wordchar(source[i]) {
                        let tok = lex_other(source, sourcestr, tokstart, i);
                        match tok {
                            Tok::Invalid => {
                                haderr = true;
                                error(AsmErr { message: "invalid token", line, column:startcol, context: None });
                            },
                            _ => {toks.push(Token { tok, line, column:startcol });}
                        }
                        state = ParseState::Scan;
                        continue;
                    }
                }
                _ => {}
            }
            break;
        }
        column += 1;
        i += 1;
    }
    toks.push(Token { tok: Tok::Newline, line, column });
    if haderr {
        return Err(toks);
    }
    return Ok(toks);
}

/// convert lexographic tokens into syntactic tokens
pub fn syntactic_parse<'a>(toks: Vec<Token<'a>>) -> Result<TokenVec<'a>, TokenVec<'a>> {
    let mut build: Vec<Token<'_>> = Vec::new();
    let l = toks.len();let mut i = 0usize;
    let mut haderr = false;
    while i < l {
        match toks[i].tok {
            Tok::Symbol(c) => {
                match c {
                    b':' => match build.last_mut() {
                        Some(t) => {
                            match t.tok {
                                Tok::Word(word) => {t.tok = Tok::Label(word);}
                                _ => {
                                    haderr = true;
                                    error(AsmErr { message: "invalid label", line: toks[i].line, column: toks[i].column, context: None });
                                }
                            }
                        },
                        None => {
                            error(AsmErr { message: "invalid label", line: toks[i].line, column: toks[i].column, context: None });
                            haderr = true;
                        }
                    }
                    b'!' => match toks.get(i+1) {
                        Some(token) => {
                            match token.tok {
                                Tok::Word(word) => {
                                    if let Some(kw) = match word {
                                        "section" => Some(Keyword::Section),
                                        "purpose" => Some(Keyword::Purpose),
                                        "name" => Some(Keyword::Name),
                                        "invar" => Some(Keyword::Invar),
                                        "some" => Some(Keyword::Some),
                                        "none" => Some(Keyword::None),
                                        "str" => Some(Keyword::Str),
                                        "fstr" => Some(Keyword::Fstr),
                                        "s32" => Some(Keyword::S32),
                                        "f64" => Some(Keyword::F64),
                                        "res" => Some(Keyword::Res),
                                        "cutoff" => Some(Keyword::Cutoff),
                                        _ => {
                                            error(AsmErr { message: &format!("invalid keyword: {:?}", token.tok), line: token.line, column: token.column, context: None });
                                            haderr = true;
                                            None
                                        }
                                    } {
                                        i += 1;
                                        build.push(Token { tok: Tok::Keyword(kw), line: toks[i].line, column: toks[i].column });
                                    }
                                }
                                Tok::Symbol(b'!') => {
                                    i += 1;
                                    build.push(Token { tok: Tok::Type(Type::Invalid), line: toks[i].line, column: toks[i].column });
                                }
                                _ => {
                                    error(AsmErr { message: &format!("invalid keyword: {:?}", token.tok), line: token.line, column: token.column, context: None });
                                    haderr = true;
                                }
                            }
                        }
                        None => {
                            error(AsmErr { message: "incomplete keyword declaration", line: toks[i].line, column: toks[i].column, context: None });
                            haderr = true;
                        }
                    }
                    b'$' => match toks.get(i+1) {
                        Some(token) => {
                            match token.tok {
                                Tok::UInt(val) => {
                                    build.push(Token { tok: Tok::Addr(val as uptr), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                _ => {
                                    error(AsmErr { message: &format!("invalid memory address: {:?}", token.tok), line: token.line, column: token.column, context: None });
                                    haderr = true;
                                }
                            }
                        }
                        None => {
                            error(AsmErr { message: "incomplete memory address declaration", line: toks[i].line, column: toks[i].column, context: None });
                            haderr = true;
                        }
                    }
                    _ => {build.push(toks[i].clone());}
                }
            }
            // Tok::Word(word) => {
            //     //
            // }
            _ => {build.push(toks[i].clone());}
        }
        i += 1;
    }
    if haderr {
        return Err(build);
    }
    return Ok(build);
}

enum IndexSeq {
    Scan = 0,
    LPar,
    Args,
    Rett,
}
enum CodeSeq {
    Scan = 0,
    PrefixLP,
    Prefix,
    PrefixRP,
    Mnemonic,
    Operand,
    OperandC,
}

struct CState<'a> {
    pub seq: CodeSeq,
    pub prefixes: Vec<Prefix>,
    pub mnemonic: Mnemonic,
    pub operands: Vec<(Operand, Token<'a>)>,
    /// parse failed, do not attempt to construct a token
    pub scrapped: bool,
    pub line: u32,
    pub column: u32,
}
impl<'a> Default for CState<'a> {
    fn default() -> Self {
        Self { seq: CodeSeq::Scan, prefixes: Vec::new(), mnemonic: Mnemonic::ADD, operands: Vec::new(), scrapped: false, line: 0, column: 0 }
    }
}
impl<'a> CState<'a> {
    pub fn reset(&mut self) -> () {
        self.seq = CodeSeq::Scan;
        self.prefixes.clear();
        self.operands.clear();
        self.scrapped = false;
    }
    pub fn into_token(&mut self) -> Token<'a> {
        return Token { tok: Tok::Instruction(self.mnemonic, mem::replace(&mut self.prefixes, Vec::new()), mem::replace(&mut self.operands, Vec::new())), line: self.line, column: self.column };
    }
}

/// convert syntactic tokens into semantic tokens, which take context into account
pub fn semantic_parse<'a>(toks: Vec<Token<'a>>) -> Result<TokenVec<'a>, TokenVec<'a>> {
    let mut build: Vec<Token<'_>> = Vec::new();
    let l = toks.len();let mut i = 0usize;
    let mut haderr = false;
    let mut csec = Section::None;
    let mut range_s = [0usize;3];let mut range_e = [0usize;2];
    let mut indx_seq = IndexSeq::Scan;
    let mut cstate = CState::default();
    while i < l {
        match toks[i].tok {
            Tok::Keyword(Keyword::Cutoff) => {
                return Ok(build);
            }
            Tok::Keyword(Keyword::Section) => {
                match toks.get(i+1) {
                    Some(token) => {
                        match token.tok {
                            Tok::Word(word) => {
                                if let Some(sec) = match word {
                                    ".conf" => Some(Section::Conf),
                                    ".data" => Some(Section::Data),
                                    ".code" => Some(Section::Code),
                                    ".indx" => Some(Section::Indx),
                                    _ => None
                                } {
                                    csec = sec;
                                    build.push(Token { tok: Tok::Section(csec), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                            }
                            _ => {
                                error(AsmErr { message: &format!("invalid section: {:?}", token.tok), line: token.line, column: token.column, context: None });
                            }
                        }
                    }
                    None => {
                        error(AsmErr { message: "no section given", line: toks[i].line, column: toks[i].column, context: None });
                        haderr = true;
                    }
                }
            }
            _ => match csec {
                Section::Conf => {
                    match toks[i].tok {
                        Tok::Keyword(kw) => match toks.get(i+1) {
                            Some(token) => match kw {
                                Keyword::Purpose => if let Some(p) = match token.tok {
                                    Tok::Word(word) | Tok::String(word) => {
                                        match word {
                                            "3tr" => Some(0),
                                            "gpc" => Some(1),
                                            "bot" => Some(2),
                                            "sbc" => Some(3),
                                            _ => None
                                        }
                                    }
                                    Tok::UInt(val) => Some(val),
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid purpose: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::UInt(p))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                Keyword::Name => if let Some(n) = match token.tok {
                                    Tok::String(name) => match name.len() < 256 {true=>Some(name),_=>{haderr=true;error(AsmErr { message: "string too long", line: token.line, column: token.column, context: None });None}},
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid name: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::String(n))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                Keyword::Fstr => if let Some(n) = match token.tok {
                                    Tok::String(fstr) => Some(fstr),
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid fstr: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::String(n))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                Keyword::Invar => if let Some(n) = match token.tok {
                                    Tok::UInt(count) => match count < 256 {true=>Some(count),_=>None},
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid invar count: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::UInt(n))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                Keyword::Some => if let Some(n) = match token.tok {
                                    Tok::UInt(val) => Some(val),
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid some flags: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::UInt(n))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                Keyword::None => if let Some(n) = match token.tok {
                                    Tok::UInt(val) => Some(val),
                                    _ => {haderr=true;error(AsmErr { message: &format!("invalid none flags: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                                } {
                                    build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::UInt(n))), line: toks[i].line, column: toks[i].column });
                                    i += 1;
                                }
                                _ => {
                                    haderr = true;
                                    error(AsmErr { message: "keyword declaration not valid for .conf section", line: toks[i].line, column: toks[i].column, context: None });
                                }
                            }
                            None => {
                                haderr = true;
                                error(AsmErr { message: "bad keword declaration", line: toks[i].line, column: toks[i].column, context: None });
                            }
                        }
                        Tok::Newline => {build.push(toks[i].clone());},
                        _ => {
                            warn(AsmErr { message: "unknown token", line: toks[i].line, column: toks[i].column, context: None });
                            build.push(toks[i].clone());
                        }
                    }
                }
                Section::Data => match toks[i].tok {
                    Tok::Keyword(kw) => match toks.get(i+1) {
                        Some(token) => match kw {
                            Keyword::Str => if let Some(n) = match token.tok {
                                Tok::String(s) => match s.len() < 256 {true=>Some(s),_=>{haderr=true;error(AsmErr { message: "string too long", line: token.line, column: token.column, context: None });None}},
                                _ => {haderr=true;error(AsmErr { message: &format!("invalid string: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                            } {
                                build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::String(n))), line: toks[i].line, column: toks[i].column });
                                i += 1;
                            }
                            Keyword::F64 => if let Some(n) = match token.tok {
                                Tok::Float(val) => Some(val),
                                _ => {haderr=true;error(AsmErr { message: &format!("invalid floating point literal: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                            } {
                                build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::Float(n))), line: toks[i].line, column: toks[i].column });
                                i += 1;
                            }
                            Keyword::S32 => if let Some(n) = match token.tok {
                                Tok::SInt(val) => Some(val),
                                Tok::UInt(val) => Some(val as i64),
                                _ => {haderr=true;error(AsmErr { message: &format!("invalid s32 literal: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                            } {
                                build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::SInt(n))), line: toks[i].line, column: toks[i].column });
                                i += 1;
                            }
                            Keyword::Res => if let Some(n) = match token.tok {
                                Tok::UInt(val) => Some(val),
                                _ => {haderr=true;error(AsmErr { message: &format!("invalid reserve literal: {:?}",token.tok), line: token.line, column: token.column, context: None });None}
                            } {
                                build.push(Token { tok: Tok::KwDecl(kw, Box::new(Tok::UInt(n))), line: toks[i].line, column: toks[i].column });
                                i += 1;
                            }
                            _ => {
                                haderr = true;
                                error(AsmErr { message: "keyword declaration not valid for .data section", line: toks[i].line, column: toks[i].column, context: None });
                            }
                        }
                        None => {
                            haderr = true;
                            error(AsmErr { message: "bad keword declaration", line: toks[i].line, column: toks[i].column, context: None });
                        }
                    }
                    Tok::Newline | Tok::Label(_) => {build.push(toks[i].clone());},
                    _ => {
                        warn(AsmErr { message: "unknown token", line: toks[i].line, column: toks[i].column, context: None });
                        build.push(toks[i].clone());
                    }
                }
                Section::Indx => match indx_seq {
                    IndexSeq::Scan => match toks[i].tok {
                        Tok::Word(_) => {range_s[0]=build.len();build.push(toks[i].clone());indx_seq = IndexSeq::LPar;}
                        Tok::Newline => {}
                        _ => {error(AsmErr { message: "expected word as start of index entry", line: toks[i].line, column: toks[i].column, context: None });haderr=true;}
                    }
                    IndexSeq::LPar => match toks[i].tok {
                        Tok::Symbol(b'(') => {indx_seq=IndexSeq::Args;range_s[1]=build.len();}
                        _ => {error(AsmErr { message: &format!("expected left parenthesis, got: {:?}", toks[i].tok), line: toks[i].line, column: toks[i].column, context: None });haderr=true;}
                    }
                    IndexSeq::Args => match toks[i].tok {
                        Tok::Symbol(b')') | Tok::Symbol(b',') => {
                            // println!("{} / {}", range_s[1], build.len());
                            if build.len() > range_s[1] {
                                let nt = build.pop().unwrap();
                                match nt.tok {
                                    Tok::Word(pname) => {
                                        let t = match parse_type(build.split_off(range_s[1])) {
                                            Ok(v) => v,
                                            Err(v) => {haderr=true;v}
                                        };
                                        build.push(Token { tok: Tok::Param(pname, t), line: nt.line, column: nt.column });
                                        range_s[1] += 1;
                                    }
                                    _ => {error(AsmErr { message: "invalid parameter name", line: nt.line, column: nt.column, context: None });haderr=true;}
                                }
                            }
                            match toks[i].tok {
                                Tok::Symbol(b')') => {
                                    indx_seq = IndexSeq::Rett;
                                }
                                _ => {}
                            }
                        }
                        _ => {build.push(toks[i].clone());}
                    }
                    IndexSeq::Rett => match toks[i].tok {
                        Tok::Newline => {
                            // println!("rett {} / {}", range_s[1], build.len());
                            let t = match parse_type(build.split_off(range_s[1])) {
                                Ok(v) => v,
                                Err(v) => {haderr=true;v}
                            };
                            let params = build.split_off(range_s[0]+1).into_iter().map(|x|x.tok).collect::<Vec<_>>();
                            let n = build.pop().unwrap();
                            match n.tok {
                                Tok::Word(nv) => {build.push(Token { tok: Tok::Signature(nv, params.into_boxed_slice(), t), line: n.line, column: n.column });}
                                _ => {unreachable!();}
                            }
                            indx_seq = IndexSeq::Scan;
                        }
                        _ => {build.push(toks[i].clone());}
                    }
                }
                Section::Code => match cstate.seq {
                    CodeSeq::Scan => match toks[i].tok {
                        Tok::Word(_) => {cstate.seq=CodeSeq::PrefixLP;continue;}
                        Tok::Newline => {}
                        Tok::Label(_) => {build.push(toks[i].clone());}
                        _ => {haderr=true;error(AsmErr { message: &format!("expected prefix or mnemonic, got: {:?}", toks[i].tok), line: toks[i].line, column: toks[i].column, context: None });}
                    }
                    CodeSeq::PrefixLP => match toks[i].tok {
                        Tok::Symbol(b'(') => {cstate.seq = CodeSeq::Prefix;}
                        Tok::Word(_) | Tok::Newline => {cstate.seq = CodeSeq::Mnemonic;continue;}
                        _ => {haderr=true;error(AsmErr { message: "invalid token", line: toks[i].line, column: toks[i].column, context: None });cstate.scrapped=true;}
                    }
                    CodeSeq::Prefix => {
                        match toks[i].tok {
                            Tok::Word(word) => match Prefix::from_word(word) {
                                Some(p) => {cstate.prefixes.push(p);}
                                _ => {haderr=true;error(AsmErr { message: "invalid prefix", line: toks[i].line, column: toks[i].column, context: None });cstate.scrapped=true;}
                            }
                            _ => {haderr=true;error(AsmErr { message: "invalid prefix", line: toks[i].line, column: toks[i].column, context: None });cstate.scrapped=true;}
                        }
                        cstate.seq = CodeSeq::PrefixRP;
                    }
                    CodeSeq::PrefixRP => {
                        match toks[i].tok {
                            Tok::Symbol(b')') => {}
                            _ => {haderr=true;error(AsmErr { message: "expected closing parenthesis", line: toks[i].line, column: toks[i].column, context: None });cstate.scrapped=true;}
                        }
                        cstate.seq = CodeSeq::PrefixLP;
                    }
                    CodeSeq::Mnemonic => match toks[i].tok {
                        Tok::Newline => {haderr=true;error(AsmErr { message: "expected instruction mnemonic", line: toks[i].line, column: toks[i].column, context: None });cstate.seq=CodeSeq::Operand;continue;}
                        Tok::Word(word) => {
                            match Mnemonic::from_word(word) {
                                Some(m) => {cstate.mnemonic=m;cstate.seq=CodeSeq::Operand;cstate.line=toks[i].line;cstate.column=toks[i].column;}
                                None => {haderr=true;cstate.scrapped=true;error(AsmErr { message: &format!("'{}' is not a valid instruction mnemonic", word), line: toks[i].line, column: toks[i].column, context: None });}
                            }
                            cstate.seq = CodeSeq::Operand;
                        }
                        _ => {haderr=true;error(AsmErr { message: &format!("invalid instruction mnemonic: {:?}", toks[i].tok), line: toks[i].line, column: toks[i].column, context: None });cstate.scrapped=true;}
                    }
                    CodeSeq::OperandC => match toks[i].tok {
                        Tok::Symbol(b',') => {cstate.seq = CodeSeq::Operand;}
                        Tok::Newline => {cstate.seq = CodeSeq::Operand;continue;}
                        _ => {haderr=true;cstate.scrapped=true;error(AsmErr { message: "expected comma or newline", line: toks[i].line, column: toks[i].column, context: None });}
                    }
                    CodeSeq::Operand => match toks[i].tok {
                        Tok::Symbol(b'[') => match toks[i+1].tok {
                            Tok::Word(word) => {
                                build.push(Token {tok:match Register::from_word(word) {Some(r)=>Tok::Reg(r),_=>Tok::Word(word)},line:toks[i+1].line,column:toks[i+1].column});
                                let cl = build.len();
                                match toks[i+2].tok {
                                    Tok::Symbol(b']') => {
                                        cstate.operands.push((Operand::RMem, Token {tok:Tok::Deref(build.split_off(cl).into_boxed_slice()),line:toks[i+1].line,column:toks[i+1].column}));
                                        i += 2;
                                        cstate.seq = CodeSeq::OperandC;
                                    }
                                    Tok::UInt(_) | Tok::SInt(_) => match toks[i+3].tok {
                                        Tok::Symbol(b']') => {
                                            build.push(toks[i+2].clone());
                                            cstate.operands.push((Operand::RMem, Token {tok:Tok::Deref(build.split_off(cl).into_boxed_slice()),line:toks[i+1].line,column:toks[i+1].column}));
                                            i += 3;
                                            cstate.seq = CodeSeq::OperandC;
                                        }
                                        _ => {let _=build.split_off(cl);haderr=true;cstate.scrapped=true;cstate.seq=CodeSeq::OperandC;error(AsmErr { message: "invalid deref", line: toks[i+3].line, column: toks[i+3].column, context: None });continue;}
                                    }
                                    _ => {let _=build.split_off(cl);haderr=true;cstate.scrapped=true;cstate.seq=CodeSeq::OperandC;error(AsmErr { message: "invalid deref", line: toks[i+2].line, column: toks[i+2].column, context: None });continue;}
                                }
                            }
                            _ => {haderr=true;cstate.scrapped=true;cstate.seq=CodeSeq::OperandC;error(AsmErr { message: "expected deref", line: toks[i+1].line, column: toks[i+1].column, context: None });continue;}
                        }
                        Tok::Addr(_) => {cstate.operands.push((Operand::AMem, toks[i].clone()));cstate.seq = CodeSeq::OperandC;}
                        Tok::SInt(_) | Tok::UInt(_) | Tok::Float(_) => {cstate.operands.push((Operand::Imm, toks[i].clone()));cstate.seq = CodeSeq::OperandC;}
                        Tok::Word(word) => match Register::from_word(word) {
                            Some(r) => {cstate.operands.push((r.rtype(), Token {tok:Tok::Reg(r),line:toks[i].line,column:toks[i].column}));cstate.seq = CodeSeq::OperandC;}
                            None => {cstate.operands.push((Operand::Imm, toks[i].clone()));cstate.seq = CodeSeq::OperandC;}
                        }
                        Tok::Newline => {
                            if cstate.scrapped {
                                cstate.reset();
                                continue;
                            }
                            if let Some(_) = OpPattern::try_find(cstate.mnemonic, &cstate.prefixes, &cstate.operands) {
                                build.push(cstate.into_token());
                            } else {
                                haderr = true;
                                error(AsmErr { message: "invalid operands", line: cstate.line, column: cstate.column, context: None });
                                // println!("{:?}\n{:?}\n{:?}", cstate.mnemonic, cstate.prefixes, cstate.operands);
                            }
                            cstate.reset();
                        }
                        _ => {}
                    }
                }
                Section::None => {
                    match toks[i].tok {
                        Tok::Newline => {}
                        _ => {warn(AsmErr { message: "token outside of section", line: toks[i].line, column: toks[i].column, context: None });}
                    }
                    build.push(toks[i].clone());
                }
            }
        }
        i += 1;
    }
    if haderr {
        return Err(build);
    }
    return Ok(build);
}

fn parse_type<'a>(toks: Vec<Token<'a>>) -> Result<Type, Type> {
    let mut t: Type = Type::Unset;
    let mut brackseq = 0u8;
    let mut haderr = false;
    for part in toks {
        match brackseq {
            2 => match part.tok {
                Tok::Symbol(b']') => {brackseq = 0;}
                _ => {error(AsmErr { message: "expected closing bracket", line: part.line, column: part.column, context: None });haderr=true;break;}
            }
            1 => match part.tok {
                Tok::UInt(n) => {
                    if n < 64 {
                        brackseq = 2;
                        t = Type::Sarr(n as u8, Box::new(t));
                    } else {
                        error(AsmErr { message: "size too large", line: part.line, column: part.column, context: None });
                        haderr = true;
                        break;
                    }
                }
                Tok::Symbol(b']') => {
                    t = Type::Uarr(Box::new(t));
                    brackseq = 0;
                }
                _ => {error(AsmErr { message: "expected closing bracket or size", line: part.line, column: part.column, context: None });haderr=true;break;}
            }
            0 => match part.tok {
                Tok::Symbol(b'*') => match t {
                    Type::Unset => {error(AsmErr { message: "invalid parameter type", line: part.line, column: part.column, context: None });haderr=true;break;}
                    _ => {t = Type::Ptr(Box::new(t));}
                }
                Tok::Symbol(b'[') => {
                    brackseq = 1;
                }
                Tok::Type(Type::Invalid) => match t {
                    Type::Unset => {t = Type::Invalid;}
                    _ => {error(AsmErr { message: "invalid parameter type", line: part.line, column: part.column, context: None });haderr=true;break;}
                }
                Tok::Word(tword) => match t {
                    Type::Unset => match tword {
                        "u8" => {t = Type::U8;}
                        "u16" => {t = Type::U16;}
                        "u32" => {t = Type::U32;}
                        "u64" => {t = Type::U64;}
                        "u128" => {t = Type::U128;}
                        "s8" => {t = Type::S8;}
                        "s16" => {t = Type::S16;}
                        "s32" => {t = Type::S32;}
                        "s64" => {t = Type::S64;}
                        "s128" => {t = Type::S128;}
                        "void" => {t = Type::Void;}
                        "sstr" => {t = Type::Sstr;}
                        "lstr" => {t = Type::Lstr;}
                        "struct" => {t = Type::Struct;}
                        "any" => {t = Type::Any;}
                        _ => {error(AsmErr { message: "invalid type", line: part.line, column: part.column, context: None });haderr=true;break;}
                    }
                    _ => {error(AsmErr { message: "invalid parameter type", line: part.line, column: part.column, context: None });haderr=true;break;}
                }
                _ => {error(AsmErr { message: "invalid parameter type", line: part.line, column: part.column, context: None });haderr=true;break;}
            }
            _ => {unreachable!();}
        }
    }
    if haderr {
        return Err(t);
    }
    Ok(t)
}
