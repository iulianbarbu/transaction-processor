# Transaction processor



This project is an example of using [tokio](https://github.com/tokio-rs/tokio) 
multi-threading runtime to execute transactions over multiple accounts, 
concurrently. More on the components that made up the transaction processor can
be found under [design.md](docs/design.md).

## Build

The project can be built by using `cargo build`, using rust 1.55.0. It is built
at the same time as a library and binary.

## Processing transactions

Running the binary, that processes a set of transactions described in a regular 
text file, on local filesystem, can be done by: 
`cargo run -- <filename path relative to cargo project root>`. The binary
output is a set of accounts, printed line by line, with respect to the schema
`client,available,held,total,locked`.

## Testing

Running the unit tests can be done by `cargo test`. The test are covering all
the units of the project, except for the ones from the `logger` module and the 
`drill` orphan function, present in transaction module, which is exercised 
mainly in *integration context*, during the benchmarks tests and put out for
manipulating the tokio asynchronous runtime and the abstractions introduced by
the project.

## Coverage

Coverage was computed by running `cargo kcov`. The details on coverage can be
found in [coverage.json](coverage.json).

## Benchmarks

The purpose of the existing implemented benchmarks are exercising the
transaction processing in terms of scalability and asynchronous multi-threaded
runtime. Two performance tests were separated in two groups:
* `small-inputs`
* `large-inputs`

The transaction used for the benchmarks is `deposit` and the assumption is that 
all transaction types have the same cost, so it does not matter what type of 
transaction we use to showcase the scalability and multi-threaded runtime at 
action. In order to really make use of the multi-threaded environment in a 
setup that does not involve millions of transactions, the transaction execution 
supports interfering into the execution of a transaction, by delaying it with
a 100 millis for every executed transaction. This enables greater contention
on system resources, in the period of time the transaction is delayed, because
the transaction processor wants to fulfil other transaction at the same time.

Running all the benchmarks can be done by: `cargo bench`. Benchmarks run on a
`Intel(R) Core(TM) i9-9880H CPU @ 2.30GHz`, with a 16 cores CPU, can be found
bellow:

* 100 iterations over [1-client-20-deposits](benches/1-client-20-deposits.in):
    `time:  [ 2.1548 s 2.1564 s 2.1579 s]` 
* 100 iterations over [20-clients-20-deposits](benches/20-clients-20-deposits.in):
    `time:  [ 411.23 ms 411.83 ms 412.42 ms]`
* 100 iterations over [1-client-100-deposits](benches/1-client-1000-deposits.in):
    `time:  [ 10.349 s 10.353 s 10.357 s]`
* 100 iterations over [50-client-100-deposits](benches/50-clients-100-deposits.in):
    `time:  [1.2296 s 1.2307 s 1.2318 s]`
* 100 iterations over [100-client-100-deposits](benches/50-clients-100-deposits.in):
    `time:  [ 1.4356 s 1.4368 s 1.4380 s]`
