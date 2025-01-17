from typing import defaultdict
import polars as pl
from datetime import datetime
from pathlib import Path

import yaml

bank_transaction_columns = [
    "date",
    "account",
    "partner",
    "desc",
    "classification",
    "partner_iban",
    "amount",
]


class Parser:
    def __init__(
        self,
        folder: Path,
        expected_out_columns=bank_transaction_columns,
    ):
        self.folder = folder
        self.expected_out_columns = expected_out_columns

    def parse(self) -> pl.DataFrame | None:
        files = list(self.folder.glob("*.csv"))
        if len(files) == 0:
            raise FileNotFoundError(f"No files found in {self.folder}")

        account_to_timeranges_already_parsed: defaultdict[
            str, list[(datetime, datetime)]
        ] = defaultdict(list)

        dfs = []
        for file in files:
            print(f"    Parsing {file.relative_to(self.folder.parent.parent)}..")
            df = self.parse_single_file(file)
            assert df.columns == self.expected_out_columns
            for account, timeranges in account_to_timeranges_already_parsed.items():
                for start, end in timeranges:
                    df = df.filter(
                        (pl.col("account") != account)
                        | (pl.col("date") < start)
                        | (pl.col("date") > end)
                    )
            timeframe_parsed = df.group_by("account").agg(
                min_time=pl.min("date"), max_time=pl.max("date")
            )
            for row in timeframe_parsed.iter_rows(named=True):
                account = row["account"]
                min_time = row["min_time"]
                max_time = row["max_time"]
                account_to_timeranges_already_parsed[account].append(
                    (min_time, max_time)
                )

            dfs.append(df)

        df = pl.concat(dfs)

        if "partner_iban" in df.columns:
            df = df.with_columns(
                partner_iban=pl.when(pl.col("partner_iban").is_not_null())
                .then(pl.col("partner_iban").cast(pl.String).str.replace_all(" ", ""))
                .otherwise(pl.col("partner_iban"))
            )
        df = df.select(self.expected_out_columns)
        return df

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        raise NotImplementedError()


class ConfigFileBasedParser(Parser):
    def __init__(self, folder: Path):
        super().__init__(folder)

        self.parse_config_file = folder / "parser_config.yml"
        if not self.parse_config_file.exists():
            raise FileNotFoundError(f"No config file found in {folder}")

        with open(self.parse_config_file, encoding="UTF-8") as file:
            self.config: dict[str, str] = yaml.load(file, Loader=yaml.FullLoader)

        if "expected_out_columns" in self.config:
            self.expected_out_columns = self.config["expected_out_columns"]

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        df = pl.read_csv(file, **self.config.get("read_csv", {}))

        if self.config.get("pre_rename", {}).get("lower_columns", False):
            df = df.rename({col: col.lower() for col in df.columns})
        if self.config.get("pre_rename", {}).get("strip_spaces", False):
            df = df.rename({col: col.replace(" ", "") for col in df.columns})

        rename_dict = {
            value: key for key, value in self.config.get("rename", {}).items()
        }

        df: pl.DataFrame = df.rename(rename_dict)
        if "amount" in df.columns:
            if df.dtypes[df.columns.index("amount")] == pl.String:
                df = df.with_columns(
                    pl.col("amount")
                    .cast(pl.String)
                    .str.replace(r"\.", "")
                    .str.replace(r",", ".")
                    .fill_null(0)
                    .cast(pl.Float64)
                )
            df = df.with_columns(amount=pl.col("amount").cast(pl.Float64))

        if "date_format" in self.config:
            df = df.with_columns(
                date=pl.col("date")
                .str.to_datetime(self.config["date_format"])
                .cast(pl.Date)
            )

        if partner_settings := self.config.get("partner_settings", None):
            if (
                "amount" in df.columns
                and "partner_column_if_amount_negative" in partner_settings
                and "partner_column_if_amount_positive" in partner_settings
            ):
                when_condition = pl.col("amount") < 0
                if partner_settings.get("use_other_column_if_partner_empty", False):
                    when_condition = (
                        when_condition
                        & pl.col(
                            partner_settings["partner_column_if_amount_negative"]
                        ).is_not_null()
                    ) | (
                        pl.col(
                            partner_settings["partner_column_if_amount_positive"]
                        ).is_null()
                    )

                df = df.with_columns(
                    partner=pl.when(when_condition)
                    .then(pl.col(partner_settings["partner_column_if_amount_negative"]))
                    .otherwise(
                        pl.col(partner_settings["partner_column_if_amount_positive"])
                    )
                )

        if account_settings := self.config.get("account_settings", None):
            if "account_name" in account_settings:
                df = df.with_columns(
                    account=pl.lit(self.config["account_settings"]["account_name"])
                )
            elif account_settings.get("account_name_is_file_name", False):
                df = df.with_columns(account=pl.lit(file.stem))

            if "account_aliases" in account_settings:
                df = df.with_columns(
                    account=pl.col("account").replace(
                        account_settings["account_aliases"]
                    )
                )

        for col in self.expected_out_columns:
            if col not in df.columns:
                df = df.with_columns(pl.lit(None).alias(col))

        if row_filter := self.config.get("row_filter", None):
            if "date_begin" in row_filter:
                df = df.filter(pl.col("date") >= row_filter["date_begin"])
            if "date_end" in row_filter:
                df = df.filter(pl.col("date") < row_filter["date_end"])

        df = df.filter(pl.col("account").is_not_null()).select(
            self.expected_out_columns
        )
        return df


