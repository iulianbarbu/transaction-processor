use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;
use crate::transaction::{Transaction, Type};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    FundsExceeded,
    DepositTooLow,
    OperationNotSupported,
    TxNotFound,
    AccountLocked,
    TxNotDisputed,
    TxAlreadyDisputed,
    Handle(Account)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    client_id: u16,
    available: f64,
    held: f64,
    locked: bool
}

impl Account {
    pub fn new(client_id: u16, available: f64, held: f64, locked: bool) -> Self {
        Account {client_id, available, held, locked}
    }

    pub fn new_unlocked(client_id: u16, available: f64, held: f64) -> Self {
        Account {client_id, available, held, locked: false}
    }

    pub fn client_id(&self) -> u16 {
        self.client_id
    }

    pub fn available(&self) -> f64 {
        self.available
    }

    pub fn held(&self) -> f64 {
        self.held
    }

    pub fn total(&self) -> f64 {
        self.available + self.held
    }

    pub fn add_available(&mut self, amount: f64) -> Result<()> {
        self.available += amount;
        Ok(())
    }

    pub fn sub_available(&mut self, amount: f64) -> Result<()> {
        if self.available < amount {
            return Err(Error::DepositTooLow)
        }

        self.available -= amount;
        Ok(())
    }

    pub fn add_held(&mut self, amount: f64) -> Result<()> {
        self.held += amount;
        Ok(())
    }

    pub fn sub_held(&mut self, amount: f64) -> Result<()> {
        if self.held < amount {
            return Err(Error::DepositTooLow)
        }

        self.held -= amount;
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }
}

pub struct AccountAdmin {
    account: Account,
    tx_history: HashMap<u32, Transaction>,
    receiver: Receiver<Transaction>
}

impl AccountAdmin {
    pub fn new(id: u16, receiver: Receiver<Transaction>) -> AccountAdmin {
        AccountAdmin {
            account: Account::new_unlocked(id, 0.0, 0.0),
            tx_history: HashMap::new(),
            receiver
        }
    }

    pub fn id(&self) -> u16 { self.account.client_id() }

    pub fn account(&self) -> &Account {
        &self.account
    }

