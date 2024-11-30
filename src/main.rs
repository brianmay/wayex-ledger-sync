use chrono::DateTime;
use chrono::NaiveDate;
use chrono::Utc;
use clap::Parser;
use rust_decimal::Decimal;
use std::path::Path;
use std::path::PathBuf;
use std::{error::Error, process};

mod version;

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
enum TransactionType {
    Received,
    Spent,
    Sent,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Copy, Clone, Eq, PartialEq)]
enum Currency {
    #[serde(rename = "BTC")]
    Btc,
    #[serde(rename = "XRP")]
    Xrp,
    #[serde(rename = "BCH")]
    Bch,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct WayexRecord {
    #[serde(rename = "Time")]
    time: DateTime<Utc>,

    #[serde(rename = "Crypto")]
    crypto: Currency,

    #[serde(rename = "Amount (AUD)")]
    amount_aud: Decimal,

    #[serde(rename = "Amount (Crypto)")]
    amount_crypto: Decimal,

    #[serde(rename = "Type")]
    transaction_type: TransactionType,

    #[serde(rename = "Transaction ID")]
    transaction_id: String,

    #[serde(rename = "Fees")]
    fees: Decimal,

    #[serde(rename = "Destination")]
    destination: String,

    #[serde(rename = "Details/TX Hash")]
    details: String,
}

impl WayexRecord {
    fn get_amount(&self) -> Decimal {
        match self.transaction_type {
            TransactionType::Received => self.amount_crypto,
            TransactionType::Spent => -self.amount_crypto,
            TransactionType::Sent => -self.amount_crypto,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct LedgerRecord {
    date: NaiveDate,
    description: String,
    currency: Currency,
    amount: Decimal,
}

fn example(wayex_file: &Path, ledger_file: &Path) -> Result<(), Box<dyn Error>> {
    let weyex_data: Vec<WayexRecord> = {
        let mut rdr = csv::Reader::from_path(wayex_file)?;
        let results: Result<Vec<WayexRecord>, _> = rdr.deserialize().collect();
        results?
            .into_iter()
            .filter(|x| x.crypto == Currency::Btc)
            .rev()
            .collect()
    };

    // let mut total = Decimal::from_i128_with_scale(0, 8);
    // for record in results {
    //     // println!("{:?}", record);
    //     let amount = record.get_amount();
    //     total = total + amount;
    //     println!(
    //         "{} {:40} {:.8} {:.8}",
    //         record.time, record.details, amount, total,
    //     );
    // }

    let ledger_data = {
        let mut rdr = csv::Reader::from_path(ledger_file)?;
        let results: Result<Vec<LedgerRecord>, _> = rdr.deserialize().collect();
        results?
    };

    let mut wayex_total = Decimal::from_i128_with_scale(0, 8);
    let mut ledger_total = Decimal::from_i128_with_scale(0, 8);

    for (wayex, ledger) in weyex_data.iter().zip(ledger_data.iter()) {
        let wayex_amount = wayex.get_amount();
        wayex_total += wayex_amount;

        let ledger_amount = ledger.amount;
        ledger_total += ledger_amount;

        if wayex_amount != ledger_amount || wayex_total != ledger_total {
            println!();
            println!(
                "{:24} {:40} {:.8} {:.8}",
                wayex.time, wayex.details, wayex_amount, wayex_total,
            );
            println!(
                "{:24} {:40} {:.8} {:.8}",
                ledger.date, ledger.description, ledger_amount, ledger_total,
            );
            panic!("Amounts do not match");
        } else {
            println!(
                "{:24} {:40} {:.8} {:.8}",
                ledger.date, ledger.description, ledger_amount, ledger_total,
            );
        }

        // println!(
        //     "{} {:40} {:.8} {:.8} {:.8} {:.8}",
        //     wayex.time,
        //     wayex.details,
        //     wayex.get_amount(),
        //     wayex_total,
        //     ledger.amount,
        //     ledger_total,
        // );
    }
    // for record in ledger_data {
    //     let amount = record.amount;
    //     total += amount;
    //     println!(
    //         "{} {:40} {:.8} {:.8}",
    //         record.date, record.description, amount, total,
    //     );
    // }

    Ok(())
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    wayex_file: PathBuf,

    #[arg(short, long)]
    ledger_file: PathBuf,

    #[arg(short, long)]
    version: bool,
}

fn main() {
    let args = Args::parse();

    if args.version {
        println!(
            "wayex_ledger v{} ({} {})",
            version::VERSION,
            version::VCS_REF.unwrap_or("unknown"),
            version::BUILD_DATE.unwrap_or("unknown"),
        );
        process::exit(0);
    }

    if let Err(err) = example(&args.wayex_file, &args.ledger_file) {
        println!("error running example: {}", err);
        process::exit(1);
    }
}
