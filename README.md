## Архитектура приложения: единый потоковый парсер с расширяемой поддержкой форматов

Приложение реализовано как универсальный потоковый парсер транзакций с **абстрактным интерфейсом**, позволяющим добавлять новые форматы без изменения consumer-кода.

### **Единый API для всех форматов**
```rust
let mut parser = Parser::new(file, RecordFormat::Csv);  // или Tsv, Binary
for trans in parser.transactions() {
    match trans {
        Ok(t) => process(t),  // Один код для всех!
        Err(e) => log(e),
    }
}
```
**Consumer не знает о формате** — всегда получает `Iterator<Result<Transaction, Error>>`.

### **Логика работы (Core Flow)**

1. **Инициализация**: `Parser::new(reader, format)`
   - Создаёт `BufReader<R>`
   - Сохраняет `format: RecordFormat` для диспетчеризации

2. **Запуск итератора**: `parser.transactions() → ParserTransactions`
   ```rust
   impl Iterator for ParserTransactions {
       fn next(&mut self) → Option<Result<Transaction, ParseError>> {
           match self.parser.format {
               Csv | Tsv  → self.next_text(),    // Текстовые ветки
               Binary    → self.next_binary(),   // Бинарная ветка
           }
       }
   }
   ```

3. **Диспетчеризация по формату** (расширяемость!):

![](docs/flow.png)

4. **Парсинг → Transaction** (единая точка):
   ```rust
   fn parse_*_line/record(line/bytes) → Result<Transaction, Error>
   ```
   Все форматы **гарантированно возвращают** `Transaction {date, category, kind, amount}`.

### **Расширяемость: добавление нового формата за 10 строк**

```rust
#[derive(Debug, Copy, Clone)]
enum RecordFormat {
    Csv, Tsv, Binary, JsonLines, // ← Новый!
}

impl<'a, R: Read> ParserTransactions<'a, R> {
    fn next_json(&mut self) → Option<Result<Transaction, ParseError>> {
        // serde_json::from_str(line) → Transaction
        // или ручной парсер JSONL
    }
    
    fn next(&mut self) → Option<Result<Transaction, Error>> {
        match self.parser.format {
            RecordFormat::JsonLines => self.next_json(),
            // ... остальные
        }
    }
}
```

**Преимущества**:
- **Consumer неизменён** — `for trans in parser.transactions()`
- **Добавление формата** = 1 enum вариант + 1 метод `next_*()`
- **Ошибки унифицированы** — `ParseError` для всех
- **Потоковость сохранена** — O(1) память для любого формата

### **Внутренняя архитектура Parser**
```rust
pub struct Parser<R: Read> {
    reader:   BufReader<R>,     // Универсальный источник
    format:   RecordFormat,     // Диспетчер
    buffer:   String,           // Для текста
    line_num: usize,            // Для ошибок
}
```

### **Почему это масштабируемо**
1. **Декомпозиция**: `next() → next_format() → parse_format() → Transaction`
2. **Плейсхолдеры**: Легко добавить `next_protobuf()`, `next_avro()`
3. **Типобезопасность**: Компилятор гарантирует `Result<Transaction>`
4. **Производительность**: Нативный парсинг, потоковый, без аллоков кроме результата

**Итог**: **Архитектура "Strategy Pattern" через enum**: один интерфейс, бесконечные форматы, нулевые изменения в бизнес-логике! 
