// Primitives around transactions.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use csv::ReaderBuilder;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

use crate::account::{Account, AccountAdmin, Error as ClientError};
use crate::input::Input;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidRecord,
    Send(SendError<Transaction>),
    Client(ClientError)
}

// Abstraction over transaction types.
#[derive(PartialEq, Debug, Clone)]
pub enum Type {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
    ERR
}

impl From<&str> for Type {
    fn from(transaction_type: &str) -> Self {
        match transaction_type {
            "dispute" => Type::Dispute,
            "resolve" => Type::Resolve,
            "chargeback" => Type::Chargeback,
            "deposit" => Type::Deposit,
            "withdrawal" => Type::Withdrawal,
            _ => Type::ERR
        }
    }
}

// Wrapper over a line from the input file.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    ttype: Type,
    client_id: u16,
    tx_id: u32,
    amount: Option<f64>,
    disputed: bool,
    resolved: bool,
    charged_back: bool,
}

impl Transaction {
    pub fn new_with_amount(ttype: Type, client_id: u16, tx_id: u32,
                           amount: f64) -> Self {
        Transaction { ttype, client_id, tx_id, amount: Some(amount), disputed: false,
            resolved: false, charged_back: false }
    }

    pub fn new(ttype: Type, client_id: u16, tx_id: u32) -> Self {
        Transaction { ttype, client_id, tx_id, amount: None, disputed: false, resolved: false,
            charged_back: false }
    }

    pub fn mark_disputed(&mut self) {
        self.disputed = true;
        self.resolved = false;
        self.charged_back = false;
    }

    pub fn mark_resolved(&mut self) {
        self.resolved = true;
        self.disputed = false;
        self.charged_back = false;
    }

    pub fn mark_charged_back(&mut self) {
        self.charged_back = true;
        self.disputed = false;
        self.resolved = false;
    }

    // A flag is considered one of the `disputed`, `resolved` or `charged_back` states.
    pub fn is_emtpy_flags(&self) -> bool {
        return !self.disputed && !self.resolved && !self.charged_back
    }

    pub fn is_disputed(&self) -> bool {
        self.disputed
    }

    pub fn tx_id(&self) -> u32 {
        self.tx_id
    }

    pub fn transaction_type(&self) -> Type {
        self.ttype.clone()
    }

    pub fn amount(&self) -> Option<f64> {
        self.amount
    }

    pub fn client_id(&self) -> u16 {
        self.client_id
    }

    // CSV records String to Transaction convertor. We avoid implementing the From<String> trait
    // because we want to propagate parsing errors.
    pub fn from(line: String) -> Result<Transaction> {
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());

        // We can not use serde deserialization because of
        // https://github.com/BurntSushi/rust-csv/issues/113.
        for result in rdr.records() {
            return match result {
                Ok(str_record) => {
                    if str_record.len() < 3 {
                        return Err(Error::InvalidRecord);
                    }

                    // We know for sure that the record has at least three elements.
                    let ttype = Type::from(str_record.get(0).unwrap());
                    if ttype == Type::ERR {
                        return Err(Error::InvalidRecord);
                    }

                    let client_id = str_record.get(1)
                        .unwrap().parse::<u16>()
                        .map_err(|_| Error::InvalidRecord)?;

                    let tx_id = str_record.get(2)
                        .unwrap().parse::<u32>()
                        .map_err(|_| Error::InvalidRecord)?;

                    if str_record.len() == 4 {
                        let amount = str_record.get(3)
                            .unwrap()
                            .parse::<f64>().map_err(|_| Error::InvalidRecord)?;
                        return Ok(Transaction::new_with_amount(ttype,
                                                               client_id,
                                                               tx_id,
                                                               amount));
                    }

                    Ok(Transaction::new(ttype, client_id, tx_id))
                }
                Err(_) => Err(Error::InvalidRecord)
            };
        }

        Err(Error::InvalidRecord)
    }

    #[cfg(test)]
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    #[cfg(test)]
    pub fn is_charged_back(&self) -> bool {
        self.charged_back
    }

    #[cfg(test)]
    pub fn clear_flags(&mut self) {
        self.disputed = false;
        self.resolved = false;
        self.charged_back = false;
    }
}

