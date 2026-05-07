use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Income,
    Expense,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Дата операции в формате `YYYY-MM-DD`.
    pub date: String,
    /// Категория операции.
    pub category: String,
    /// Тип операции: доход или расход.
    pub kind: TransactionKind,
    /// Сумма операции.
    pub amount: i64,
}

#[derive(Debug)]
pub struct CsvError {
    msg: String,
    line: usize,
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSV error at line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for CsvError {}

pub struct CsvReader<R: Read> {
    reader: BufReader<R>,
    buffer: String,
    line_num: usize,
}

impl<R: Read> CsvReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
            line_num: 0,
        }
    }

    pub fn transactions(&mut self) -> CsvTransactions<'_, R> {
        CsvTransactions { reader: self }
    }
}

pub struct CsvTransactions<'a, R: Read> {
    reader: &'a mut CsvReader<R>,
}

impl<'a, R: Read> Iterator for CsvTransactions<'a, R> {
    type Item = Result<Transaction, CsvError>;

    fn next(&mut self) -> Option<Self::Item> {
        let line_num = self.reader.line_num + 1;
        let line = match self.reader.reader.fill_buf().ok()? {
            buf if buf.is_empty() => return None,
            buf => {
                let len = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
                self.reader.buffer.clear();
                self.reader
                    .buffer
                    .extend(buf[..len].iter().map(|&b| b as char));
                self.reader.reader.consume(len + 1); // +1 для \n
                self.reader.line_num = line_num;
                &self.reader.buffer[..]
            }
        };

        parse_transaction_line(line, line_num)
    }
}

// impl<'a, R: Read> Iterator for CsvTransactions<'a, R> {
//     type Item = Result<Transaction, CsvError>;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         let line_num = self.reader.line_num + 1;
//         let line = match self.reader.reader.fill_buf().ok()? {
//             buf if buf.is_empty() => return None,
//             buf => {
//                 let end = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
//                 self.reader.buffer.clear();
//                 self.reader
//                     .buffer
//                     .extend(buf[..end].iter().map(|&b| b as char));
//                 self.reader.reader.consume(end);
//                 if end < buf.len() && buf[end] == b'\n' {
//                     self.reader.reader.consume(1);
//                 }
//                 self.reader.line_num = line_num;
//                 &self.reader.buffer[..]
//             }
//         };
//
//         parse_transaction_line(line, line_num)
//     }
// }

fn parse_transaction_line(line: &str, line_num: usize) -> Option<Result<Transaction, CsvError>> {
    let fields = match parse_csv_line(line) {
        Ok(f) => f,
        Err(e) => {
            return Some(Err(CsvError {
                msg: e.to_string(),
                line: line_num,
            }))
        }
    };

    if fields.len() != 4 {
        return Some(Err(CsvError {
            msg: format!("expected 4 fields, got {}", fields.len()),
            line: line_num,
        }));
    }

    let date = fields[0].clone();
    let category = fields[1].clone();
    let kind_str = &fields[2];
    let kind = match kind_str.as_str() {
        "income" => TransactionKind::Income,
        "expense" => TransactionKind::Expense,
        _ => {
            return Some(Err(CsvError {
                msg: format!("unknown kind: {}", kind_str),
                line: line_num,
            }))
        }
    };
    let amount = match fields[3].parse::<i64>() {
        Ok(a) => a,
        Err(_) => {
            return Some(Err(CsvError {
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

fn main() -> io::Result<()> {
    let file = File::open("assets/transactions.csv")?;
    let mut csv = CsvReader::new(file);

    let mut total_income = 0i64;
    let mut total_expense = 0i64;

    for trans in csv.transactions() {
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

    println!("Total income: {}, expense: {}", total_income, total_expense);
    Ok(())
}
