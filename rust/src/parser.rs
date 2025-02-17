use crate::util::read_yaml_file;
use log::{info, warn};
use polars::error::{PolarsError, PolarsResult};
use polars::frame::DataFrame;
use polars::prelude::{
    concat, CsvParseOptions, CsvReadOptions, NullValues, PlSmallStr, SerReader, StringMethods,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

use polars::prelude::*;

const PARSER_CONFIG_FILE_NAME: &str = "parser_config.yml";
const EXPECTED_BOW_COLUMNS: [&str; 5] = ["date", "amount", "partner", "account", "partner_iban"];

pub struct BowParser {
    parse_config: HashMap<String, serde_yml::Value>,
    expected_out_columns: [&'static str; 5],
    banking_input_csvs: Vec<PathBuf>,
}

impl BowParser {
    pub fn new(
        parse_config: HashMap<String, serde_yml::Value>,
        banking_input_csvs: Vec<PathBuf>,
    ) -> BowParser {
        Self {
            parse_config,
            expected_out_columns: EXPECTED_BOW_COLUMNS,
            banking_input_csvs,
        }
    }

    pub fn from_folder(folder: &Path) -> Self {
        let parse_config_yaml = read_yaml_file(&folder.join(PARSER_CONFIG_FILE_NAME));

        let mut banking_input_csvs = Vec::new();
        for entry in WalkDir::new(folder)
            .into_iter()
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap().into_path())
            .filter(|x| x.is_file() && x.extension().is_some() && x.extension().unwrap() == "csv")
        {
            info!("parser checks {:?}", entry);
            let parent_folder = entry
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .unwrap_or("unknown");

            print!(
                "Found banking csv '{}' in folder '{}'",
                entry.file_name().unwrap().to_str().unwrap(),
                parent_folder,
            );
            banking_input_csvs.push(entry.to_path_buf());
        }

        Self {
            parse_config: parse_config_yaml.unwrap_or(HashMap::new()),
            expected_out_columns: EXPECTED_BOW_COLUMNS,
            banking_input_csvs,
        }
    }

    pub fn parse(&self) -> Result<DataFrame, PolarsError> {
        let mut dfs: Vec<LazyFrame> = Vec::new();
        for csv in &self.banking_input_csvs {
            let mut df = self.read_csv(csv)?;
            df = self.rename_df(df, &csv)?;
            df = self.convert_amount(df)?;
            df = self.convert_date_column(df, &csv)?;
            df = self.apply_account_settings(df, &csv)?;
            df = self.apply_partner_settings(df)?;
            df = df.select(self.expected_out_columns)?;
            dfs.push(df.lazy());
        }

        let df = concat(dfs, UnionArgs::default())?.collect()?;

        Ok(df)
        // TODO
        // row_filter:
        //   date_begin: 2022-01-01
    }

    fn apply_account_settings(
        &self,
        mut df: DataFrame,
        csv: &Path,
    ) -> Result<DataFrame, PolarsError> {
        if let Some(account_settings) = self.parse_config.get("account_settings") {
            if let Some(account_name) = account_settings.get("account_name") {
                df = df
                    .lazy()
                    .with_column(lit(account_name.as_str().unwrap()).alias("account"))
                    .collect()?;
            } else if account_settings
                .get("account_name_is_file_name")
                .and_then(|x| x.as_bool())
                .unwrap_or(false)
            {
                df = df
                    .lazy()
                    .with_column(lit(csv.file_stem().unwrap().to_str().unwrap()).alias("account"))
                    .collect()?;
            }

            if let Some(account_aliases) = account_settings.get("account_aliases") {
                for (pat, value) in account_aliases
                    .as_mapping()
                    .unwrap()
                    .into_iter()
                    .map(|(k, v)| (k.as_str().unwrap(), v.as_str().unwrap()))
                {
                    df = df
                        .lazy()
                        .with_column(col("account").str().replace(lit(pat), lit(value), false))
                        .collect()?;
                }
            }
        }
        Ok(df)
    }

    fn convert_date_column(&self, mut df: DataFrame, csv: &Path) -> Result<DataFrame, PolarsError> {
        let date_format = self.parse_config.get("date_format");
        df.with_column(
            df.column("date")?
                .str()?
                .as_date(date_format.and_then(|x| x.as_str()), false)
                .expect(format!("Error parsing date of {csv:?}").as_str()),
        )?;
        Ok(df)
    }

    fn rename_df(&self, mut df: DataFrame, csv: &Path) -> PolarsResult<DataFrame> {
        if let Some(rename) = self.parse_config.get("rename") {
            let rename_dict: HashMap<&str, &str> = rename
                .as_mapping()
                .unwrap()
                .iter()
                .map(|(key, value)| (value.as_str().unwrap(), key.as_str().unwrap()))
                .collect();

            for (key, value) in rename_dict {
                if let Err(e) = df.rename(key, value.into()) {
                    warn!(
                        "The column '{key}' of {csv:?} could not be renamed to '{value}'. Error: {e}"
                    );
                }
            }
        }

        Ok(df)
    }

    fn get_csv_read_options(&self) -> CsvReadOptions {
        let mut parse_options = CsvParseOptions::default();
        let mut read_options = CsvReadOptions::default();

        if let Some(read_csv) = self.parse_config.get("read_csv") {
            if let Some(separator_value) = read_csv.get("separator").and_then(|x| x.as_str()) {
                parse_options.separator = separator_value.chars().next().unwrap() as u8;
            }
            if let Some(decimal_comma_value) =
                read_csv.get("decimal_comma").and_then(|x| x.as_bool())
            {
                parse_options.decimal_comma = decimal_comma_value;
            }
            if let Some(null_values_value) =
                read_csv.get("null_values").and_then(|x| x.as_sequence())
            {
                parse_options.null_values = Some(NullValues::AllColumns(
                    null_values_value
                        .iter()
                        .map(|x| PlSmallStr::from_str(x.as_str().unwrap()))
                        .collect(),
                ));
            }
            if let Some(try_parse_dates) = read_csv.get("try_parse_dates").and_then(|x| x.as_bool())
            {
                parse_options.try_parse_dates = try_parse_dates;
            }

            if let Some(skip_rows_value) = read_csv.get("skip_rows").and_then(|x| x.as_i64()) {
                read_options.skip_rows = skip_rows_value as usize;
            }
            read_options.parse_options = Arc::new(parse_options);
        }
        read_options
    }

    fn apply_partner_settings(&self, mut df: DataFrame) -> Result<DataFrame, PolarsError> {
        if let Some(partner_columns) = self
            .parse_config
            .get("partner_settings")
            .and_then(|x| {
                x.get("partner_column_if_amount_negative")
                    .zip(x.get("partner_column_if_amount_positive"))
            })
            .map(|x| (x.0.as_str().unwrap(), x.1.as_str().unwrap()))
        {
            let (neg, pos) = partner_columns;

            df = df
                .lazy()
                .with_column(
                    (when(col("amount").lt(lit(0)))
                        .then(col(neg))
                        .otherwise(col(pos)))
                    .alias("partner"),
                )
                .collect()
                .map_err(|e| {
                    PolarsError::ComputeError(
                        format!(
                            "Could not set partner column negative: '{neg}' and positive: '{pos}', due to {e}"
                        )
                        .into(),
                    )
                })?;
        }
        Ok(df)
    }

    fn convert_amount(&self, mut df: DataFrame) -> Result<DataFrame, PolarsError> {
        if df.column("amount")?.dtype() == &DataType::String {
            df.with_column(
                df.column("amount")?
                    .str()?
                    .replace_all(r"\.", "")?
                    .replace_all(r",", ".")?
                    .cast(&DataType::Float64)?
                    .fill_null(FillNullStrategy::Zero)?,
            )?;
        }
        df.with_column(df.column("amount")?.cast(&DataType::Float64)?)?;
        Ok(df)
    }

    fn read_csv(&self, csv: &PathBuf) -> PolarsResult<DataFrame> {
        let read_options = self.get_csv_read_options();
        let df: DataFrame = read_options
            .try_into_reader_with_file_path(Some(csv.clone()))?
            .finish()?;
        Ok(df)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_dkb_works() {
        let folder = get_test_data_folder().join("dkb_test");
        let parser = BowParser::from_folder(folder.as_path());
        let df = parser.parse().unwrap();
        assert_eq!(df.get_column_names(), EXPECTED_BOW_COLUMNS);
    }

    fn get_test_data_folder() -> PathBuf {
        let folder = PathBuf::from(file!())
            .ancestors()
            .nth(1)
            .unwrap()
            .join("test_data");
        folder
    }

    fn test_skip_yml(skip: i32) {
        let yml = format!(
            r#"
read_csv:
    skip_rows: {skip}
        "#
        );
        let config = serde_yml::from_str(yml.as_str()).unwrap();
        let parser = BowParser::new(config, vec![]);
        let csv = get_test_data_folder().join("test_input_with_dummylines.csv");

        let df = parser.read_csv(&csv).unwrap();
        let skip_plus_one = skip + 1;
        assert_eq!(
            df.get_column_names(),
            vec![format!("Dummyline {skip_plus_one};").as_str()]
        );
    }

    #[test]
    fn test_parser_read_csv_skip_rows() {
        test_skip_yml(0);
        test_skip_yml(1);
        test_skip_yml(2);
        test_skip_yml(3);
    }

    #[test]
    fn test_rename() {
        let yml = r#"
    rename:
        date: "Buchungsdatum"
        classification: "Umsatztyp"
        amount: "Betrag (€)"
        desc: "Verwendungszweck"
        partner_iban: "IBAN"
            "#;

        let config = serde_yml::from_str(yml).unwrap();
        let parser = BowParser::new(config, vec![]);
        let csv = get_test_data_folder().join("test_input_rename.csv");
        let mut df = parser.read_csv(&csv).unwrap();
        df = parser.rename_df(df, &csv).unwrap();
        let columns = df.get_column_names_str();
        assert!(columns.contains(&"date"));
        assert!(columns.contains(&"classification"));
        assert!(columns.contains(&"amount"));
        assert!(columns.contains(&"desc"));
        assert!(columns.contains(&"partner_iban"));
    }

    #[test]
    fn test_semicolon_separator() {
        let yml = r#"
read_csv:
    separator: ";"
        "#;

        let config = serde_yml::from_str(yml).unwrap();
        let parser = BowParser::new(config, vec![]);
        let csv = get_test_data_folder().join("test_input_separator.csv");
        let df = parser.read_csv(&csv).unwrap();
        assert_eq!(df.get_column_names_str(), vec!["col1", "col2"]);
    }

    #[test]
    fn test_decimal_comma() {
        let yml = r#"
read_csv:
    separator: ";"
    decimal_comma: true
        "#;

        let config = serde_yml::from_str(yml).unwrap();
        let parser = BowParser::new(config, vec![]);
        let csv = get_test_data_folder().join("test_input_decimal_comma.csv");
        let df = parser.read_csv(&csv).unwrap();
        let col = df.column("col1").unwrap();
        assert_eq!(col.get(0).unwrap(), polars::prelude::AnyValue::Float64(1.5));
        assert_eq!(
            col.get(1).unwrap(),
            polars::prelude::AnyValue::Float64(-1.5)
        );
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
