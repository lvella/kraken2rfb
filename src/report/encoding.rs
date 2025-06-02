use chrono::Datelike;
use chrono::NaiveDate;
use itertools::Itertools;
use rust_decimal::Decimal;
use std::fmt;
use std::io::{self, Write};

pub enum Field<'a> {
    Date(NaiveDate),
    DecimalNumber { value: &'a Decimal, precision: u32 },
    AlphaNumber { value: &'a str },
    Empty,
}

impl<'a> From<&'a str> for Field<'a> {
    fn from(value: &'a str) -> Self {
        if value.contains('|') {
            panic!("Field value cannot contain '|' character");
        }
        Field::AlphaNumber { value }
    }
}

impl<'a> From<&'a String> for Field<'a> {
    fn from(value: &'a String) -> Self {
        Field::from(value.as_str())
    }
}

impl<'a> From<NaiveDate> for Field<'a> {
    fn from(value: NaiveDate) -> Self {
        Field::Date(value)
    }
}

impl<'a> fmt::Display for Field<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Field::Date(date) => {
                // Format as ddmmaaaa without separators
                write!(f, "{:02}{:02}{:04}", date.day(), date.month(), date.year())
            }
            Field::DecimalNumber { value, precision } => {
                // Format with specified precision, using comma as decimal separator
                // and no thousand separators
                let rounded = value.round_dp(*precision);
                let formatted = format!("{:.1$}", rounded, *precision as usize).replace('.', ",");
                write!(f, "{}", formatted)
            }
            Field::AlphaNumber { value } => {
                // Display alphanumeric value as is
                write!(f, "{}", value)
            }
            Field::Empty => {
                // Empty fields are represented by an empty string
                write!(f, "")
            }
        }
    }
}

/// Writes a register row to the given writer, joining fields with pipe delimiters and adding CRLF.
///
/// # Arguments
///
/// * `writer` - The writer to write the row to
/// * `fields` - A slice of fields to write in the row
///
/// # Returns
///
/// * `io::Result<()>` - Result indicating success or failure of the write operation
///
/// # Example
///
/// ```
/// use std::io::Cursor;
/// let mut writer = Cursor::new(Vec::new());
/// let fields = vec![
///     Field::AlphaNumber { value: "I550" },
///     Field::AlphaNumber { value: "José Silva" },
///     Field::AlphaNumber { value: "12345678912" },
/// ];
/// write_register_row(&mut writer, &fields).unwrap();
/// assert_eq!(writer.into_inner(), b"I550|José Silva|12345678912|\r\n");
/// ```
pub fn write_register_row<W: Write>(writer: &mut W, fields: &[Field<'_>]) -> io::Result<()> {
    // Join all fields with pipe delimiter and add final pipe and CRLF
    write!(writer, "{}\r\n", fields.iter().format("|"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::io::Cursor;

    #[test]
    fn test_field_formatting() {
        // Test date formatting
        let date = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();
        let date_field = Field::Date(date);
        let formatted_date = date_field.to_string();
        println!("Date formatted: {}", formatted_date);
        assert_eq!(formatted_date, "31122023");

        // Test decimal numbers with different precisions
        let test_cases = vec![
            (dec!(1234.5678), 2, "1234,57"),
            (dec!(1234.5678), 3, "1234,568"),
            (dec!(1234.5678), 0, "1235"),
            (dec!(0.01), 2, "0,01"),
            (dec!(10000), 2, "10000,00"),
        ];

        for (value, precision, expected) in test_cases {
            let decimal_field = Field::DecimalNumber {
                value: &value,
                precision,
            };
            let formatted = decimal_field.to_string();
            println!(
                "Decimal {} with precision {} formatted: {}",
                value, precision, formatted
            );
            assert_eq!(formatted, expected);
        }

        // Test alphanumeric values
        let alpha_field = Field::AlphaNumber { value: "TEST123" };
        let formatted_alpha = alpha_field.to_string();
        println!("Alphanumeric formatted: {}", formatted_alpha);
        assert_eq!(formatted_alpha, "TEST123");

        // Test From<&str> implementation
        let string_field: Field = "VALID_STRING".into();
        let formatted_string = string_field.to_string();
        println!("String converted and formatted: {}", formatted_string);
        assert_eq!(formatted_string, "VALID_STRING");
    }

    #[test]
    fn test_write_register_row() {
        let mut writer = Cursor::new(Vec::new());
        let fields = vec![
            Field::AlphaNumber { value: "I550" },
            Field::AlphaNumber {
                value: "José Silva",
            },
            Field::AlphaNumber {
                value: "12345678912",
            },
            Field::AlphaNumber {
                value: "01238578455",
            },
        ];
        write_register_row(&mut writer, &fields).unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "I550|José Silva|12345678912|01238578455\r\n"
        );

        // Test with empty fields using the Empty variant
        let mut writer = Cursor::new(Vec::new());
        let fields = vec![
            Field::AlphaNumber { value: "I550" },
            Field::AlphaNumber {
                value: "João Silva",
            },
            Field::Empty,
            Field::AlphaNumber {
                value: "96325874177",
            },
        ];
        write_register_row(&mut writer, &fields).unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "I550|João Silva||96325874177\r\n"
        );

        // Test with multiple empty fields (last field is empty)
        let mut writer = Cursor::new(Vec::new());
        let fields = vec![
            Field::AlphaNumber { value: "I550" },
            Field::Empty,
            Field::Empty,
            Field::Empty,
        ];
        write_register_row(&mut writer, &fields).unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "I550|||\r\n"
        );

        // Test with decimal numbers
        let mut writer = Cursor::new(Vec::new());
        let value1 = dec!(1234.56);
        let value2 = dec!(0.01);
        let fields = vec![
            Field::AlphaNumber { value: "I550" },
            Field::DecimalNumber {
                value: &value1,
                precision: 2,
            },
            Field::DecimalNumber {
                value: &value2,
                precision: 2,
            },
        ];
        write_register_row(&mut writer, &fields).unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "I550|1234,56|0,01\r\n"
        );

        // Test with dates
        let mut writer = Cursor::new(Vec::new());
        let date = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();
        let fields = vec![Field::AlphaNumber { value: "I550" }, Field::Date(date)];
        write_register_row(&mut writer, &fields).unwrap();
        assert_eq!(
            String::from_utf8(writer.into_inner()).unwrap(),
            "I550|31122023\r\n"
        );
    }
}