pub struct TransactionIterator {
    input: Input
}

impl TransactionIterator {
    pub fn new(input: Input) -> Self {
        TransactionIterator { input }
    }
}

impl Iterator for TransactionIterator {
    type Item = Transaction;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(res) = self.input.next().map(Transaction::from) {
            return match res {
                Ok(tx) => Some(tx),
                Err(_) => None
            };
        }
        None
    }
}

// Entry point into transactions execution, iterating through each tx from the provided input.
pub fn drill(input: Input, multi_threaded_runtime: bool, tx_delay: Option<Duration>, dump_accounts: bool) {
    let record_iter = TransactionIterator::new(input);
    let rt = if multi_threaded_runtime {
        tokio::runtime::Builder::new_multi_thread().build().expect("Could not initialize multi threaded runtime.")
    } else {
        tokio::runtime::Builder::new_current_thread().build().expect("Could not initialize single threaded runtime.")
    };

    rt.block_on(async move {
        let mut pipes = HashMap::new();
        let mut worker_handlers: Vec<JoinHandle<Option<Account>>> = Vec::new();
        for tx in record_iter {
            let client_id = tx.client_id();
            // If the sender for a specific client was already created, send the tx on the channel.
            if pipes.contains_key(&client_id) {
                let sender: &Sender<Transaction> = pipes.get_mut(&client_id).unwrap();
                // Handle errors gracefully. When an account is locked the receiver is closed.
                // However, we still need to keep the sender in scope because otherwise we wouldn't
                // know that there were already an account for the client with the account locked,
                // which means that we will create a new account for that client, which is not the
                // expected behavior of handling transactions.
                match sender.send(tx).await {
                    Ok(_) => (),
                    Err(_) => ()
                };
            } else { // Otherwise, create the channel and spawn a task with the client waiting for
                // transactions to handle. The client will stop waiting for transactions when the
                // the channel is closed.
                let (sender, receiver) = tokio::sync::mpsc::channel(32);
                sender.send(tx).await.unwrap();
                let _ = pipes.insert(client_id, sender);
                // Store the tasks handle.
                worker_handlers.push(tokio::spawn(async move {
                    let mut account_admin = AccountAdmin::new(client_id, receiver);
                    loop {
                        if tx_delay.is_some() {
                            thread::sleep(tx_delay.unwrap());
                        }

                        let account = match account_admin.handle().await {
                            Ok(_) => None,
                            Err(ClientError::Handle(acc)) => Some(acc),
                            Err(_) => None,
                        };

                        if account.is_some() {
                            return account;
                        }
                    }
                }));
            }
        }

        // Close the senders and implicitly, stop the clients from waiting for transactions.
        for _ in pipes {
        }

        if dump_accounts {
            // Print the accounts contents.
            println!("client,available,held,total,locked");
        }

        for handle in worker_handlers {
            let res = handle.await.unwrap();
            match res {
                Some(account) => if dump_accounts {
                    println!("{},{:.4},{:.4},{:.4},{}", account.client_id(), account.available(), account.held(), account.total(), account.is_locked());
                }
                None => unreachable!()
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use std::io::{Seek, SeekFrom, Write};
    use crate::transaction::{Transaction, TransactionIterator, Type};
    use crate::input::Input;
    use tempfile::tempfile;

    #[test]
    fn test_type_from_str() {
        assert_eq!(Type::Deposit, Type::from("deposit"));
        assert_eq!(Type::Withdrawal, Type::from("withdrawal"));
        assert_eq!(Type::Dispute, Type::from("dispute"));
        assert_eq!(Type::Resolve, Type::from("resolve"));
        assert_eq!(Type::Chargeback, Type::from("chargeback"));
    }

    #[test]
    fn test_tx_new_with_amount() {
        let tx = Transaction::new_with_amount(Type::Withdrawal, 1, 2, 2.0);
        assert_eq!(tx.ttype, Type::Withdrawal);
        assert_eq!(tx.client_id, 1);
        assert_eq!(tx.tx_id, 2);
        assert_eq!(tx.amount, Some(2.0));
    }

    #[test]
    fn test_tx_new() {
        let tx = Transaction::new(Type::Withdrawal, 1, 2);
        assert_eq!(tx.ttype, Type::Withdrawal);
        assert_eq!(tx.client_id, 1);
        assert_eq!(tx.tx_id, 2);
        assert_eq!(tx.amount,None);
    }

    #[test]
    fn test_tx_disputed() {
        let mut tx = Transaction::new(Type::Deposit, 1, 1);
        assert_eq!(tx.is_emtpy_flags(), true);
        tx.mark_disputed();
        assert_eq!(tx.is_emtpy_flags(), false);
        assert_eq!(tx.is_disputed(), true);
        assert_eq!(tx.is_resolved(), false);
        assert_eq!(tx.is_charged_back(), false);
    }

    #[test]
    fn test_tx_resolved() {
        let mut tx = Transaction::new(Type::Deposit, 1, 1);
        assert_eq!(tx.is_emtpy_flags(), true);
        tx.mark_resolved();
        assert_eq!(tx.is_emtpy_flags(), false);
        assert_eq!(tx.is_disputed(), false);
        assert_eq!(tx.is_resolved(), true);
        assert_eq!(tx.is_charged_back(), false);
    }

    #[test]
    fn test_tx_charged_back() {
        let mut tx = Transaction::new(Type::Deposit, 1, 1);
        assert_eq!(tx.is_emtpy_flags(), true);
        tx.mark_charged_back();
        assert_eq!(tx.is_emtpy_flags(), false);
        assert_eq!(tx.is_disputed(), false);
        assert_eq!(tx.is_resolved(), false);
        assert_eq!(tx.is_charged_back(), true);
    }

    #[test]
    fn test_tx_getters() {
        let tx = Transaction::new(Type::Deposit, 10, 2);
        assert_eq!(tx.tx_id(), 2);
        assert_eq!(tx.client_id(), 10);
        assert_eq!(tx.amount(), None);
        assert_eq!(tx.transaction_type(), Type::Deposit);
    }

    #[test]
    fn test_tx_from_str() {
        assert_eq!(Transaction::from(String::from("deposit,1,1,1.0")).unwrap(),
                   Transaction::new_with_amount(Type::Deposit, 1, 1, 1.0));
        assert_eq!(Transaction::from(String::from("resolve,1,1")).unwrap(),
                   Transaction::new(Type::Resolve, 1, 1));
        assert!(Transaction::from(String::from("")).is_err());
        assert!(Transaction::from(String::from("Dispute,1,1,1.0")).is_err());
        assert!(Transaction::from(String::from("1,1,1.0")).is_err());
        assert!(Transaction::from(String::from("dispute,1.0,1,1.0")).is_err());
    }

    #[test]
    fn test_tx_iterator() {
        let mut tmp_file = tempfile().unwrap();
        writeln!(tmp_file, "type,client,tx,amount").unwrap();
        writeln!(tmp_file, "deposit,0,0,1.0").unwrap();
        writeln!(tmp_file, "dispute,0,0").unwrap();
        writeln!(tmp_file, "resolve,0,0").unwrap();
        writeln!(tmp_file, "error,0,0").unwrap();
        tmp_file.seek(SeekFrom::Start(0)).unwrap();

        let mut tx_iter = TransactionIterator::new(Input::from(tmp_file));
        assert_eq!(Transaction::new_with_amount(Type::Deposit, 0, 0, 1.0), tx_iter.next().unwrap());
        assert_eq!(Transaction::new(Type::Dispute, 0, 0), tx_iter.next().unwrap());
        assert_eq!(Transaction::new(Type::Resolve, 0, 0), tx_iter.next().unwrap());

        // Errors are handled gracefully.
        assert!(tx_iter.next().is_none());
        assert!(tx_iter.next().is_none());
    }
}
