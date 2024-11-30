use chrono::DateTime;
use chrono::Local;
use chrono::NaiveDate;
use chrono::TimeZone;
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

    let mut ledger_data = {
        let mut rdr = csv::Reader::from_path(ledger_file)?;
        let results: Result<Vec<Option<LedgerRecord>>, _> =
            rdr.deserialize().map(|x| x.map(Some)).collect();
        results?
    };

    let mut wayex_total = Decimal::from_i128_with_scale(0, 8);

    for wayex in &weyex_data {
        let wayex_amount = wayex.get_amount();
        wayex_total += wayex_amount;

        let time = Local.from_utc_datetime(&wayex.time.naive_utc());
        let time_str = time.format("%Y-%m-%d %H:%M:%S").to_string();

        println!(
            "{} {:40} {:.8} {:.8}",
            time_str, wayex.details, wayex_amount, wayex_total,
        );

        if wayex_amount == Decimal::from_i128_with_scale(0, 8) {
            println!();
            continue;
        }

        let ledger = ledger_data
            .iter_mut()
            .find(|ledger| {
                if let Some(ledger) = ledger {
                    let date = time.date_naive();
                    let date_diff = ledger.date - date;
                    if ledger.amount != wayex_amount {
                        false
                    } else if date_diff.num_days() < -14 || date_diff.num_days() > 14 {
                        println!(
                            "{}          {:40} {:.8}",
                            ledger.date, ledger.description, ledger.amount
                        );
                        println!("Date mismatch: {} {}", date_diff.num_days(), ledger.date);
                        false
                    } else {
                        true
                    }
                } else {
                    false
                }
            })
            .map(|x| x.take());

        if let Some(Some(ledger)) = ledger {
            println!(
                "{}          {:40} {:.8}",
                ledger.date, ledger.description, ledger.amount
            );
        } else {
            panic!("Could not find ledger record");
        }

        println!();
    }

    println!("Unaccounted ledger records:");

    for ledger in ledger_data.iter().filter_map(|x| x.as_ref()) {
        println!(
            "{:24} {:40} {:.8}",
            ledger.date, ledger.description, ledger.amount,
        );
    }

    Ok(())
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = "Synchronise Wayex and Legdger-CLI")]
struct Args {
    #[arg(short, long)]
    wayex_file: PathBuf,

    #[arg(short, long)]
    ledger_file: PathBuf,

    #[arg(short, long)]
    build_version: bool,
}

fn main() {
    let args = Args::parse();

    if args.build_version {
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
