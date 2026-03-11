//! handles emission of ttvm object files

use std::{collections::HashMap, fs::File, io::{self, Write}};
use crate::{errors::{AsmErr, error, warn}, types::*};

#[derive(Debug, Clone, Copy)]
enum Refs {
    Code = 0,
    Data,
    #[allow(dead_code)]
    Invar
}


/// returns Some((label_map, index_map, toks, config_map, sec_toks, counts))
/// 
/// returns None on assembly errors (this function will [emit the errors][crate::errors] itself, do not emit errors when it returns None)
/// 
/// panics if the [parser][crate::parse::semantic_parse] failed to generate a well-formed token sequence
/// 
/// label_map is a hashmap from label names to (size, offset, reference)
/// - size is the minimum size needed to represent the label offset in bytes
/// - offset is the position of the label
/// - reference is the [base][self::Refs] that the label refers to
/// 
/// index_map is a hasmap from symbols to the index that the index entry will have in the emitted ttvm object file
/// 
/// toks is the modified token sequence, returned for ownership purposes
/// 
/// config_map is a six entry vector that is initialized to IV, each entry represents a possible KwDecl
/// - IV: \[None; 6]
/// - 0: purpose
/// - 1: name
/// - 2: invar
/// - 3: some
/// - 4: none
/// - 5: fstr
/// 
/// sec_toks is a vector of section tokens
/// 
/// counts is a two entry vector initialized to IV, each entry represents the final total of a count
/// - IV: \[0,0]
/// - 0: datavar count
/// - 1: index entry count
fn construct_maps<'a>(mut toks: Vec<Token<'a>>) -> Option<(HashMap<String, (u64, u64, Refs)>, HashMap<String, usize>, Vec<Token<'a>>, Vec<Option<Token<'a>>>, Vec<Token<'a>>, Vec<u32>)> {
    let mut label_map: HashMap<String, (u64, u64, Refs)> = HashMap::new();
    let mut index_map = HashMap::new();let mut index_counter = 0;
    let mut config_map: Vec<Option<Token>> = vec![None; 6];
    let mut cpos = 0u64;
    let mut cseq = Section::None;
    let mut parent_label: &str = "";
    let mut haderr = false;
    let mut sized_toks: Vec<(usize, usize, u64)> = Vec::new();
    let mut sec_toks: Vec<Token> = Vec::new();
    let mut counts: Vec<u32> = vec![0,0];
    // have to make sure the index map is fully constructed before attempting to construct the label map
    // this is due to the fact that x86 style calls with immediate values are of a different size than other calls
    // the style of the call is entirely dependent on whether the label has an index entry, so the index map must be
    // constructed first
    for token in toks.iter() {
        match token.tok {
            Tok::Section(s) => {cseq = s;continue;}
            Tok::Newline => {continue;}
            _ => {}
        }
        match cseq {
            Section::Indx => match &token.tok {
                Tok::Signature(s, _, _) => {
                    if let Some(_) = index_map.insert((*s).to_string(), index_counter) {
                        haderr = true;
                        error(AsmErr { message: "duplicate index definition", line: token.line, column: token.column, context: None });
                    }
                    index_counter += 1;
                }
                _ => {}
            }
            _ => {}
        }
    }
    counts[1] = index_counter as u32;
    cseq = Section::None;
    for (i, token) in toks.iter_mut().enumerate() {
        match token.tok {
            Tok::Section(s) => {cseq = s;cpos = 0;parent_label = "";sec_toks.push(token.clone());continue;}
            Tok::Newline => {continue;}
            _ => {}
        }
        match cseq {
            Section::Conf => match &token.tok {
                Tok::KwDecl(kw, _) => {
                    let i = match kw {
                        Keyword::Purpose => 0,
                        Keyword::Name => 1,
                        Keyword::Invar => 2,
                        Keyword::Some => 3,
                        Keyword::None => 4,
                        Keyword::Fstr => 5,
                        _ => {panic!("parser failure, bad keyword in conf section");}
                    };
                    if config_map[i].is_some() {
                        haderr = true;
                        error(AsmErr { message: "duplicate config declaration", line: token.line, column: token.column, context: None });
                    } else {
                        config_map[i] = Some(token.clone());
                    }
                }
                _ => {panic!("parse failure, non KwDecl token in Conf section");}
            }
            Section::Data => match &token.tok {
                Tok::Label(v) => {
                    let use_v = match v.starts_with(".") {
                        true => parent_label.to_owned() + v,
                        _ => {parent_label = v;(*v).to_owned()}
                    };
                    if parent_label.len() == 0 {
                        error(AsmErr { message: "invalid child label definition", line: token.line, column: token.column, context: None });
                        haderr = true;
                    }
                    if let Some(_) = label_map.insert(use_v, (min_size(&Tok::UInt(cpos)), cpos, Refs::Data)) {
                        error(AsmErr { message: "duplicate label definition", line: token.line, column: token.column, context: None });
                        haderr = true;
                    }
                }
                Tok::KwDecl(kw, v) => {
                    counts[0] += 1;
                    match kw {
                        Keyword::F64 => {cpos += 8;}
                        Keyword::S32 => {cpos += 4;}
                        Keyword::Res => {cpos += match v.as_ref() {&Tok::UInt(val)=>val,_=>{unreachable!();}};}
                        Keyword::Str => {cpos += match v.as_ref() {&Tok::String(val)=>val.len(),_=>{unreachable!();}} as u64;}
                        _ => {unreachable!();}
                    }
                }
                _ => {panic!("unexpected token type in data section");}
            }
            Section::Code => match &mut token.tok {
                Tok::Label(v) => {
                    let use_v = match v.starts_with(".") {
                        true => parent_label.to_owned() + v,
                        _ => {parent_label = v;(*v).to_owned()}
                    };
                    if parent_label.len() == 0 {
                        error(AsmErr { message: "invalid child label definition", line: token.line, column: token.column, context: None });
                        haderr = true;
                    }
                    if let Some(_) = label_map.insert(use_v, (min_size(&Tok::UInt(cpos)), cpos, Refs::Code)) {
                        error(AsmErr { message: "duplicate label definition", line: token.line, column: token.column, context: None });
                        haderr = true;
                    }
                }
                Tok::Instruction(mnemonic, prefixes, operands, relto) => {
                    let mut count = 1u64;
                    let mut halfs = 0u64;
                    match relto {
                        RelTo::Data | RelTo::Invar => {count += 1;}
                        RelTo::Reg(_) => {count += 2;}
                        RelTo::None => {}
                    }
                    count += prefixes.len() as u64;
                    let mut era = false;
                    let mut sign = prefixes.contains(&Prefix::Sign);
                    let call = prefixes.contains(&Prefix::Call);
                    if (*mnemonic).eq(&Mnemonic::MOV) {
                        if operands[0].0 == Operand::RMem {
                            if match operands[0].1.tok.clone() {
                                Tok::Deref(i) => i.len(),
                                _ => unreachable!()
                            } == 1 {
                                prefixes.push(Prefix::Oprev);
                                cpos += 1;
                            }
                        }
                    }
                    if (*mnemonic).is_jmp() {
                        cpos += 2;
                        if call || (*mnemonic).eq(&Mnemonic::CALL) {
                            cpos += 1;
                        }
                        if operands[0].0 == Operand::Imm {
                            if call || (*mnemonic).eq(&Mnemonic::CALL) {
                                if match &operands[0].1.tok {
                                    Tok::Word(w) => index_map.contains_key(*w),
                                    _ => false
                                } {
                                    cpos += 2;
                                } else {
                                    cpos += 4;
                                }
                            } else {
                                cpos += 2;
                            }
                        }
                        continue;
                    }
                    for op in operands.iter_mut() {
                        if op.0 == Operand::XReg {
                            match mnemonic {
                                Mnemonic::ADD|Mnemonic::SUB|Mnemonic::MUL|Mnemonic::IMUL|Mnemonic::DIV|Mnemonic::IDIV|
                                Mnemonic::CMP|Mnemonic::TEST|Mnemonic::CMPXCHG|Mnemonic::MOV => {
                                    if !era {
                                        era = true;
                                        prefixes.push(Prefix::Era);
                                        cpos += 1;
                                    }
                                }
                                Mnemonic::PUSH|Mnemonic::POP => {era=true;}
                                _ => {
                                    haderr = true;
                                    error(AsmErr { message: &format!("'{:?}' instruction is not ERA COMPAT", mnemonic), line: token.line, column: token.column, context: None });
                                }
                            }
                        }
                        if op.0 == Operand::Imm {
                            match op.1.tok {
                                Tok::SInt(_) => {if !sign {sign = true;prefixes.push(Prefix::Sign);cpos += 1;}}
                                _ => {}
                            }
                        }
                    }
                    for (j, op) in operands.iter().enumerate() {
                        match op.0 {
                            Operand::XReg|Operand::Reg => {
                                if era {
                                    count += 1;
                                } else {
                                    halfs += 1;
                                }
                            }
                            Operand::AMem|Operand::RMem => {
                                halfs += 1;
                                match &op.1.tok {
                                    Tok::Addr(val) => {count += min_size(&Tok::UInt(*val as u64));}
                                    Tok::Deref(inner) => {
                                        for itok in inner {
                                            match itok.tok {
                                                Tok::SInt(_)|Tok::UInt(_) => {count += min_size(&itok.tok);}
                                                _ => {unreachable!();}
                                            }
                                        }
                                    }
                                    _ => {unreachable!();}
                                }
                            }
                            Operand::Imm => {
                                halfs += 1;
                                match &op.1.tok {
                                    Tok::Float(_) => {count += match prefixes.contains(&Prefix::QWord) {true=>8,_=>4};}
                                    Tok::SInt(_) | Tok::UInt(_) => {count += min_size(&op.1.tok);}
                                    Tok::Word(word) => {
                                        match label_map.get(*word) {
                                            Some(v) => {count += v.0;}
                                            None => {count += 4;sized_toks.push((i, j, 4));}
                                        }
                                    }
                                    _ => {unreachable!();}
                                }
                            }
                        }
                    }
                    // print!("{mnemonic:?}, {cpos}, {count}, {halfs}");
                    cpos += count + (halfs >> 1) + (halfs&1);
                    // println!(", {cpos}");
                }
                _ => {panic!("unexpected token type in code section");}
            }
            _ => {}
        }
    }
    for sizing in sized_toks {
        let t = &mut toks[sizing.0];
        match &mut t.tok {
            Tok::Instruction(_, _, ops, _) => {
                ops[sizing.1].1.tok = Tok::Sized(sizing.2, Box::new(Tok::Word(match ops[sizing.1].1.tok{Tok::Word(word)=>word,_=>{unreachable!();}})));
            }
            _ => {unreachable!();}
        }
    }
    if haderr {
        return None;
    }
    return Some((label_map, index_map, toks, config_map, sec_toks, counts));
}

