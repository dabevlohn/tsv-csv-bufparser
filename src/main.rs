use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Income,
    Expense,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum RecordFormat {
    Csv,
    Tsv,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub date: String,
    pub category: String,
    pub kind: TransactionKind,
    pub amount: i64,
}

#[derive(Debug)]
pub enum ParseError {
    Io(std::io::Error),
    Format(String, usize),
    Binary { msg: String, record: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "IO error: {}", e),
            ParseError::Format(msg, line) => write!(f, "Parse error at line {}: {}", line, msg),
            ParseError::Binary { msg, record } => {
                write!(f, "Binary error at record {}: {}", record, msg)
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e)
    }
}

pub struct Parser<R: Read> {
    reader: BufReader<R>,
    buffer: String,
    line_num: usize,
    format: RecordFormat,
}

impl<R: Read> Parser<R> {
    pub fn new(reader: R, format: RecordFormat) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
            line_num: 0,
            format,
        }
    }

    pub fn transactions(&mut self) -> ParserTransactions<'_, R> {
        ParserTransactions { parser: self }
    }
}

pub struct ParserTransactions<'a, R: Read> {
    parser: &'a mut Parser<R>,
}

impl<'a, R: Read> Iterator for ParserTransactions<'a, R> {
    type Item = Result<Transaction, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parser.format {
            RecordFormat::Csv | RecordFormat::Tsv => self.next_text(),
            RecordFormat::Binary => self.next_binary(),
        }
    }
}

impl<'a, R: Read> ParserTransactions<'a, R> {
    fn next_text(&mut self) -> Option<Result<Transaction, ParseError>> {
        let line_num = self.parser.line_num + 1;
        let line = match self.parser.reader.fill_buf().ok()? {
            buf if buf.is_empty() => return None,
            buf => {
                let len = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
                self.parser.buffer.clear();
                self.parser
                    .buffer
                    .extend(buf[..len].iter().map(|&b| b as char));
                self.parser.reader.consume(len + 1); // +1 для \n
                self.parser.line_num = line_num;
                &self.parser.buffer[..]
            }
        };

        parse_text_transaction(line, line_num, self.parser.format)
    }

    fn next_binary(&mut self) -> Option<Result<Transaction, ParseError>> {
        let reader = self.parser.reader.get_mut();
        let mut sig = [0u8; 4];
        if reader.read_exact(&mut sig).is_err() {
            return None;
        }
        if &sig != b"YPB1" {
            return Some(Err(ParseError::Binary {
                msg: "invalid signature".to_string(),
                record: 0,
            }));
        }

        let mut count_buf = [0u8; 4];
        reader.read_exact(&mut count_buf).ok()?;
        let count = u32::from_le_bytes(count_buf);

        // Пропускаем до конца заголовка (читаем count)
        for record_idx in 0..count {
            match parse_binary_transaction(reader) {
                Ok(trans) => return Some(Ok(trans)),
                Err(e) => {
                    return Some(Err(ParseError::Binary {
                        msg: e.to_string(),
                        record: (record_idx + 1) as usize,
                    }))
                }
            }
        }
        None
    }
}

fn parse_binary_transaction<R: Read>(reader: &mut R) -> Result<Transaction, String> {
    // date: u16 len + bytes
    let mut len_buf = [0u8; 2];
    reader.read_exact(&mut len_buf).map_err(|_| "date len")?;
    let date_len = u16::from_le_bytes(len_buf) as usize;
    let mut date_bytes = vec![0u8; date_len];
    reader
        .read_exact(&mut date_bytes)
        .map_err(|_| "date bytes")?;
    let date = String::from_utf8(date_bytes).map_err(|_| "date utf8")?;

    // category: u16 len + bytes
    reader
        .read_exact(&mut len_buf)
        .map_err(|_| "category len")?;
    let cat_len = u16::from_le_bytes(len_buf) as usize;
    let mut cat_bytes = vec![0u8; cat_len];
    reader
        .read_exact(&mut cat_bytes)
        .map_err(|_| "category bytes")?;
    let category = String::from_utf8(cat_bytes).map_err(|_| "category utf8")?;

    // kind: u8
    let mut kind_buf = [0u8; 1];
    reader.read_exact(&mut kind_buf).map_err(|_| "kind")?;
    let kind = match kind_buf[0] {
        0 => TransactionKind::Income,
        1 => TransactionKind::Expense,
        _ => return Err("invalid kind".to_string()),
    };

    // amount: i64 LE
    let mut amount_buf = [0u8; 8];
    reader.read_exact(&mut amount_buf).map_err(|_| "amount")?;
    let amount = i64::from_le_bytes(amount_buf);

    Ok(Transaction {
        date,
        category,
        kind,
        amount,
    })
}

fn parse_text_transaction(
    line: &str,
    line_num: usize,
    format: RecordFormat,
) -> Option<Result<Transaction, ParseError>> {
    let fields = match format {
        RecordFormat::Csv => parse_csv_line(line),
        RecordFormat::Tsv => parse_tsv_line(line),
        _ => unreachable!(),
    };

    let fields = match fields {
        Ok(f) => f,
        Err(e) => return Some(Err(ParseError::Format(e.to_string(), line_num))),
    };

    if fields.len() != 4 {
        return Some(Err(ParseError::Format(
            format!("expected 4 fields, got {}", fields.len()),
            line_num,
        )));
    }

    let date = fields[0].clone();
    let category = fields[1].clone();
    let kind_str = fields[2].as_str();
    let kind = match kind_str {
        "income" => TransactionKind::Income,
        "expense" => TransactionKind::Expense,
        _ => {
            return Some(Err(ParseError::Format(
                format!("unknown kind: {}", kind_str),
                line_num,
            )))
        }
    };
    let amount = match fields[3].parse::<i64>() {
        Ok(a) => a,
        Err(_) => {
            return Some(Err(ParseError::Format(
                format!("invalid amount: {}", fields[3]),
                line_num,
            )))
        }
    };

    Some(Ok(Transaction {
        date,
        category,
        kind,
        amount,
    }))
}

// CSV парсер (с кавычками)
fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' => {
                chars.next();
                if in_quotes && chars.peek() == Some(&'"') {
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

// TSV парсер (точка с запятой, без кавычек)
fn parse_tsv_line(line: &str) -> Result<Vec<String>, String> {
    Ok(line.split(';').map(|s| s.trim().to_string()).collect())
}

fn main() -> io::Result<()> {
    // Текстовый CSV
    let csv_file = File::open("assets/transactions.csv")?;
    let mut _csv_parser = Parser::new(csv_file, RecordFormat::Csv);

    // Текстовый TSV
    let tsv_file = File::open("assets/transactions.tsv")?;
    let mut _tsv_parser = Parser::new(tsv_file, RecordFormat::Tsv);

    // Бинарный YPB1
    // let bin_file = File::open("assets/transactions.ypb")?;
    let bin_file = File::open("assets/transactions.bin")?;
    let mut bin_parser = Parser::new(bin_file, RecordFormat::Binary);

    for trans in bin_parser.transactions() {
        match trans {
            Ok(t) => println!("Binary: {:?}", t),
            Err(e) => eprintln!("Binary error: {}", e),
        }
    }
    Ok(())
}
