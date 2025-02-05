use crate::util::read_yaml_file;
use crate::EXPECTED_BOW_FOLDERS;
use polars::frame::DataFrame;
use polars::prelude::{CsvParseOptions, CsvReadOptions, CsvReader, SerReader};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const PARSER_CONFIG_FILE_NAME: &str = "parser_config.yml";

pub struct BowParser<'a> {
    folder: &'a Path,
    parse_config: HashMap<String, serde_yaml::Value>,
    expected_out_columns: [&'static str; 5],
    banking_input_csvs: Vec<PathBuf>,
}

impl<'a> BowParser<'a> {
    pub fn new(folder: &'a Path) -> Self {
        let parse_config_yaml = read_yaml_file(&folder.join(PARSER_CONFIG_FILE_NAME));

        let mut banking_input_csvs = Vec::new();
        for entry in WalkDir::new(folder)
            .into_iter()
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap().into_path())
            .filter(|x| x.is_file() && x.extension().is_some() && x.extension().unwrap() == "csv")
        {
            print!("parser checks {:?}", entry);
            let parent_folder = entry
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");

            println!(
                "Found banking csv '{}' in folder '{}'",
                entry.file_name().unwrap().to_str().unwrap(),
                parent_folder,
            );
            banking_input_csvs.push(entry.to_path_buf());
        }

        Self {
            folder,
            parse_config: parse_config_yaml.unwrap_or(HashMap::new()),
            expected_out_columns: ["date", "amount", "partner", "account", "partner_iban"],
            banking_input_csvs,
        }
    }

    pub fn parse(&self) -> Option<DataFrame> {
        for csv in &self.banking_input_csvs {
            let df: DataFrame = CsvReadOptions::default()
                .with_skip_rows(
                    self.parse_config
                        .get("read_csv")
                        .unwrap()
                        .get("skip_rows")
                        .unwrap()
                        .as_i64()
                        .unwrap_or(0) as usize,
                )
                .with_parse_options(CsvParseOptions::default().with_separator(b';'))
                .try_into_reader_with_file_path(Some(csv.clone()))
                .unwrap()
                .finish()
                .unwrap();
            // let df = CsvReader::new(file.unwrap()).finish().unwrap();
            print!("{:?}", df);
        }
        None
    }
}

// class Parser:
//     def __init__(
//         self,
//         folder: Path,
//         expected_out_columns=bank_transaction_columns,
//     ):
//         self.folder = folder
//         self.expected_out_columns = expected_out_columns

//     def parse(self) -> pl.DataFrame | None:
//         files = list(self.folder.glob("*.csv"))
//         if len(files) == 0:
//             raise FileNotFoundError(f"No files found in {self.folder}")

//         account_to_timeranges_already_parsed: defaultdict[
//             str, list[(datetime, datetime)]
//         ] = defaultdict(list)

//         dfs = []
//         for file in files:
//             print(f"    Parsing {file.relative_to(self.folder.parent.parent)}..")
//             df = self.parse_single_file(file)
//             assert df.columns == self.expected_out_columns
//             for account, timeranges in account_to_timeranges_already_parsed.items():
//                 for start, end in timeranges:
//                     df = df.filter(
//                         (pl.col("account") != account)
//                         | (pl.col("date") < start)
//                         | (pl.col("date") > end)
//                     )
//             timeframe_parsed = df.group_by("account").agg(
//                 min_time=pl.min("date"), max_time=pl.max("date")
//             )
//             for row in timeframe_parsed.iter_rows(named=True):
//                 account = row["account"]
//                 min_time = row["min_time"]
//                 max_time = row["max_time"]
//                 account_to_timeranges_already_parsed[account].append(
//                     (min_time, max_time)
//                 )

//             dfs.append(df)

//         df = pl.concat(dfs)

//         if "partner_iban" in df.columns:
//             df = df.with_columns(
//                 partner_iban=pl.when(pl.col("partner_iban").is_not_null())
//                 .then(pl.col("partner_iban").cast(pl.String).str.replace_all(" ", ""))
//                 .otherwise(pl.col("partner_iban"))
//             )
//         df = df.select(self.expected_out_columns)
//         return df

