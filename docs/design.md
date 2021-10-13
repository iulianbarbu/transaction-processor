# Transaction processor design & assumptions

The project follows the hard rules below in terms of how components interact:
* Transactions execution consumes transactions from an input source.
* The execution means passing transactions to an account admin.
* Each account admin works independent of the others to execute its assigned
  transactions.
* The account admins tasks are scheduled by a multi-threaded runtime, to scale 
  transactions execution for multiple account admins, at the same time.
* The order of the transactions that target a specific account admin, as they 
  are  gathered from the input source, is the chronological order in which they
  are executed by that account admin.
* Once an account is locked, then execution of future transactions for the 
  account owner will result in error, that is handled gracefully by the driver.

## Account admin

This abstraction is the owner of an account information, in terms of applying
transactions on top of it. It is a component that once started in an async
runtime,  it will wait on a channel for transactions, in an infinite loop, to
process them. Once the channel is closed the component will return with the
account.

## Input

This abstraction is a wrapper over a `std::fs::File` that iterates through
the contents of the file, line by line.

## Logger

Not intensively used for this project, because each message logged means
additional overhead on the transaction execution hot path. It is used few times
for providing structure and severity attached to messages printed to stdout.

## Transaction

The abstractions around transactions provide support for transforming a string
into a `Transaction` struct, a higher level iterator that consumes an `Input`
and iterates over the transactions extracted from the input, but also example
for driving all the abstractions together and handling a set of transactions
concurrently.