fn make_valbytes<'a>(op: &(Operand, Token<'a>), fpd: bool, vsize: i32, parent_label: &'a str, label_map: &'a HashMap<String, (u64, u64, Refs)>) -> Result<Vec<u8>, ()> {
    match op.0 {
        Operand::AMem => {
            let addr = match op.1.tok {Tok::Addr(v)=>v,_=>{unreachable!();}};
            let ms = min_size(&op.1.tok) as u8;
            return Ok((&addr.to_be_bytes()[(4-(ms as usize))..4]).to_vec());
        }
        Operand::RMem => {
            let mut osize;
            match &op.1.tok {
                Tok::Deref(inner) => {
                    if inner.len() < 2 {
                        panic!("parse failure, RMem deref has < 2 components");
                    }
                    match inner[1].tok {
                        Tok::UInt(v) => {
                            osize = min_size(&inner[1].tok) as u8;
                            // RMem is interpreted as a two's compliment signed value
                            // must ensure that the high bit is not one for a non-negative value
                            if v.leading_zeros() % 8 == 0 {
                                osize += 1;
                                if osize > 8 {
                                    panic!("size overflow");
                                }
                            }
                            return Ok((&v.to_be_bytes()[(8-(osize as usize))..]).to_vec());
                        }
                        Tok::SInt(v) => {
                            // non-negative adjustments already taken into account by min_size
                            // for SInt type
                            osize = min_size(&inner[1].tok) as u8;
                            return Ok((&v.to_be_bytes()[(8-(osize as usize))..]).to_vec());
                        }
                        _ => {panic!("parse failure, RMem deref has non integer second component");}
                    }
                }
                _ => {unreachable!();}
            };
        }
        Operand::Imm => {
            match op.1.tok {
                Tok::Sized(_,_)|
                Tok::Word(_) => {
                    let word;let size;
                    match &op.1.tok {
                        Tok::Word(w) => {word = *w;size = 0;}
                        Tok::Sized(s, wt) => {word = *match wt.as_ref() {Tok::Word(w)=>w,_=>{unreachable!();}};size = *s;}
                        _ => {unreachable!();}
                    }
                    if fpd {
                        error(AsmErr { message: "illegal fp conversion", line: op.1.line, column: op.1.column, context: None });
                        return Err(());
                    }
                    let lab = match word.starts_with(".") {
                        true => parent_label.to_owned()+word,
                        _ => word.to_owned()
                    };
                    if let Some(v) = label_map.get(word) {
                        return Ok((&(v.1.to_be_bytes())[(8-((match size == 0 {true=>v.0,_=>size}) as usize))..]).to_vec());
                    } else {
                        error(AsmErr { message: &format!("cannot find label '{}'", lab), line: op.1.line, column: op.1.column, context: None });
                        return Err(());
                    }
                }
                _ => {
                    if fpd {
                        if vsize < 4 {
                            error(AsmErr { message: "float literals require at least dword size", line: op.1.line, column: op.1.column, context: None });
                            return Err(());
                        }
                        let val = match op.1.tok {
                            Tok::Float(v) => v,
                            Tok::UInt(v) => v as f64,
                            Tok::SInt(v) => v as f64,
                            _ => {unreachable!();}
                        };
                        if vsize == 4 {
                            return Ok((val as f32).to_be_bytes().to_vec());
                        } else {
                            return Ok(val.to_be_bytes().to_vec());
                        }
                    } else {
                        match op.1.tok {
                            Tok::Float(_) => {
                                error(AsmErr { message: "must mark instruction as 'fp' when using float literals", line: op.1.line, column: op.1.column, context: None });
                                return Err(());
                            }
                            Tok::UInt(_) | Tok::SInt(_) => {
                                let s = min_size(&op.1.tok) as usize;
                                return Ok(match op.1.tok {
                                    Tok::UInt(v) => v.to_be_bytes()[(8-s)..].to_vec(),
                                    Tok::SInt(v) => v.to_be_bytes()[(8-s)..].to_vec(),
                                    _ => {unreachable!();}
                                });
                            }
                            _ => {unreachable!();}
                        }
                    }
                }
            }
        }
        _ => {panic!("cannot construct valbytes from non AMem,RMem,Imm types");}
    }
}

