use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

#[derive(Debug)]
pub struct CsvError {
    msg: String,
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSV error: {}", self.msg)
    }
}

impl std::error::Error for CsvError {}

pub struct CsvReader<R: Read> {
    reader: BufReader<R>,
    buffer: String,
}

impl<R: Read> CsvReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
        }
    }

    pub fn records(&mut self) -> CsvRecords<'_, R> {
        CsvRecords { reader: self }
    }
}

pub struct CsvRecords<'a, R: Read> {
    reader: &'a mut CsvReader<R>,
}

impl<'a, R: Read> Iterator for CsvRecords<'a, R> {
    type Item = Result<Vec<String>, CsvError>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = match self.reader.reader.fill_buf().ok()? {
            buf if buf.is_empty() => return None,
            buf => {
                let len = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
                self.reader.buffer.clear();
                self.reader
                    .buffer
                    .extend(buf[..len].iter().map(|&b| b as char));
                self.reader.reader.consume(len + 1); // +1 для \n
                &self.reader.buffer[..]
            }
        };

        Some(parse_csv_line(line))
    }
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, CsvError> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' => {
                chars.next();
                if in_quotes && chars.peek() == Some(&'"') {
                    // Escaped quote ""
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                chars.next();
                fields.push(std::mem::take(&mut field));
            }
            '\r' | '\n' => break,
            _ => {
                chars.next();
                field.push(ch);
            }
        }
    }
    fields.push(field);
    Ok(fields)
}

fn main() -> io::Result<()> {
    let file = File::open("assets/example.csv")?;
    let mut csv = CsvReader::new(file);
    for row in csv.records() {
        match row {
            Ok(fields) => println!("{:?}", fields),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    Ok(())
}
