// Offers primitives for parsing the transaction processor input.

use std::fs::File;
use std::io::{BufRead, BufReader};

// A file wrapper that provides primitives for iterating through a specifc CSV file line by line.
// This also takes into account the header line.
pub struct Input {
    reader: BufReader<File>,
}

impl From<File> for Input {
    fn from(file: File) -> Self {
        let mut buf_reader = BufReader::new(file);
        let mut line = String::new();
        let bytes_read = buf_reader.read_line(&mut line);
        match bytes_read {
            Ok(_) => if line != "type,client,tx,amount\n" {
                panic!("The CSV file format is not as expected.\n\
                Please stick to the following header line `type,client,tx,amount`.\n\
                If still in doubt, consult the documentation.");
            }
            Err(_) => panic!("Error while reading the header line of the CSV file.\n\
            It is mandatory that the CSV file to began with the header line.")
        };

        Input { reader: buf_reader }
    }
}

// We want to take on the input line by line.
impl Iterator for Input {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line: String = String::new();
        let line_wrapper = self.reader.read_line(&mut line);
        match line_wrapper {
            Ok(bytes_read) if bytes_read > 0 => Some(line),
            Ok(_) | Err(_) => None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};
    use tempfile::tempfile;
    use crate::input::Input;

    #[test]
    #[should_panic]
    fn test_input_from_file_invalid_header_line() {
        let mut tmp_file = tempfile().unwrap();
        writeln!(tmp_file, "1,2,3,4").unwrap();
        tmp_file.seek(SeekFrom::Start(0)).unwrap();
        let _ = Input::from(tmp_file);
    }

    #[test]
    fn test_input_from_file_valid_header_line() {
        let mut tmp_file = tempfile().unwrap();
        writeln!(tmp_file, "type,client,tx,amount").unwrap();
        tmp_file.seek(SeekFrom::Start(0)).unwrap();
        let _ = Input::from(tmp_file);
    }

    #[test]
    fn test_input_from_file_iterator() {
        let mut tmp_file = tempfile().unwrap();
        writeln!(tmp_file, "type,client,tx,amount").unwrap();
        writeln!(tmp_file, "deposit,0,0,1.0").unwrap();
        writeln!(tmp_file, "withdrawal,0,1,0.5").unwrap();
        tmp_file.seek(SeekFrom::Start(0)).unwrap();
        let mut input = Input::from(tmp_file);
        assert_eq!(input.next().unwrap(), "deposit,0,0,1.0\n");
        assert_eq!(input.next().unwrap(), "withdrawal,0,1,0.5\n");
        assert!(input.next().is_none());
    }
}