    pub async fn  handle(&mut self) -> Result<&Account> {
        match self.receiver.recv().await {
            Some(tx) => {
                let tx_type = tx.transaction_type().clone();
                let tx_id = tx.tx_id();
                match tx_type {
                    Type::Deposit => {
                        if self.account.is_locked() {
                            return Err(Error::Handle(self.account().clone()));
                        }

                        // Safe to unwrap, since we are handling a deposit tx.
                        let amount = tx.amount().unwrap();
                        self.tx_history.insert(tx_id, tx);
                        self.account.add_available(amount)?;
                        Ok(self.account())
                    },
                    Type::Withdrawal => {
                        if self.account.is_locked() {
                            return Err(Error::Handle(self.account().clone()));
                        }

                        // Safe to unwrap, since we are handling a withdrawal tx.
                        let amount = tx.amount().unwrap();
                        self.tx_history.insert(tx_id, tx);
                        self.account.sub_available(amount)?;
                        Ok(self.account())
                    },
                    Type::Dispute => {
                        match self.tx_history.get_mut(&tx_id) {
                            None => Err(Error::TxNotFound),
                            Some(to_be_disputed_tx) => {
                                if !to_be_disputed_tx.is_emtpy_flags() {
                                    return Err(Error::TxAlreadyDisputed);
                                }

                                if self.account.is_locked() {
                                    return Err(Error::Handle(self.account().clone()));
                                }

                                let amount = to_be_disputed_tx.amount();
                                self.account.sub_available(amount.unwrap())?;
                                to_be_disputed_tx.mark_disputed();
                                self.account.add_held(amount.unwrap())?;
                                Ok(self.account())
                            }
                        }
                    },
                    Type::Resolve => {
                        match self.tx_history.get_mut(&tx_id) {
                            None => Err(Error::TxNotFound),
                            Some(disputed_tx) => {
                                if disputed_tx.is_emtpy_flags() {
                                    return Err(Error::TxNotDisputed);
                                }

                                if self.account.is_locked() {
                                    return Err(Error::Handle(self.account().clone()));
                                }

                                let amount = disputed_tx.amount();
                                self.account.sub_held(amount.unwrap())?;
                                disputed_tx.mark_resolved();
                                self.account.add_available(amount.unwrap())?;
                                Ok(self.account())
                            }
                        }
                    },
                    Type::Chargeback => {
                        match self.tx_history.get_mut(&tx_id) {
                            None => Err(Error::TxNotFound),
                            Some(disputed_tx) => {
                                if disputed_tx.is_emtpy_flags() {
                                    return Err(Error::TxNotDisputed);
                                }

                                if self.account.is_locked() {
                                    return Err(Error::Handle(self.account().clone()));
                                }

                                let amount = disputed_tx.amount();
                                self.account.sub_held(amount.unwrap())?;
                                self.account.set_locked(true);
                                disputed_tx.mark_charged_back();
                                Ok(self.account())
                            }
                        }
                    }
                    _ => Err(Error::OperationNotSupported)
                }
            }
            None => {
                Err(Error::Handle(self.account().clone()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_new_unlocked() {
        let account = Account::new_unlocked(0,0.0, 0.0);
        assert_eq!(account.is_locked(), false);
    }

    #[test]
    fn test_account_new() {
        let account = Account::new(0,1.0, 2.0, true);
        assert_eq!(account.available, 1.0);
        assert_eq!(account.held, 2.0);
        assert_eq!(account.locked, true);
    }

    #[test]
    fn test_account_getters() {
        let account = Account::new_unlocked(0,1.5, 2.0);
        assert_eq!(account.available(), 1.5);
        assert_eq!(account.held(), 2.0);
        assert_eq!(account.total(), 3.5);
    }

    #[test]
    fn test_account_setters() {
        let mut account = Account::new(0,1.0, 2.0, true);
        account.set_locked(false);
        assert_eq!(account.is_locked(), false);
    }

    #[test]
    fn test_account_add_available() {
        let mut account = Account::new(0,1.0, 2.0, false);
        assert!(account.add_available(1.1).is_ok());
        assert_eq!(account.available(), 2.1);
    }

    #[test]
    fn test_account_sub_available() {
        let mut account = Account::new(0,1.0, 2.0, false);
        assert!(account.sub_available(1.1).is_err());
        assert!(account.sub_available(0.5).is_ok());
        assert_eq!(account.available(), 0.5);
    }

    #[test]
    fn test_account_add_held() {
        let mut account = Account::new(0,1.0, 2.0, false);
        assert!(account.add_held(1.1).is_ok());
        assert_eq!(account.held(), 3.1);
    }

    #[test]
    fn test_account_sub_held() {
        let mut account = Account::new(0,1.0, 2.0, false);
        assert!(account.sub_held(2.1).is_err());
        assert!(account.sub_held(0.5).is_ok());
        assert_eq!(account.held(), 1.5);
    }

    #[test]
    fn test_client_new() {
        let (_, receiver) = tokio::sync::mpsc::channel(32);
        let client = AccountAdmin::new(1, receiver);
        assert_eq!(client.account.client_id, 1);
        assert_eq!(client.account, Account::new(1,0.0, 0.0, false));
        assert!(client.tx_history.is_empty());
    }

    #[test]
    fn test_client_id() {
        let (_, receiver) = tokio::sync::mpsc::channel(32);
        let client = AccountAdmin::new(2, receiver);
        assert_eq!(client.id(), client.account.client_id);
    }

    #[test]
    fn test_client_account() {
        let (_, receiver) = tokio::sync::mpsc::channel(32);
        let client = AccountAdmin::new(2, receiver);
        assert_eq!(client.account().clone(), client.account);
    }

    #[test]
    fn test_client_handle_deposit() {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = AccountAdmin::new(1, receiver);
            sender.send(Transaction::new_with_amount(Type::Deposit, 1, 0, 1.0)).await.unwrap();
            client.handle().await.unwrap();
            assert_eq!(client.account().available(), 1.0);
            assert_eq!(client.account().held(), 0.0);
            assert_eq!(client.account().is_locked(), false);
            assert!(client.tx_history.contains_key(&0));
        });
    }

    #[test]
    fn test_client_handle_withdrawal() {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = AccountAdmin::new(1, receiver);
            client.account.available = 2.0;
            sender.send(Transaction::new_with_amount(Type::Withdrawal, 1, 0, 1.0)).await.unwrap();
            client.handle().await.unwrap();
            assert_eq!(client.account().available(), 1.0);
            assert_eq!(client.account().held(), 0.0);
            assert_eq!(client.account().is_locked(), false);
            assert!(client.tx_history.contains_key(&0));
        });
    }

    #[test]
    fn test_client_handle_dispute() {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = AccountAdmin::new(1, receiver);
            client.account.available = 2.0;
            client.tx_history.insert(0, Transaction::new_with_amount(Type::Deposit, 1, 0, 2.0));
            sender.send(Transaction::new(Type::Dispute, 1, 0)).await.unwrap();
            client.handle().await.unwrap();
            assert_eq!(client.account().available(), 0.0);
            assert_eq!(client.account().held(), 2.0);
            assert_eq!(client.account().is_locked(), false);
            assert!(client.tx_history.get(&0).unwrap().is_disputed());
            assert!(!client.tx_history.get(&0).unwrap().is_resolved());
            assert!(!client.tx_history.get(&0).unwrap().is_charged_back());
            sender.send(Transaction::new(Type::Dispute, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
            client.tx_history.get_mut(&0).unwrap().clear_flags();
            client.account.set_locked(true);
            sender.send(Transaction::new(Type::Dispute, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
        });
    }

    #[test]
    fn test_client_handle_resolve() {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = AccountAdmin::new(1, receiver);
            client.account.held = 2.0;
            client.tx_history.insert(0, Transaction::new_with_amount(Type::Deposit, 1, 0, 2.0));
            client.tx_history.get_mut(&0).unwrap().mark_disputed();
            sender.send(Transaction::new(Type::Resolve, 1, 0)).await.unwrap();
            client.handle().await.unwrap();
            assert_eq!(client.account().held(), 0.0);
            assert_eq!(client.account().available(), 2.0);
            assert_eq!(client.account().is_locked(), false);
            assert!(!client.tx_history.get(&0).unwrap().is_disputed());
            assert!(client.tx_history.get(&0).unwrap().is_resolved());
            assert!(!client.tx_history.get(&0).unwrap().is_charged_back());
            sender.send(Transaction::new(Type::Resolve, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
            client.tx_history.get_mut(&0).unwrap().clear_flags();
            client.account.set_locked(true);
            sender.send(Transaction::new(Type::Resolve, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
        });
    }

    #[test]
    fn test_client_handle_charge_back() {
        let (sender, receiver) = tokio::sync::mpsc::channel(32);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = AccountAdmin::new(1, receiver);
            client.account.held = 2.0;
            client.tx_history.insert(0, Transaction::new_with_amount(Type::Deposit, 1, 0, 2.0));
            client.tx_history.get_mut(&0).unwrap().mark_disputed();
            sender.send(Transaction::new(Type::Chargeback, 1, 0)).await.unwrap();
            client.handle().await.unwrap();
            assert_eq!(client.account().held(), 0.0);
            assert_eq!(client.account().available(), 0.0);
            assert_eq!(client.account().is_locked(), true);
            assert_eq!(client.tx_history.get(&0).unwrap().is_disputed(), false);
            assert_eq!(client.tx_history.get(&0).unwrap().is_resolved(), false);
            assert_eq!(client.tx_history.get(&0).unwrap().is_charged_back(), true);
            // Try to charge back the same transaction again results in error, because it was already
            // disputed.
            sender.send(Transaction::new(Type::Chargeback, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
            client.tx_history.get_mut(&0).unwrap().clear_flags();
            // Even if the transaction flags are cleared, the account is locked after a `chargeback`,
            // so retrying the operation again result in error.
            sender.send(Transaction::new(Type::Chargeback, 1, 0)).await.unwrap();
            assert!(client.handle().await.is_err());
        });
    }

}