pub fn emit(dstfile: &str, toks: Vec<Token>, dry: bool) -> io::Result<bool> {
    let mut output: [Vec<Box<[u8]>>;4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    // (size, ptr, ref)
    let (label_map, index_map, toks, config_map, sec_toks, counts) = match construct_maps(toks) {
        Some(v) => v,
        None => {return Ok(false);}
    };
    {
        let confst = match sec_toks.iter().find(|x|match (*x).tok {Tok::Section(s)=>s==Section::Conf,_=>{unreachable!();}}) {
            Some(v) => v,
            None => {error(AsmErr { message: "missing config section", line: toks.last().unwrap().line, column: toks.last().unwrap().column, context: None });return Ok(false);}
        };
        if config_map[0].is_none() {
            error(AsmErr { message: "missing purpose", line: confst.line, column: confst.column, context: None });
            return Ok(false);
        }
        let ptok = &(config_map[0].as_ref().unwrap().tok);
        let pur = *match ptok {Tok::KwDecl(_,t)=>match t.as_ref() {Tok::UInt(v)=>v,_=>unreachable!()},_=>unreachable!()};
        output[0].push([pur as u8].to_vec().into_boxed_slice());
        if let Some(t) = config_map[1].as_ref() {
            match &t.tok {
                Tok::KwDecl(_, p) => match p.as_ref() {
                    Tok::String(s) => {output[0].push([&[s.len() as u8], (*s).as_bytes()].concat().into_boxed_slice());}
                    _ => {unreachable!();}
                }
                _ => {unreachable!();}
            }
        } else {
            error(AsmErr { message: "missing name declaration", line: confst.line, column: confst.column, context: None });
            return Ok(false);
        }
        if let Some(t) = config_map[2].as_ref() {
            output[0].push([*match &t.tok {Tok::KwDecl(_,p)=>match p.as_ref() {Tok::UInt(v)=>v,_=>unreachable!()},_=>unreachable!()} as u8].to_vec().into_boxed_slice());
        } else {
            warn(AsmErr { message: "missing invar declaration, defaulting to 0", line: confst.line, column: confst.column, context: None });
            output[0].push([0].to_vec().into_boxed_slice());
        }
        if pur != 0 {
            output[1].push([0,0].to_vec().into_boxed_slice());
            output[0].push([0,0,0,0].to_vec().into_boxed_slice());
            if let Some(t) = config_map[5].as_ref() {
                error(AsmErr { message: "fstr must not be declared for non 3tr purpose", line: t.line, column: t.column, context: None });
                return Ok(false);
            }
            for i in 3..5 {
                if let Some(t) = config_map[i].as_ref() {
                    warn(AsmErr { message: "some and none are ignored for non 3tr purposes", line: t.line, column: t.column, context: None });
                }
            }
        } else {
            if let Some(t) = config_map[5].as_ref() {
                let s = *match &t.tok {Tok::KwDecl(_,p)=>match p.as_ref() {Tok::String(s)=>s,_=>unreachable!()},_=>unreachable!()};
                output[1].push([&((s.len() as u16).to_be_bytes()), s.as_bytes()].concat().into_boxed_slice());
            } else {
                error(AsmErr { message: "missing fstr declaration", line: confst.line, column: confst.column, context: None });
                return Ok(false);
            }
            for i in 3..5 {
                if let Some(t) = config_map[i].as_ref() {
                    let v = *match &t.tok {Tok::KwDecl(_,p)=>match p.as_ref() {Tok::UInt(n)=>n,_=>unreachable!()},_=>unreachable!()};
                    output[0].push((v as u16).to_be_bytes().to_vec().into_boxed_slice());
                } else {
                    error(AsmErr { message: "missing some/none declaration", line: confst.line, column: confst.column, context: None });
                    return Ok(false);
                }
            }
        }
    }
    output[1].push((counts[0] as u16).to_be_bytes().to_vec().into_boxed_slice());
    output[3].push((counts[1] as u16).to_be_bytes().to_vec().into_boxed_slice());
    // let mut label_map: HashMap<String, (u64, u64, Refs)> = HashMap::new();
    // let mut index_map: Vec<&str> = Vec::new();
    // let mut cpos = 0u64;
    // let mut cseq = Section::None;
    let mut parent_label: &str = "";
    let mut haderr = false;
    // for token in &toks {
    //     match token.tok {
    //         Tok::Section(s) => {cseq = s;cpos = 0;parent_label = "";continue;}
    //         Tok::Newline => {continue;}
    //         _ => {}
    //     }
    //     match cseq {
    //         Section::Code => match &token.tok {
    //             Tok::Label(v) => {
    //                 let use_v = match v.starts_with(".") {
    //                     true => parent_label.to_owned() + v,
    //                     _ => {parent_label = v;(*v).to_owned()}
    //                 };
    //                 if parent_label.len() == 0 {
    //                     error(AsmErr { message: "invalid child label definition", line: token.line, column: token.column, context: None });
    //                     haderr = true;
    //                 }
    //                 if let Some(_) = label_map.insert(use_v, (min_size(Tok::UInt(cpos)), cpos, Refs::Code)) {
    //                     error(AsmErr { message: "duplicate label definition", line: token.line, column: token.column, context: None });
    //                     haderr = true;
    //                 }
    //             }
    //             Tok::Instruction(mnemonic, prefixes, operands, relto) => {
    //                 //
    //             }
    //             _ => {panic!("unexpected token type in code section");}
    //         }
    //         _ => {}
    //     }
    // }
    if dry {
        println!("{label_map:?}");
        println!("{index_map:?}");
        println!("{toks:?}");
    }
    let mut cseq = Section::None;
    'outer: for token in &toks {
        match token.tok {
            Tok::Section(s) => {
                cseq = s;
                // cpos = 0;
                parent_label = "";
                continue;
            }
            Tok::Newline => {continue;}
            _ => {}
        }
        match cseq {
            Section::Indx => match &token.tok {
                Tok::Signature(id, args, rtype) => {
                    output[3].push([&([id.len() as u8])[..], (*id).as_bytes()].concat().into_boxed_slice());
                    output[3].push((label_map.get(*id).unwrap().1 as u32).to_be_bytes().to_vec().into_boxed_slice());
                    output[3].push([args.len() as u8].to_vec().into_boxed_slice());
                    match *id {
                        "@constructor" => {
                            if rtype.auto_eq(&Type::Void) {
                                haderr = true;
                                error(AsmErr { message: "@constructor must be void", line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                            for arg in args {
                                if let Tok::Param(name, ty) = arg {
                                    if !ty.auto_eq(&Type::U32) {
                                        haderr = true;
                                        error(AsmErr { message: "@constructor parameters must be auto or u32", line: token.line, column: token.column, context: None });
                                        continue 'outer;
                                    }
                                    output[3].push([&([name.len() as u8])[..], (*name).as_bytes()].concat().into_boxed_slice());
                                } else {
                                    unreachable!();
                                }
                            }
                            output[3].push(Type::Void.to_bytes());
                        }
                        "@getpositionof" => {
                            if !rtype.auto_eq(&Type::Uarr(Box::new(Type::U16))) {
                                haderr = true;
                                error(AsmErr { message: "@getpositionof must return u16[] or auto", line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                        }
                        "@getneighbors" => {
                            if !rtype.auto_eq(&Type::Uarr(Box::new(Type::U32))) {
                                haderr = true;
                                error(AsmErr { message: "@getneighbors must return u32[] or auto", line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                        }
                        "@getrequiredbits" => {
                            if !rtype.auto_eq(&Type::U8) {
                                haderr = true;
                                error(AsmErr { message: "@getrequiredbits must return u8 or auto", line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                        }
                        "@think" => {
                            if !rtype.auto_eq(&Type::U32) {
                                haderr = true;
                                error(AsmErr { message: "@think must return u32 or auto", line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                        }
                        _ => {
                            let (n,t): (Vec<_>,Vec<_>) = args.iter().map(|x|match x {Tok::Param(n, t)=>(*n,t),_=>{panic!("parser fail, unexpected token in indx section");}}).unzip();
                            for name in n {
                                output[3].push([&([name.len() as u8])[..], name.as_bytes()].concat().into_boxed_slice());
                            }
                            for ty in t {
                                output[3].push(ty.to_bytes());
                            }
                            output[3].push(rtype.to_bytes());
                        }
                    }
                }
                _ => {panic!("parser fail, unexpected token type in indx section");}
            }
            Section::Code => match &token.tok {
                Tok::Label(s) => {parent_label=*s;}
                Tok::Instruction(mnemonic, prefixes, operands, relto) => {
                    let mut build: Vec<u8> = Vec::new();
                    let mut sized = false;
                    let mut fpd = false;
                    let mut call = false;
                    let mut era = false;
                    let mut vsize = 4;
                    for prefix in prefixes {
                        match prefix {
                            Prefix::Byte|Prefix::Word|Prefix::DWord|Prefix::QWord => {
                                if sized {
                                    haderr = true;
                                    error(AsmErr { message: "conflicting sizes", line: token.line, column: token.column, context: None });
                                    continue 'outer;
                                }
                                sized = true;
                                match prefix {
                                    Prefix::Byte => {build.push(0x40);vsize=1;}
                                    Prefix::Word => {build.push(0x41);vsize=2;}
                                    Prefix::DWord => {build.push(0x42);vsize=4;}
                                    Prefix::QWord => {build.push(0x43);vsize=8;}
                                    _ => {unreachable!();}
                                }
                            }
                            Prefix::Call => {
                                if call {
                                    haderr = true;
                                    error(AsmErr { message: "duplicate call prefix", line: token.line, column: token.column, context: None });
                                    continue 'outer;
                                }
                                call = true;
                                build.push(0x45);
                            }
                            Prefix::Fp => {
                                if fpd {
                                    haderr = true;
                                    error(AsmErr { message: "duplicate fp prefix", line: token.line, column: token.column, context: None });
                                    continue 'outer;
                                }
                                fpd = true;
                                build.push(0x51);
                            }
                            Prefix::Sign => {build.push(0x50);}
                            Prefix::Oprev => {build.push(0x46);}
                            Prefix::Era => {build.push(0x44);era=true;}
                        }
                    }
                    match relto {
                        RelTo::None => {}
                        RelTo::Data => {build.push(0x4b);}
                        RelTo::Invar => {build.push(0x4c);}
                        RelTo::Reg(r) => {build.push(0x49);build.push(0x40|r.value());}
                    }
                    match mnemonic {
                        Mnemonic::ADD|Mnemonic::SUB|Mnemonic::MUL|Mnemonic::DIV|Mnemonic::IMUL|Mnemonic::IDIV|Mnemonic::CMP => match operands[1].0 {
                            Operand::Reg|Operand::XReg => {
                                build.push(match mnemonic {
                                    Mnemonic::ADD => 0,
                                    Mnemonic::SUB => 3,
                                    Mnemonic::MUL|Mnemonic::IMUL => 6,
                                    Mnemonic::DIV|Mnemonic::IDIV => 12,
                                    Mnemonic::CMP => 0x1d,
                                    _ => {unreachable!();}
                                });
                                if era {
                                    build.push(match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                    build.push(match operands[1].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                } else {
                                    build.push((match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}}<<4)|match operands[1].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                }
                            }
                            Operand::AMem|Operand::RMem => {
                                build.push(match mnemonic {
                                    Mnemonic::ADD => 1,
                                    Mnemonic::SUB => 4,
                                    Mnemonic::MUL|Mnemonic::IMUL => 7,
                                    Mnemonic::DIV|Mnemonic::IDIV => 13,
                                    Mnemonic::CMP => 0x1e,
                                    _ => {unreachable!();}
                                });
                                match operands[1].0 {
                                    Operand::AMem => {
                                        let addr = match operands[1].1.tok {Tok::Addr(v)=>v,_=>{unreachable!();}};
                                        let ms = min_size(&operands[1].1.tok) as u8;
                                        if era {
                                            build.push(match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                            build.push(ms>>1);
                                        } else {
                                            build.push((match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}}<<4)|(ms>>1));
                                        }
                                        build.extend_from_slice(&addr.to_be_bytes()[(4-(ms as usize))..4]);
                                    }
                                    Operand::RMem => {
                                        let offset;let mut osize;
                                        match &operands[1].1.tok {
                                            Tok::Deref(inner) => {
                                                if inner.len() < 2 {
                                                    panic!("parse failure, RMem deref has < 2 components");
                                                }
                                                match inner[1].tok {
                                                    Tok::UInt(v) => {
                                                        osize = min_size(&inner[1].tok) as u8;
                                                        // RMem is interpreted as a two's compliment signed value
                                                        // must ensure that the high bit is not one for a non-negative value
                                                        if v.leading_zeros() % 8 == 0 {
                                                            osize += 1;
                                                            if osize > 8 {
                                                                panic!("size overflow");
                                                            }
                                                        }
                                                        offset = (&v.to_be_bytes()[(8-(osize as usize))..]).to_vec();
                                                    }
                                                    Tok::SInt(v) => {
                                                        // non-negative adjustments already taken into account by min_size
                                                        // for SInt type
                                                        osize = min_size(&inner[1].tok) as u8;
                                                        offset = (&v.to_be_bytes()[(8-(osize as usize))..]).to_vec();
                                                    }
                                                    _ => {panic!("parse failure, RMem deref has non integer second component");}
                                                }
                                            }
                                            _ => {unreachable!();}
                                        };
                                        if era {
                                            build.push(match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                            build.push(osize>>1);
                                        } else {
                                            build.push((match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}}<<4)|(osize>>1));
                                        }
                                        build.extend_from_slice(&offset);
                                    }
                                    _ => {unreachable!();}
                                }
                            }
                            Operand::Imm => {
                                build.push(match mnemonic {
                                    Mnemonic::ADD => 2,
                                    Mnemonic::SUB => 5,
                                    Mnemonic::MUL|Mnemonic::IMUL => 8,
                                    Mnemonic::DIV|Mnemonic::IDIV => 14,
                                    Mnemonic::CMP => 0x1f,
                                    _ => {unreachable!();}
                                });
                                let valbytes;
                                match operands[1].1.tok {
                                    Tok::Sized(_,_)|
                                    Tok::Word(_) => {
                                        let word;let size;
                                        match &operands[1].1.tok {
                                            Tok::Word(w) => {word = *w;size = 0;}
                                            Tok::Sized(s, wt) => {word = *match wt.as_ref() {Tok::Word(w)=>w,_=>{unreachable!();}};size = *s;}
                                            _ => {unreachable!();}
                                        }
                                        if fpd {
                                            haderr = true;
                                            error(AsmErr { message: "illegal fp conversion", line: operands[1].1.line, column: operands[1].1.column, context: None });
                                            continue 'outer;
                                        }
                                        let lab = match word.starts_with(".") {
                                            true => parent_label.to_owned()+word,
                                            _ => word.to_owned()
                                        };
                                        if let Some(v) = label_map.get(word) {
                                            valbytes = (&(v.1.to_be_bytes())[(8-((match size == 0 {true=>v.0,_=>size}) as usize))..]).to_vec();
                                        } else {
                                            haderr = true;
                                            error(AsmErr { message: &format!("cannot find label '{}'", lab), line: operands[1].1.line, column: operands[1].1.column, context: None });
                                            continue 'outer;
                                        }
                                    }
                                    _ => {
                                        if fpd {
                                            if vsize < 4 {
                                                haderr = true;
                                                error(AsmErr { message: "float literals require at least dword size", line: operands[1].1.line, column: operands[1].1.column, context: None });
                                                continue 'outer;
                                            }
                                            let val = match operands[1].1.tok {
                                                Tok::Float(v) => v,
                                                Tok::UInt(v) => v as f64,
                                                Tok::SInt(v) => v as f64,
                                                _ => {unreachable!();}
                                            };
                                            if vsize == 4 {
                                                valbytes = (val as f32).to_be_bytes().to_vec();
                                            } else {
                                                valbytes = val.to_be_bytes().to_vec();
                                            }
                                        } else {
                                            match operands[1].1.tok {
                                                Tok::Float(_) => {
                                                    haderr = true;
                                                    error(AsmErr { message: "must mark instruction as 'fp' when using float literals", line: operands[1].1.line, column: operands[1].1.column, context: None });
                                                    continue 'outer;
                                                }
                                                Tok::UInt(_) | Tok::SInt(_) => {
                                                    let s = min_size(&operands[1].1.tok) as usize;
                                                    valbytes = match operands[1].1.tok {
                                                        Tok::UInt(v) => v.to_be_bytes()[(8-s)..].to_vec(),
                                                        Tok::SInt(v) => v.to_be_bytes()[(8-s)..].to_vec(),
                                                        _ => {unreachable!();}
                                                    };
                                                }
                                                _ => {unreachable!();}
                                            }
                                        }
                                    }
                                }
                                if era {
                                    build.push(match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}});
                                    build.push((valbytes.len() as u8)>>1);
                                } else {
                                    build.push((match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>{unreachable!();}}<<4)|((valbytes.len().trailing_zeros() as u8)));
                                }
                                build.extend_from_slice(&valbytes);
                            }
                        }
                        Mnemonic::SHL|Mnemonic::SHR|Mnemonic::SAR|Mnemonic::XCHG => {
                            build.push(match mnemonic {
                                Mnemonic::SHL => 0x0f,
                                Mnemonic::SHR => 0x10,
                                Mnemonic::SAR => 0x11,
                                Mnemonic::XCHG => 0x20,
                                _ => unreachable!()
                            });
                            build.push((match operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()}<<4)|match operands[1].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()});
                        }
                        Mnemonic::XOR|Mnemonic::OR|Mnemonic::AND => {
                            let rx = match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()};
                            match operands[1].0 {
                                Operand::Reg|Operand::XReg => {
                                    build.push(match mnemonic {
                                        Mnemonic::XOR => 0x12,
                                        Mnemonic::OR => 0x15,
                                        Mnemonic::AND => 0x18,
                                        _ => unreachable!()
                                    });
                                    build.push((rx<<4)|match &operands[1].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()});
                                }
                                _ => {
                                    build.push(match operands[1].0 {
                                        Operand::Imm => match mnemonic {
                                            Mnemonic::XOR => 0x14,
                                            Mnemonic::OR => 0x17,
                                            Mnemonic::AND => 0x1a,
                                            _ => unreachable!()
                                        }
                                        _ => match mnemonic {
                                            Mnemonic::XOR => 0x13,
                                            Mnemonic::OR => 0x16,
                                            Mnemonic::AND => 0x19,
                                            _ => unreachable!()
                                        }
                                    });
                                    let valbytes = match make_valbytes(&operands[1], fpd, vsize, parent_label, &label_map) {Ok(v)=>v,Err(_)=>{haderr=true;continue 'outer;}};
                                    build.push((rx<<4)|(valbytes.len().trailing_zeros() as u8));
                                    build.extend_from_slice(&valbytes);
                                }
                            }
                        }
                        Mnemonic::NOT => {
                            build.push(0x12);
                            build.push((match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()}<<4)|13);
                        }
                        Mnemonic::TEST => {
                            build.push(0x1d);
                            let rx = match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()};
                            if era {
                                build.push(rx);
                                build.push(15);
                            } else {
                                build.push((rx<<4)|15);
                            }
                        }
                        Mnemonic::PUSH|Mnemonic::POP => {
                            build.push(match mnemonic {
                                Mnemonic::PUSH => 0x1b,
                                Mnemonic::POP => 0x1c,
                                _ => unreachable!()
                            });
                            build.push(match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()});
                        }
                        Mnemonic::MOV => {
                            match operands[0].0 {
                                Operand::Reg|Operand::XReg => {
                                    let rx = match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()};
                                    match operands[1].0 {
                                        Operand::Reg|Operand::XReg => {
                                            let ry = match &operands[1].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()};
                                            if era {
                                                build.push(rx);
                                                build.push(ry);
                                            } else {
                                                build.push((rx<<4)|ry);
                                            }
                                        }
                                        Operand::AMem|Operand::RMem|Operand::Imm => {
                                            match operands[1].0 {
                                                Operand::Imm => {build.push(0x2d);}
                                                _ => {build.push(0x2c);}
                                            }
                                            let valbytes = match make_valbytes(&operands[1], fpd, vsize, parent_label, &label_map) {Ok(v)=>v,Err(_)=>{haderr=true;continue 'outer;}};
                                            if era {
                                                build.push(rx);
                                                build.push(valbytes.len() as u8);
                                            } else {
                                                build.push((rx<<4)|((valbytes.len().trailing_zeros() as u8)));
                                            }
                                            build.extend_from_slice(&valbytes);
                                        }
                                    }
                                }
                                Operand::RMem => {
                                    let ry = match &operands[0].1.tok {
                                        Tok::Deref(l) => match l[0].tok {
                                            Tok::Reg(r) => r.value(),
                                            _ => unreachable!()
                                        },
                                        _ => unreachable!()
                                    };
                                    let rx = match operands[1].1.tok {
                                        Tok::Reg(r) => r.value(),
                                        _ => unreachable!()
                                    };
                                    build.push(0x2e);
                                    if era {
                                        build.push(rx);
                                        build.push(ry);
                                    } else {
                                        build.push((rx<<4)|ry);
                                    }
                                }
                                _ => {unreachable!();}
                            }
                        }
                        Mnemonic::JMP|Mnemonic::JA|Mnemonic::JB|Mnemonic::JG|Mnemonic::JL|Mnemonic::JE|Mnemonic::CALL|
                        Mnemonic::JNE|Mnemonic::JAE|Mnemonic::JBE|Mnemonic::JGE|Mnemonic::JLE|Mnemonic::JZ|Mnemonic::JNZ => {
                            let mut opcode: u16 = match mnemonic {
                                Mnemonic::JMP|Mnemonic::CALL => 0x2200,
                                Mnemonic::JE|Mnemonic::JZ => 0x2300,
                                Mnemonic::JNZ|Mnemonic::JNE => 0x2400,
                                Mnemonic::JL => 0x2500,
                                Mnemonic::JB => 0x2501,
                                Mnemonic::JLE => 0x2600,
                                Mnemonic::JBE => 0x2601,
                                Mnemonic::JG => 0x2700,
                                Mnemonic::JA => 0x2701,
                                Mnemonic::JGE => 0x2800,
                                Mnemonic::JAE => 0x2801,
                                _ => unreachable!()
                            };
                            match operands[0].0 {
                                Operand::Reg|Operand::XReg => {
                                    let rx = match &operands[0].1.tok {Tok::Reg(r)=>r.value(),_=>unreachable!()} as u16;
                                    opcode |= rx << 4;
                                    if call {
                                        opcode |= 2;
                                    }
                                    build.extend_from_slice(&(opcode.to_be_bytes()));
                                }
                                Operand::Imm => {
                                    opcode |= 4;
                                    let immbytes;
                                    match &operands[0].1.tok {
                                        Tok::Word(sym) => {
                                            if let Some(iv) = index_map.get(*sym) {
                                                immbytes = (*iv as u16).to_be_bytes().to_vec();
                                            } else if let Some(ov) = label_map.get(*sym) {
                                                if call {
                                                    opcode |= 2;
                                                    immbytes = (ov.1 as u32).to_be_bytes().to_vec();
                                                } else {
                                                    immbytes = (ov.1 as u16).to_be_bytes().to_vec();
                                                }
                                            } else {
                                                panic!("parser failure, undeclared label");
                                            }
                                        }
                                        Tok::UInt(v) => {
                                            opcode |= 8;
                                            immbytes = (*v as u16).to_be_bytes().to_vec();
                                        }
                                        Tok::SInt(v) => {
                                            opcode |= 8;
                                            immbytes = (&(v.to_be_bytes())[6..]).to_vec();
                                        }
                                        _ => {panic!("parser failure, bad immediate jump operand {:?}", operands[0].1.tok);}
                                    }
                                    build.extend_from_slice(&opcode.to_be_bytes());
                                    build.extend_from_slice(&immbytes);
                                }
                                _ => {panic!("parser failure, non-reg, non-imm value as jump operand");}
                            }
                        }
                        Mnemonic::RET => {build.push(0x29);}
                        Mnemonic::HLT => {build.push(0x3f);}
                        Mnemonic::SYSCALL => {build.push(0x2a);}
                        _ => {
                            if !dry {
                                haderr = true;
                                error(AsmErr { message: &format!("unimplemented mnemonic '{:?}'", mnemonic), line: token.line, column: token.column, context: None });
                                continue 'outer;
                            }
                        }
                    }
                    output[2].push(build.into_boxed_slice());
                }
                _ => {}
            }
            Section::Data => match &token.tok {
                Tok::Label(_) => {
                    // if !v.starts_with(".") {
                    //     parent_label = *v;
                    // }
                }
                Tok::KwDecl(kw, val) => match kw {
                    Keyword::Str => match val.as_ref() {
                        Tok::String(s) => {output[1].push([&[1, s.len() as u8], (*s).as_bytes()].concat().into_boxed_slice());}
                        _ => {unreachable!();}
                    }
                    Keyword::F64 => match val.as_ref() {
                        Tok::Float(v) => {output[1].push([&([2u8])[..], &(v.to_be_bytes())].concat().into_boxed_slice());}
                        _ => {unreachable!();}
                    }
                    Keyword::Res => match val.as_ref() {
                        Tok::UInt(v) => {output[1].push([&([3u8])[..], &((*v as u16).to_be_bytes())].concat().into_boxed_slice());}
                        _ => {unreachable!();}
                    }
                    Keyword::S32 => match val.as_ref() {
                        Tok::SInt(v) => {output[1].push([&([0u8][..]), &((*v as i32).to_be_bytes())].concat().into_boxed_slice());}
                        _ => {unreachable!();}
                    }
                    _ => {}
                }
                _ => {}
            }
            _ => {}
        }
    }
    if haderr {
        return Ok(false);
    }
    if dry {
        println!("{:?}", output[0]);
        println!("{:?}", output[1]);
        println!("{:?}", output[2]);
        println!("{:?}", output[3]);
        return Ok(true);
    }
    if dstfile.len() == 0 {
        return Ok(false);
    }
    let mut file = File::options().truncate(true).create(true).write(true).open(dstfile)?;
    // output[0].push(Box::from("SECTION.conf".as_bytes()));
    // output[1].push(Box::from("SECTION.data".as_bytes()));
    // output[2].push(Box::from("SECTION.code".as_bytes()));
    // output[3].push(Box::from("SECTION.indx".as_bytes()));
    file.write_all(&[1])?;
    file.write_all("SECTION.conf".as_bytes())?;
    let secs = output.map(|x|x.concat());
    file.write_all(&((secs[0].len() as u32).to_be_bytes()))?;
    file.write_all(secs[0].as_slice())?;
    file.write_all("SECTION.data".as_bytes())?;
    file.write_all(&((secs[1].len() as u32).to_be_bytes()))?;
    // file.write_all("E".as_bytes())?;
    file.write_all(secs[1].as_slice())?;
    file.write_all("SECTION.code".as_bytes())?;
    file.write_all(&((secs[2].len() as u32).to_be_bytes()))?;
    file.write_all(secs[2].as_slice())?;
    file.write_all("SECTION.indx".as_bytes())?;
    file.write_all(&((secs[3].len() as u32).to_be_bytes()))?;
    file.write_all(secs[3].as_slice())?;
    // file.write_all(output.iter().map(|x|x.concat()).collect::<Vec<_>>().concat().as_slice())?;
    Ok(true)
}
