use beancount_parser::parse;
use beancount_parser::BeancountFile;
use beancount_parser::DirectiveContent;
use chrono::Local;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::TimeZone;
use clap::Parser;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
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
    Sell,
    #[serde(rename = "Bank Withdrawal (BSB)")]
    BankWithdrawal,
    #[serde(rename = "Card (Purchase)")]
    CardPurchase,
    #[serde(rename = "Card (Refund)")]
    CardRefund,
    #[serde(rename = "Crypto Deposit")]
    CryptoDeposit,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Copy, Clone, Eq, PartialEq)]
enum Currency {
    #[serde(rename = "AUD")]
    Aud,
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
    #[serde(rename = "Date/Time")]
    time: String,

    #[serde(rename = "Type")]
    transaction_type: TransactionType,

    #[serde(rename = "Asset")]
    crypto: Currency,

    #[serde(rename = "Amount AUD")]
    amount_aud: Option<Decimal>,

    #[serde(rename = "Amount Crypto")]
    amount_crypto: Option<Decimal>,

    #[serde(rename = "Details")]
    details: String,

    #[serde(rename = "Reference")]
    reference: String,
}

impl WayexRecord {
    fn get_amount(&self) -> Option<Decimal> {
        let amount = self.amount_crypto?;
        let amount = match self.transaction_type {
            TransactionType::Received => amount,
            TransactionType::CryptoDeposit => amount,
            TransactionType::CardRefund => amount,
            TransactionType::Spent => -amount,
            TransactionType::BankWithdrawal => -amount,
            TransactionType::CardPurchase => -amount,
            TransactionType::Sell => -amount,
            TransactionType::Sent => -amount,
        };
        Some(amount)
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

    let mut ledger_data: Vec<Option<LedgerRecord>> = {
        let unparsed_file = std::fs::read_to_string(ledger_file)?;
        let ledger: BeancountFile<rust_decimal::Decimal> = parse(&unparsed_file)?;
        // dbg!(ledger);

        ledger
            .directives
            .into_iter()
            .filter_map(|d| {
                let DirectiveContent::Transaction(transaction) = d.content else {
                    return None;
                };

                let Some(description) = transaction.narration else {
                    panic!("No description");
                };

                let posting = transaction
                    .postings
                    .into_iter()
                    .find(|p| p.account.as_str() == "Assets:Cash-On-Hand:CryptoSpend:BTC")?;

                let Some(amount) = posting.amount else {
                    panic!("No amount");
                };

                if amount.currency.as_str() != "BTC" {
                    panic!("Invalid currency");
                }

                Some(Some(LedgerRecord {
                    date: NaiveDate::from_ymd_opt(
                        d.date.year.into(),
                        d.date.month.into(),
                        d.date.day.into(),
                    )
                    .unwrap(),
                    description,
                    amount: amount.value,
                    currency: Currency::Btc,
                }))
            })
            .collect()
    };

    let mut wayex_total = Decimal::from_i128_with_scale(0, 8);

    let mut spent = Decimal::from_i128_with_scale(0, 8);
    let mut paid = Decimal::from_i128_with_scale(0, 8);

    for wayex in &weyex_data {
        let Some(wayex_amount) = wayex.get_amount() else {
            continue;
        };
        wayex_total += wayex_amount;
        if wayex_amount > dec! { 0 } {
            paid += wayex_amount;
        }
        if wayex_amount < dec! { 0 } {
            spent += wayex_amount;
        }

        let maybe_time = NaiveDateTime::parse_from_str(&wayex.time, "%a, %d %b %Y, %I:%M %P");
        let time = match maybe_time {
            Ok(time) => time,
            Err(err) => panic!("Cannot parse datetime {}: {err}", wayex.time),
        };
        let time = Local.from_local_datetime(&time).unwrap();
        println!("{} {}", wayex.time, time);
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
                    let date_diff = date - ledger.date;
                    if ledger.amount != wayex_amount {
                        false
                    } else if date_diff.num_days() < -2 || date_diff.num_days() > 14 {
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

    println!();
    println!("spent {spent}");
    println!("paid {paid}");
    println!("total {wayex_total}");
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
