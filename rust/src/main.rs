// src/main.rs
use clap::Parser;
use serde_yaml::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// Folder to work in
    #[arg(short, long)]
    path: std::path::PathBuf,
}
// inlay - hints: press CTRL + ALT

struct Main {
    working_dir: PathBuf,
    config: HashMap<String, Value>,
    config_file: PathBuf,
}

impl Main {
    fn new(working_dir: PathBuf) -> Self {
        let config_file = working_dir.join("config.yml");
        let config = if config_file.exists() {
            println!("Found config file in {:?}", config_file);
            let file = fs::File::open(&config_file).expect("Unable to open config file");
            serde_yaml::from_reader(file).expect("Unable to parse config file")
        } else {
            println!("No config file {:?} found.", config_file);
            HashMap::new()
        };

        for folder in &[
            "1_imports",
            "1_imports/bank",
            "2_rules",
            "3_manual",
            "4_output",
            "5_analysis",
        ] {
            fs::create_dir_all(working_dir.join(folder)).expect("Unable to create directory");
        }

        Main {
            working_dir,
            config,
            config_file,
        }
    }
}
// impl Main {
//     fn import(&self) -> polars::prelude::DataFrame {
//         println!("Importing transactions..");
//         let parsed = self.parse_transactions();
//         if parsed.is_empty() {
//             println!("No transactions found, exiting.");
//             std::process::exit(0);
//         }
//         let combined_transactions = concat(&parsed.values().collect::<Vec<_>>()).unwrap();
//         let combined_transactions_enriched =
//             self.enrich_transactions_with_amazon_data(combined_transactions);
//         self.correct_balance(combined_transactions_enriched)
//     }
//
//     fn parse_transactions(&self) -> HashMap<PathBuf, polars::prelude::DataFrame> {
//         let mut parsed = HashMap::new();
//         for entry in fs::read_dir(self.working_dir.join("1_imports/bank")).unwrap() {
//             let entry = entry.unwrap();
//             if entry.path().is_file() {
//                 continue;
//             }
//             let parser = ConfigFileBasedParser::new(entry.path());
//             parsed.insert(entry.path(), parser.parse());
//         }
//         parsed
//     }
//
//     fn enrich_transactions_with_amazon_data(
//         &self,
//         combined_transactions: polars::prelude::DataFrame,
//     ) -> polars::prelude::DataFrame {
//         let amazon_folder = self.working_dir.join("1_imports/amazon");
//         if !amazon_folder.exists() {
//             return combined_transactions;
//         }
//
//         let amazon_all = ConfigFileBasedParser::new(amazon_folder)
//             .parse()
//             .groupby(&["amazon_order_id", "amazon_asin"])
//             .first()
//             .unwrap();
//
//         let combined_with_amazon_info = combined_transactions
//             .lazy()
//             .with_column(
//                 polars::prelude::col("desc")
//                     .str()
//                     .split(" ")
//                     .list()
//                     .first()
//                     .alias("amazon_order_id")
//                     .when(polars::prelude::col("partner").str().contains("AMAZON"))
//                     .then(polars::prelude::col("desc")),
//             )
//             .sort("date", false)
//             .join(
//                 amazon_all.lazy(),
//                 ["amazon_order_id"],
//                 ["amazon_order_id"],
//                 polars::prelude::JoinType::Left,
//             )
//             .with_column(
//                 polars::prelude::col("desc_order")
//                     .is_not_null()
//                     .when_then(
//                         polars::prelude::col("desc")
//                             + polars::prelude::lit(" amazon_product:")
//                             + polars::prelude::col("desc_order"),
//                         polars::prelude::col("desc"),
//                     )
//                     .alias("desc"),
//             )
//             .drop(&[
//                 "desc_order",
//                 "account_right",
//                 "amazon_asin",
//                 "amazon_order_id",
//                 "date_right",
//             ])
//             .collect()
//             .unwrap();
//
//         combined_with_amazon_info
//     }
//
//     fn correct_balance(
//         &self,
//         combined_transactions: polars::prelude::DataFrame,
//     ) -> polars::prelude::DataFrame {
//         let online_balances_file = self.working_dir.join("1_imports/online_balances.csv");
//         if !online_balances_file.exists() {
//             println!("No online balances file found, skipping balance correction. Consider creating one.");
//             return combined_transactions;
//         }
//
//         let daily_balances = combined_transactions
//             .lazy()
//             .sort(&["date", "account"], false)
//             .groupby(&["account", "date"])
//             .agg(&[polars::prelude::col("amount").sum().alias("amount")])
//             .with_column(
//                 polars::prelude::col("amount")
//                     .cumsum(false)
//                     .over(&["account"])
//                     .alias("balance"),
//             )
//             .sort(&["account", "date"], false)
//             .select(&["date", "account", "balance"])
//             .collect()
//             .unwrap();
//
//         let real_balances = polars::prelude::CsvReader::from_path(online_balances_file)
//             .unwrap()
//             .try_parse_dates(true)
//             .with_schema_overrides(&[("amount", polars::prelude::DataType::Float64)])
//             .finish()
//             .unwrap()
//             .lazy()
//             .with_column(
//                 polars::prelude::lit("Balance correction according to online status").alias("desc"),
//             )
//             .with_column(
//                 (polars::prelude::lit("account:") + polars::prelude::col("account"))
//                     .alias("account1"),
//             )
//             .with_column(polars::prelude::lit("balance_correction").alias("account2"))
//             .sort("date", false)
//             .collect()
//             .unwrap();
//
//         let correction_transactions = real_balances
//             .lazy()
//             .join_asof(
//                 daily_balances.lazy(),
//                 "date",
//                 "account",
//                 polars::prelude::JoinType::Backward,
//             )
//             .sort(&["account", "date"], false)
//             .with_column(
//                 (polars::prelude::col("online_balance") - polars::prelude::col("balance"))
//                     .round(2)
//                     .alias("agb"),
//             )
//             .with_column(
//                 polars::prelude::col("agb")
//                     .shift(1)
//                     .over(&["account"])
//                     .fill_null(polars::prelude::lit(0))
//                     .alias("last_agb"),
//             )
//             .with_column(
//                 (polars::prelude::col("agb") - polars::prelude::col("last_agb")).alias("agb_final"),
//             )
//             .with_column(polars::prelude::col("agb_final").alias("amount"))
//             .with_column(polars::prelude::lit(None).alias("partner"))
//             .with_column(polars::prelude::lit(None).alias("classification"))
//             .with_column(polars::prelude::lit(None).alias("partner_iban"))
//             .select(combined_transactions.get_column_names())
//             .filter(polars::prelude::col("amount").abs().gt(0))
//             .collect()
//             .unwrap();
//
//         let transactions_corr =
//             polars::prelude::concat(&[combined_transactions, correction_transactions]).unwrap();
//         transactions_corr.sort("date", false)
//     }
// }
//
// def _1_import(self):
// print("Importing transactions..")
// parsed = self._parse_transactions()
// if len(parsed) == 0:
//     print("No transactions found, exiting.")
//     sys.exit(0)
// combined_transactions = pl.concat(parsed.values())
// combined_transactions_enriched = self._enrich_transactions_with_amazon_data(
//     combined_transactions
// )
// combined_transactions_enriched_with_balance_corrections = self._correct_balance(
//     combined_transactions_enriched
// )
// return combined_transactions_enriched_with_balance_corrections
//
// def _parse_transactions(self):
// parsed = {}
// for folder in (self.working_dir / "1_imports" / "bank").iterdir():
//     if folder.is_file():
//         continue
//     parsed[folder] = ConfigFileBasedParser(folder=folder).parse()
//
// return parsed
//
// def _enrich_transactions_with_amazon_data(
// self, combined_transactions: pl.DataFrame
// ):
// amazon_folder = self.working_dir / "1_imports" / "amazon"
// if not amazon_folder.exists():
//     return combined_transactions
//
// amazon_all = (
//     ConfigFileBasedParser(folder=self.working_dir / "1_imports" / "amazon")
//     .parse()
//     .group_by("amazon_order_id", "amazon_asin")
//     .first()
// )
//
// combined_with_amazon_info = (
//     combined_transactions.with_columns(
//         amazon_order_id=pl.when(pl.col("partner").str.contains("AMAZON")).then(
//             pl.col("desc").str.split(" ").list.first()
//         ),
//     )
//     .sort("date", descending=True)
//     .join(
//         amazon_all,
//         on="amazon_order_id",
//         how="left",
//     )
//     .with_columns(
//         desc=pl.when(pl.col("desc_order").is_not_null())
//         .then(pl.col("desc") + " amazon_product:" + pl.col("desc_order"))
//         .otherwise(pl.col("desc"))
//     )
// ).drop(
//     "desc_order",
//     "account_right",
//     "amazon_asin",
//     "amazon_order_id",
//     "date_right",
// )
//
// return combined_with_amazon_info
//
// def _correct_balance(self, combined_transactions: pl.DataFrame):
// online_balances_file = self.working_dir / "1_imports" / "online_balances.csv"
// if not online_balances_file.exists():
//     print(
//         "No online balances file found, skipping balance correction. Consider creating one."
//     )
//     return combined_transactions
//
// daily_balances = (
//     combined_transactions.sort("date", "account")
//     .group_by("account", "date", maintain_order=True)
//     .agg(amount=pl.col("amount").sum())
//     .with_columns(
//         balance=pl.cum_sum("amount").over(
//             "account", order_by="date", mapping_strategy="group_to_rows"
//         )
//     )
//     .sort("account", "date")
// ).select("date", "account", "balance")
//
// real_balances = (
//     pl.read_csv(
//         online_balances_file,
//         try_parse_dates=True,
//         schema_overrides={"amount": pl.Float64},
//     )
//     .with_columns(
//         desc=pl.lit("Balance correction according to online status"),
//         account1=pl.lit("account:") + pl.col("account"),
//         account2=pl.lit("balance_correction"),
//     )
//     .sort("date")
// )
//
// correction_transactions = (
//     real_balances.join_asof(
//         daily_balances, on="date", by="account", strategy="backward"
//     )
//     .sort("account", "date")
//     .with_columns(agb=(pl.col("online_balance") - pl.col("balance")).round(2))
//     .with_columns(last_agb=pl.col("agb").shift(1).over("account").fill_null(0))
//     .with_columns(agb_final=pl.col("agb") - pl.col("last_agb"))
//     .with_columns(
//         amount=pl.col("agb_final"),
//         partner=None,
//         classification=None,
//         partner_iban=None,
//     )
//     .select(combined_transactions.columns)
// ).filter(pl.col("amount").abs() > 0)
//
// transactions_corr = pl.concat(
//     [combined_transactions, correction_transactions]
// ).sort("date")
//
// return transactions_corr

fn main() {
    let args = Cli::parse();

    println!(" path: {:?}", args.path);

    let m = Main::new(args.path);

    println!("{:?}", m.config)
}
