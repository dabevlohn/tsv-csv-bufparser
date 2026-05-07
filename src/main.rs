use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Income,
    Expense,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordFormat {
    Csv, // Запятая, кавычки
    Tsv, // Точка с запятой без кавычек
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub date: String,
    pub category: String,
    pub kind: TransactionKind,
    pub amount: i64,
}

#[derive(Debug)]
pub struct ParseError {
    msg: String,
    line: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

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
                &self.parser.buffer[..]
            }
        };

        parse_transaction_line(line, line_num, self.parser.format.clone())
    }
}

fn parse_transaction_line(
    line: &str,
    line_num: usize,
    format: RecordFormat,
) -> Option<Result<Transaction, ParseError>> {
    let fields = match format {
        RecordFormat::Csv => parse_csv_line(line),
        RecordFormat::Tsv => parse_tsv_line(line),
    };

    let fields = match fields {
        Ok(f) => f,
        Err(e) => {
            return Some(Err(ParseError {
                msg: e.to_string(),
                line: line_num,
            }))
        }
    };

    if fields.len() != 4 {
        return Some(Err(ParseError {
            msg: format!("expected 4 fields, got {}", fields.len()),
            line: line_num,
        }));
    }

    let date = fields[0].clone();
    let category = fields[1].clone();
    let kind_str = fields[2].as_str();
    let kind = match kind_str {
        "income" => TransactionKind::Income,
        "expense" => TransactionKind::Expense,
        _ => {
            return Some(Err(ParseError {
                msg: format!("unknown kind: {}", kind_str),
                line: line_num,
            }))
        }
    };
    let amount = match fields[3].parse::<i64>() {
        Ok(a) => a,
        Err(_) => {
            return Some(Err(ParseError {
                msg: format!("invalid amount: {}", fields[3]),
                line: line_num,
            }))
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
    let tsv_file = File::open("assets/transactions.tsv")?;
    let mut tsv_parser = Parser::new(tsv_file, RecordFormat::Tsv);
    let mut total_income = 0i64;
    let mut total_expense = 0i64;

    for trans in tsv_parser.transactions() {
        match trans {
            Ok(t) => {
                match t.kind {
                    TransactionKind::Income => total_income += t.amount,
                    TransactionKind::Expense => total_expense += t.amount,
                }
                println!("{:?}", t);
            }
            Err(e) => eprintln!("{}", e),
        }
    }

    println!("TSV: Income {}, Expense {}", total_income, total_expense);

    Ok(())
}
