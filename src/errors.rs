#[derive(Debug)]
pub struct AsmErr<'a> {
    pub message: &'a str,
    pub line: u32,
    pub column: u32,
    pub context: Option<&'a str>,
}

pub fn warn(e: AsmErr) -> () {
    println!("{:?}", e);
}

pub fn error(e: AsmErr) -> () {
    println!("{:?}", e);
}
