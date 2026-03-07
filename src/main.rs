use std::env::args;
use std::fs;
use tas::emit::emit;
use tas::types::*;
use tas::parse::{lex, semantic_parse, syntactic_parse};

struct Params {
    pub s: String,
    pub o: String,
    pub lex_only: bool,
    pub use_stdout: bool,
    pub syn_parse_only: bool,
    pub sem_parse_only: bool,
    pub dry_run: bool,
}
impl Default for Params {
    fn default() -> Self {
        Self { s: "".to_owned(), o: "".to_owned(), lex_only: false, use_stdout: false, syn_parse_only: false, sem_parse_only: false, dry_run: false }
    }
}
impl Params {
    pub fn unpopulated(&self) -> bool {
        return self.s.len() == 0 || (!self.use_stdout && self.o.len() == 0);
    }
}

fn test() -> () {
    let testvals = vec![(Tok::UInt(0),1),(Tok::SInt(0),1),(Tok::SInt(-1),1),(Tok::UInt(0x8000),2),(Tok::SInt(-32768),2)];
    for (t, e) in testvals {
        println!("testing min_size({t:?}), expecting {e}");
        println!("result: {}", min_size(&t));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut a = args();
    let mut params = Params::default();
    loop {
        if let Some(s) = a.next() {
            if s == "-s" {
                if let Some(p) = a.next() {
                    params.s = p;
                } else {
                    panic!("cli args ended too early");
                }
            } else if s == "-o" {
                if let Some(p) = a.next() {
                    params.o = p;
                } else {
                    panic!("cli args ended too early");
                }
            } else if s.starts_with('-') {
                if s.contains('l') {
                    params.lex_only = true;
                    params.use_stdout = true;
                }
                if s.contains('t') {
                    params.use_stdout = true;
                }
                if s.contains('p') {
                    params.syn_parse_only = true;
                    params.use_stdout = true;
                }
                if s.contains('P') {
                    params.sem_parse_only = true;
                    params.use_stdout = true;
                }
                if s.contains('d') {
                    params.dry_run = true;
                    params.use_stdout = true;
                }
                if s.contains('T') {
                    test();
                    return Ok(());
                }
            }
        } else {
            break;
        }
    }
    if params.unpopulated() {
        panic!("missing args");
    }
    let sourcefile = fs::read_to_string(params.s)?;
    let tokens: Result<Vec<Token>, Vec<_>> = lex(&sourcefile);
    match tokens {
        Ok(v) => {
            if params.lex_only {
                for i in v {println!("{:?}", i);}
                return Ok(());
            }
            let ptoks = syntactic_parse(v);
            match ptoks {
                Ok(pt) => {
                    if params.syn_parse_only {
                        for i in pt {println!("{:?}", i);}
                        return Ok(());
                    }
                    let stoks = semantic_parse(pt);
                    match stoks {
                        Ok(st) => {
                            if params.sem_parse_only {
                                for i in st {println!{"{:?}", i};}
                                return Ok(());
                            }
                            if params.o.len() == 0 && !params.dry_run {
                                println!("MUST SPECIFY OUTPUT PATH FOR EMISSION");
                                return Ok(());
                            }
                            if !emit(&params.o, st, params.dry_run)? {
                                println!("EMISSION ERR");
                                return Ok(());
                            }
                        }
                        Err(_) => {println!("SEMANTIC PARSE ERR");}
                    }
                }
                Err(_) => {println!("SYNTACTIC PARSE ERR");}
            }
        },
        Err(_) => {println!("LEX ERR");}
    }
    Ok(())
}