//     def parse_single_file(self, file: Path) -> pl.DataFrame:
//         raise NotImplementedError()

// class ConfigFileBasedParser(Parser):
//     def __init__(self, folder: Path):
//         super().__init__(folder)

//         self.parse_config_file = folder / "parser_config.yml"
//         if not self.parse_config_file.exists():
//             raise FileNotFoundError(f"No config file found in {folder}")

//         with open(self.parse_config_file, encoding="UTF-8") as file:
//             self.config: dict[str, str] = yaml.load(file, Loader=yaml.FullLoader)

//         if "expected_out_columns" in self.config:
//             self.expected_out_columns = self.config["expected_out_columns"]

//     def parse_single_file(self, file: Path) -> pl.DataFrame:
//         df = pl.read_csv(file, **self.config.get("read_csv", {}))

//         if self.config.get("pre_rename", {}).get("lower_columns", False):
//             df = df.rename({col: col.lower() for col in df.columns})
//         if self.config.get("pre_rename", {}).get("strip_spaces", False):
//             df = df.rename({col: col.replace(" ", "") for col in df.columns})

//         rename_dict = {
//             value: key for key, value in self.config.get("rename", {}).items()
//         }

//         df: pl.DataFrame = df.rename(rename_dict)
//         if "amount" in df.columns:
//             if df.dtypes[df.columns.index("amount")] == pl.String:
//                 df = df.with_columns(
//                     pl.col("amount")
//                     .cast(pl.String)
//                     .str.replace(r"\.", "")
//                     .str.replace(r",", ".")
//                     .fill_null(0)
//                     .cast(pl.Float64)
//                 )
//             df = df.with_columns(amount=pl.col("amount").cast(pl.Float64))

//         if "date_format" in self.config:
//             df = df.with_columns(
//                 date=pl.col("date")
//                 .str.to_datetime(self.config["date_format"])
//                 .cast(pl.Date)
//             )

//         if partner_settings := self.config.get("partner_settings", None):
//             if (
//                 "amount" in df.columns
//                 and "partner_column_if_amount_negative" in partner_settings
//                 and "partner_column_if_amount_positive" in partner_settings
//             ):
//                 when_condition = pl.col("amount") < 0
//                 if partner_settings.get("use_other_column_if_partner_empty", False):
//                     when_condition = (
//                         when_condition
//                         & pl.col(
//                             partner_settings["partner_column_if_amount_negative"]
//                         ).is_not_null()
//                     ) | (
//                         pl.col(
//                             partner_settings["partner_column_if_amount_positive"]
//                         ).is_null()
//                     )

//                 df = df.with_columns(
//                     partner=pl.when(when_condition)
//                     .then(pl.col(partner_settings["partner_column_if_amount_negative"]))
//                     .otherwise(
//                         pl.col(partner_settings["partner_column_if_amount_positive"])
//                     )
//                 )

//         if account_settings := self.config.get("account_settings", None):
//             if "account_name" in account_settings:
//                 df = df.with_columns(
//                     account=pl.lit(self.config["account_settings"]["account_name"])
//                 )
//             elif account_settings.get("account_name_is_file_name", False):
//                 df = df.with_columns(account=pl.lit(file.stem))

//             if "account_aliases" in account_settings:
//                 df = df.with_columns(
//                     account=pl.col("account").replace(
//                         account_settings["account_aliases"]
//                     )
//                 )

//         for col in self.expected_out_columns:
//             if col not in df.columns:
//                 df = df.with_columns(pl.lit(None).alias(col))

//         if row_filter := self.config.get("row_filter", None):
//             if "date_begin" in row_filter:
//                 df = df.filter(pl.col("date") >= row_filter["date_begin"])
//             if "date_end" in row_filter:
//                 df = df.filter(pl.col("date") < row_filter["date_end"])

//         df = df.filter(pl.col("account").is_not_null()).select(
//             self.expected_out_columns
//         )
//         return df
