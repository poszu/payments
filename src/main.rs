use clap::Parser;
use payments::{parser::parse, payments::Payments};

#[derive(Parser)]
struct Cli {
    input: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename = Cli::parse().input;
    let mut payments = Payments::default();

    let rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(filename)
        .expect("opening transactions input file");

    for trans in parse(rdr) {
        if let Err(error) = payments.apply(trans?) {
            eprintln!("Transaction failed: '{}'", error);
        }
    }

    payments.serialize(std::io::stdout())
}