class FinanzmanagerParser(Parser):
    def __init__(self, folder: Path):
        super().__init__(folder=folder)

    def parse_single_file_raw(self, file: Path) -> pl.DataFrame:
        buchungen_finanzmanager_raw = pl.read_csv(
            file,
            separator=";",
            decimal_comma=True,
            schema_overrides={
                "Haben": pl.String,
                "Soll": pl.String,
                "Beleg": pl.String,
            },
            try_parse_dates=True,
            null_values=[""],
            encoding="UTF-16",
        )
        return buchungen_finanzmanager_raw

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        buchungen_finanzmanager = self.parse_single_file_raw(file)
        buchungen_finanzmanager = (
            (
                buchungen_finanzmanager.rename(
                    {col: col.lower() for col in buchungen_finanzmanager.columns}
                )
                .rename(
                    {
                        "buchungstag": "date",
                        "verwendungszweck": "desc",
                        "iban/kto-nr. auftragg.": "partner_iban",
                        "name auftragg.": "partner",
                        "betrag": "amount",
                        "konto": "account",
                        "kategorie": "classification",
                    }
                )
                .with_columns(
                    pl.col("amount")
                    .str.replace(r"\.", "")
                    .str.replace(r",", ".")
                    .fill_null(0)
                    .cast(pl.Float64),
                    pl.col("partner_iban").str.replace_all(" ", ""),
                )
                .with_columns(
                    partner=pl.when(
                        (pl.col("amount") < 0) | (pl.col("partner").is_null())
                    )
                    .then(pl.col("empfänger"))
                    .otherwise(pl.col("partner"))
                )
            )
            .filter(pl.col("account").is_not_null())
            .select(self.expected_out_columns)
        )
        return buchungen_finanzmanager


class DKBParser(Parser):
    def __init__(self, folder: Path, names_to_unique_account_names: dict[str, str]):
        super().__init__(folder, names_to_unique_account_names)

    def parse_single_file_raw(self, file: Path) -> pl.DataFrame:
        return pl.read_csv(
            file,
            separator=";",
            decimal_comma=True,
            schema_overrides={
                "Haben": pl.String,
                "Soll": pl.String,
                "Betrag (€)": pl.String,
            },
            try_parse_dates=False,
            null_values=[""],
            encoding="UTF-8",
            skip_rows=4,
        )

    def get_account_name(self, file: Path) -> str:
        with open(file, "r", encoding="UTF-8") as file:
            first_line = file.readline()
            account_description = (
                first_line.split(";")[-1].replace("\n", "").replace('"', "").strip()
            )
        return self.names_to_unique_account_names[account_description]

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        account_name = self.get_account_name(file)

        buchungen_dkb = self.parse_single_file_raw(file)
        buchungen_dkb = buchungen_dkb.rename(
            {col: col.lower() for col in buchungen_dkb.columns}
        )
        rename_dict = {
            "buchungsdatum": "date",
            "belegdatum": "date",
            "beschreibung": "desc",
            "umsatztyp": "classification",
            "betrag (€)": "amount",
            "verwendungszweck": "desc",
            "iban": "partner_iban",
        }
        rename_dict = {
            key: value
            for key, value in rename_dict.items()
            if key
            in buchungen_dkb.columns  # the VISA csv has different column names therefore we need this robustness
        }
        buchungen_dkb = buchungen_dkb.rename(rename_dict).with_columns(
            pl.col("amount")
            .str.replace(r"\.", "")
            .str.replace(r",", ".")
            .cast(pl.Float64)
            .fill_null(0),
            account=pl.lit(account_name),
            date=pl.col("date").str.to_datetime("%d.%m.%y").cast(pl.Date),
        )
        if "zahlungspflichtige*r" in buchungen_dkb.columns:
            buchungen_dkb = buchungen_dkb.with_columns(
                partner=pl.when(pl.col("classification") == "Eingang")
                .then(pl.col("zahlungspflichtige*r"))
                .otherwise(pl.col("zahlungsempfänger*in"))
            )
        else:
            buchungen_dkb = buchungen_dkb.with_columns(
                partner=pl.lit(None), partner_iban=pl.lit(None)
            )
        buchungen_dkb = buchungen_dkb.select(self.expected_out_columns)
        return buchungen_dkb


class N26Parser(Parser):
    def __init__(self, folder, names_to_unique_account_names):
        super().__init__(folder)

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        buchungen_n26 = pl.read_csv(
            file,
            separator=",",
            decimal_comma=False,
            try_parse_dates=True,
            null_values=[""],
            encoding="UTF-8",
        )

        buchungen_n26 = (
            buchungen_n26.rename({col: col.lower() for col in buchungen_n26.columns})
            .rename(
                {
                    "booking date": "date",
                    "partner name": "partner",
                    "partner iban": "partner_iban",
                    "type": "classification",
                    "payment reference": "desc",
                    "amount (eur)": "amount",
                }
            )
            .with_columns(
                pl.col("amount").cast(pl.Float64),
                account=pl.lit("N26 ") + pl.col("account name"),
            )
            .select(self.expected_out_columns)
        )
        return buchungen_n26


class AmazonParser(Parser):
    def __init__(self, folder):
        super().__init__(
            folder=folder,
            expected_out_columns=[
                "date",
                "account",
                "desc_order",
                "amazon_order_id",
                "amazon_asin",
            ],
        )

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        df = pl.read_csv(file, try_parse_dates=True)
        orders = (
            df.rename({col: col.lower().replace(" ", "") for col in df.columns})
            .rename(
                {
                    "orderdate": "date",
                    "productname": "desc_order",
                    "orderid": "amazon_order_id",
                    "asin": "amazon_asin",
                }
            )
            .with_columns(
                pl.col("date").cast(pl.Date),
                desc_order="amazon-product:" + pl.col("desc_order"),
                account=pl.lit(file.stem),
            )
            .select(self.expected_out_columns)
        )

        return orders
