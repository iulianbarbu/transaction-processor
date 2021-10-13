use std::env;
use std::fs::File;
use std::process::exit;
use transaction_processor::input::Input;
use transaction_processor::logger::init_logger;
use transaction_processor::transaction::drill;

fn main() {
    if init_logger().is_err() {
        log::error!("Could not initialse the logger. Exiting...");
        exit(1);
    }

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        log::error!("Invalid arguments. Please provide a correctly formatted csv file.\n\
        Example of csv file:
        deposit,1,1,1.0
        withdrawal,1,2,0.5
        deposit,2,3,1.0
        dispute,2,3
        resolve,2,3,
        dispute,2,3
        chargeback,2,3");
        exit(1);
    }

    let file_path = args.get(1).unwrap();
    let result = File::open(file_path);
    if result.is_err() {
        log::error!("Invalid path. Please provide the path to a correctly formatted csv file.\n\
        Example of csv file:
        deposit,1,1,1.0
        withdrawal,1,2,0.5
        deposit,2,3,1.0
        dispute,2,3
        resolve,2,3,
        dispute,2,3
        chargeback,2,3");
        exit(1);
    }

    // Process the tx from input.
    drill(Input::from(result.unwrap()), true, None, true);

}
