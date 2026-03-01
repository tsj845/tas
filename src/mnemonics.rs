//! instruction mnemonics

use crate::types::Tok;

macro_rules! sw {
    ($($x:expr),*) => {
        &[$($x),*]
    }
}

#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
pub enum P {
    Byte = 0,
    Word,
    DWord,
    QWord,
    Fp,
    Call
}
#[derive(Debug, Clone, Copy)]
pub enum O {
    Reg = 0,
    XReg,
    AMem,
    RMem,
    Imm
}

pub type StatSlice<T> = &'static [T];
pub type OpList = StatSlice<StatSlice<O>>;
pub type PreList = StatSlice<P>;

#[derive(Debug, Clone, Copy)]
pub struct AllowedPattern(pub M,pub PreList,pub OpList);

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
