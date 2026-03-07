#[derive(Debug)]
pub struct AsmErr<'a> {
    pub message: &'a str,
    pub line: u32,
    pub column: u32,
    pub context: Option<&'a str>,
}

const DEF: &str = "\x1b[39m";
const RED: &str = "\x1b[31m";
const YEL: &str = "\x1b[33m";

pub fn warn(e: AsmErr) -> () {
    // println!("{:?}", e);
    println!("{YEL}WARNING{DEF} ({},{}): {}", e.line, e.column, e.message);
}

pub fn error(e: AsmErr) -> () {
    // println!("{:?}", e);
    println!("{RED}ERROR{DEF} ({},{}): {}", e.line, e.column, e.message);
}
