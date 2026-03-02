//! instruction mnemonics

use crate::types::Tok;

macro_rules! sw {
    ($($x:expr),*) => {
        &[$($x),*]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Register {
    R0 = 0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    RONES,
    RONE,
    RZERO,
    SP,
    BP,
    PC,
    CF,
    RF0,
    RF1,
    RF2,
    RF3,
    RF4,
    RF5,
}
impl Register {
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "r0" => Self::R0,
            "r1" => Self::R1,
            "r2" => Self::R2,
            "r3" => Self::R3,
            "r4" => Self::R4,
            "r5" => Self::R5,
            "r6" => Self::R6,
            "r7" => Self::R7,
            "r8" => Self::R8,
            "r9" => Self::R9,
            "r10" => Self::R10,
            "r11" => Self::R11,
            "r12" => Self::R12,
            "rones"|"r13" => Self::RONES,
            "rone"|"r14" => Self::RONE,
            "rzero"|"r15" => Self::RZERO,
            "sp" => Self::SP,
            "bp" => Self::BP,
            "pc" => Self::PC,
            "cf" => Self::CF,
            "rf0" => Self::RF0,
            "rf1" => Self::RF1,
            "rf2" => Self::RF2,
            "rf3" => Self::RF3,
            "rf4" => Self::RF4,
            "rf5" => Self::RF5,
            _ => {return None;}
        })
    }
    pub fn rtype(&self) -> O {
        if (*self as u8) < 16 {
            return O::Reg;
        }
        return O::XReg;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum M {
    ADD = 0,
    SUB,
    MUL,
    IMUL,
    DIV,
    IDIV,
    SHL,
    SHR,
    SAR,
    XOR,
    NOT,
    OR,
    AND,
    PUSH,
    POP,
    CMP,
    TEST,
    XCHG,
    CMPXCHG,
    JMP,
    JZ,JE,
    JNZ,JNE,
    JA,JAE,
    JG,JGE,
    JB,JBE,
    JL,JLE,
    RET,
    SYSCALL,
    CALL,
    MOV,
    HLT
}
impl M {
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "add" => Self::ADD,
            "sub" => Self::SUB,
            "mul" => Self::MUL,
            "imul" => Self::IMUL,
            "div" => Self::DIV,
            "idiv" => Self::IDIV,
            "shl" => Self::SHL,
            "shr" => Self::SHR,
            "sar" => Self::SAR,
            "xor" => Self::XOR,
            "not" => Self::NOT,
            "or" => Self::OR,
            "and" => Self::AND,
            "push" => Self::PUSH,
            "pop" => Self::POP,
            "cmp" => Self::CMP,
            "test" => Self::TEST,
            "xchg" => Self::XCHG,
            "cmpxchg" => Self::CMPXCHG,
            "jmp" => Self::JMP,
            "jz" => Self::JZ,
            "je" => Self::JE,
            "jnz" => Self::JNZ,
            "jne" => Self::JNE,
            "ja" => Self::JA,
            "jae" => Self::JAE,
            "jg" => Self::JG,
            "jge" => Self::JGE,
            "jb" => Self::JB,
            "jbe" => Self::JBE,
            "jl" => Self::JL,
            "jle" => Self::JLE,
            "ret" => Self::RET,
            "syscall" => Self::SYSCALL,
            "call" => Self::CALL,
            "mov" => Self::MOV,
            "hlt" => Self::HLT,
            _ => {return None;}
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum P {
    Byte = 0,
    Word,
    DWord,
    QWord,
    Fp,
    Call
}
impl P {
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word {
            "byte" => Self::Byte,
            "word" => Self::Word,
            "dword" => Self::DWord,
            "qword" => Self::QWord,
            "fp" => Self::Fp,
            "call" => Self::Call,
            _ => {return None;}
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum O {
    Reg = 0,
    XReg,
    AMem,
    RMem,
    Imm
}
impl O {
    pub fn qualified_eq(&self, other: &Self) -> bool {
        match self {
            Self::Reg => *self == *other || *other == Self::XReg,
            Self::AMem => *self == *other || *other == Self::RMem,
            _ => *self == *other
        }
    }
}

pub type StatSlice<T> = &'static [T];
pub type OpList = StatSlice<StatSlice<O>>;
pub type PreList = StatSlice<P>;

#[derive(Debug, Clone, Copy)]
pub struct AllowedPattern(pub M,pub PreList,pub OpList);
#[derive(Debug, Clone)]
pub struct OpPattern(pub M,pub Vec<P>,pub Vec<O>);

const NO_OPS: OpList = &[];
const NO_PRE: PreList = &[];

const STD_PREFIXES: PreList = &[P::Byte,P::Word,P::DWord,P::QWord];
const FP_PREFIXES: PreList = &[P::Byte,P::Word,P::DWord,P::QWord,P::Fp];

const ARITH_OPS: OpList = &[&[O::XReg],&[O::XReg,O::RMem,O::Imm]];
const RR_OPS: OpList = &[&[O::Reg],&[O::Reg]];
const RMI: StatSlice<O> = &[O::Reg,O::RMem,O::Imm];
// const RAI: StatSlice<O> = &[O::Reg,O::AMem,O::Imm];
const XMI: StatSlice<O> = &[O::XReg,O::RMem,O::Imm];
const JMP_OPS: OpList = sw!(&[O::Reg,O::RMem]);

const JMP_PRE: PreList = sw!(P::Call);

const RRMI: OpList = &[&[O::Reg],RMI];
const RXMI: OpList = &[&[O::Reg],XMI];

pub const ALLOWED_PATTERNS: &'static [AllowedPattern] = &[
    AllowedPattern(M::ADD,STD_PREFIXES,ARITH_OPS),
    AllowedPattern(M::SUB,STD_PREFIXES,ARITH_OPS),
    AllowedPattern(M::MUL,FP_PREFIXES,ARITH_OPS),
    AllowedPattern(M::IMUL,FP_PREFIXES,ARITH_OPS),
    AllowedPattern(M::DIV,FP_PREFIXES,ARITH_OPS),
    AllowedPattern(M::IDIV,FP_PREFIXES,ARITH_OPS),
    AllowedPattern(M::SHL,STD_PREFIXES,RR_OPS),
    AllowedPattern(M::SHR,STD_PREFIXES,RR_OPS),
    AllowedPattern(M::SAR,STD_PREFIXES,RR_OPS),
    AllowedPattern(M::XOR,STD_PREFIXES,RRMI),
    AllowedPattern(M::NOT,STD_PREFIXES,&[&[O::Reg]]),
    AllowedPattern(M::OR,STD_PREFIXES,RRMI),
    AllowedPattern(M::AND,STD_PREFIXES,RRMI),
    AllowedPattern(M::PUSH,STD_PREFIXES,sw!(sw!(O::XReg))),
    AllowedPattern(M::POP,STD_PREFIXES,sw!(sw!(O::XReg))),
    AllowedPattern(M::CMP,FP_PREFIXES,RXMI),
    AllowedPattern(M::TEST,FP_PREFIXES,sw!(sw!(O::XReg))),
    AllowedPattern(M::XCHG,STD_PREFIXES,RR_OPS),
    AllowedPattern(M::CMPXCHG,STD_PREFIXES,sw!(sw!(O::XReg),sw!(O::XReg),sw!(O::RMem),sw!(O::RMem))),
    AllowedPattern(M::JMP,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JZ,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JNZ,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JNE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JA,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JG,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JB,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JL,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JAE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JGE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JBE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::JLE,JMP_PRE,JMP_OPS),
    AllowedPattern(M::RET,NO_PRE,NO_OPS),
    AllowedPattern(M::SYSCALL,NO_PRE,NO_OPS),
    AllowedPattern(M::CALL,NO_PRE,JMP_OPS),
    AllowedPattern(M::MOV,FP_PREFIXES,sw!(sw!(O::XReg),XMI)),
    AllowedPattern(M::HLT,NO_PRE,NO_OPS),
];

pub struct Instruction<'a> {
    pub pat: &'static AllowedPattern,
    pub pres: Vec<Tok<'a>>,
    pub ops: Vec<Tok<'a>>,
}

impl OpPattern {
    pub fn try_find(mnemonic: M, prefixes: &Vec<P>, operands: &Vec<(O, crate::types::Token)>) -> Option<Self> {
        let mfilt = ALLOWED_PATTERNS.iter().filter(|x|x.0==mnemonic).collect::<Vec<_>>();
        // println!("{mfilt:?}");
        for pat in mfilt {
            if prefixes.iter().all(|x|pat.1.contains(x)) {
                if pat.2.len() != operands.len() {
                    continue;
                }
                if operands.iter().enumerate().all(|x|pat.2[x.0].iter().any(|y|x.1.0.qualified_eq(y))) {
                    return Some(Self(mnemonic,prefixes.to_owned(),operands.iter().map(|x|x.0).collect::<Vec<_>>()));
                }
            }
        }
        None
    }
}
