from pathlib import Path
import os

import sys

sys.path.append(str(Path(__file__).parent.absolute()))

from parser import ConfigFileBasedParser
import polars as pl
from rules_applier import RulesApplier
from rules_parser import RulesParser
from rule import Rule
from analyzer import TransactionVisualizer
import yaml
import argparse

parser = argparse.ArgumentParser(description="Booking Organization Flow.")
parser.add_argument("-f", "--folder", help="folder to work in", default=".")


class Main:
    def __init__(self, working_dir: Path):
        self.working_dir = working_dir
        self.config = {}
        self.config_file = self.working_dir / "config.yml"
        if os.path.exists(self.config_file):
            print(f"Found config file in {self.config_file}")
            with open(self.config_file, encoding="utf-8") as file:
                self.config = yaml.load(file, Loader=yaml.FullLoader)
        else:
            print(f"No config file {self.config_file} found.")

        for folder in [
            "1_imports",
            Path("1_imports") / "bank",
            "2_rules",
            "3_manual",
            "4_output",
            "5_analysis",
        ]:
            (self.working_dir / folder).mkdir(exist_ok=True, parents=True)

    def _1_import(self):
        print("Importing transactions..")
        parsed = self._parse_transactions()
        if len(parsed) == 0:
            print("No transactions found, exiting.")
            sys.exit(0)
        combined_transactions = pl.concat(parsed.values())
        combined_transactions_enriched = self._enrich_transactions_with_amazon_data(
            combined_transactions
        )
        combined_transactions_enriched_with_balance_corrections = self._correct_balance(
            combined_transactions_enriched
        )
        return combined_transactions_enriched_with_balance_corrections

    def _parse_transactions(self):
        parsed = {}
        for folder in (self.working_dir / "1_imports" / "bank").iterdir():
            if folder.is_file():
                continue
            parsed[folder] = ConfigFileBasedParser(folder=folder).parse()

        return parsed

    def _enrich_transactions_with_amazon_data(
        self, combined_transactions: pl.DataFrame
    ):
        amazon_folder = self.working_dir / "1_imports" / "amazon"
        if not amazon_folder.exists():
            return combined_transactions

        amazon_all = (
            ConfigFileBasedParser(folder=self.working_dir / "1_imports" / "amazon")
            .parse()
            .group_by("amazon_order_id")
            .first()
        )

        combined_with_amazon_info = (
            combined_transactions.with_columns(
                amazon_order_id=pl.when(pl.col("partner").str.contains("AMAZON")).then(
                    pl.col("desc").str.split(" ").list.first()
                ),
            )
            .sort("date", descending=True)
            .join(
                amazon_all,
                on="amazon_order_id",
                how="left",
            )
            .with_columns(
                desc=pl.when(pl.col("desc_order").is_not_null())
                .then(pl.col("desc") + " amazon_product:" + pl.col("desc_order"))
                .otherwise(pl.col("desc"))
            )
        ).drop(
            "desc_order",
            "account_right",
            "amazon_order_id",
            "date_right",
        )

        return combined_with_amazon_info

    def _correct_balance(self, combined_transactions: pl.DataFrame):
        online_balances_file = self.working_dir / "1_imports" / "online_balances.csv"
        if not online_balances_file.exists():
            print(
                f"No file {online_balances_file} found, skipping balance correction. Consider creating one."
            )
            return combined_transactions

        daily_balances = (
            combined_transactions.sort("date", "account")
            .group_by("account", "date", maintain_order=True)
            .agg(amount=pl.col("amount").sum())
            .with_columns(
                balance=pl.cum_sum("amount").over(
                    "account", order_by="date", mapping_strategy="group_to_rows"
                )
            )
            .sort("account", "date")
        ).select("date", "account", "balance")

        real_balances = (
            pl.read_csv(
                online_balances_file,
                try_parse_dates=True,
                schema_overrides={"amount": pl.Float64},
            )
            .with_columns(
                desc=pl.lit("Balance correction according to online status"),
                account1=pl.lit("account:") + pl.col("account"),
                account2=pl.lit("balance_correction"),
            )
            .sort("date")
        )

        correction_transactions = (
            real_balances.join_asof(
                daily_balances, on="date", by="account", strategy="backward"
            )
            .sort("account", "date")
            .with_columns(agb=(pl.col("online_balance") - pl.col("balance")).round(2))
            .with_columns(last_agb=pl.col("agb").shift(1).over("account").fill_null(0))
            .with_columns(agb_final=pl.col("agb") - pl.col("last_agb"))
            .with_columns(
                amount=pl.col("agb_final"),
                partner=None,
                classification=None,
                partner_iban=None,
            )
            .select(combined_transactions.columns)
        ).filter(pl.col("amount").abs() > 0)

        transactions_corr = pl.concat(
            [combined_transactions, correction_transactions]
        ).sort("date")

        return transactions_corr

    def _2_rules(self, combined_transactions_enriched: pl.DataFrame):
        print("Applying rules..")
        rules: list[Rule] = RulesParser().parse(self.working_dir / "2_rules")
        categorized_transactions = RulesApplier(rules).apply(
            combined_transactions_enriched
        )
        return categorized_transactions

    def _3_manual(
        self,
        categorized_transactions: pl.DataFrame,
    ):
        uncategorized_pattern = self.config.get("3_manual", {}).get(
            "uncategorized_pattern", "unknown"
        )

        print(
            f"Applying manual categories, uncategorized pattern: {uncategorized_pattern}"
        )
        manual_category_file_name = "manual_categories.csv"

        if len(list((self.working_dir / "3_manual").glob("*.csv"))) == 0:
            man_categorized = categorized_transactions.filter(
                pl.col("account2").str.contains(uncategorized_pattern)
            )
            man_categorized.write_csv(
                self.working_dir / "3_manual" / manual_category_file_name
            )
            return categorized_transactions

        join_cols = [
            "date",
            "account",
            "partner",
            "desc",
            "classification",
            "partner_iban",
            "amount",
            "account1",
        ]

        man_categorized = pl.read_csv(
            self.working_dir / "3_manual" / manual_category_file_name,
            try_parse_dates=True,
            schema_overrides={
                "amount": pl.Float64,
                "date": pl.Date,
                "account": pl.String,
                "partner": pl.String,
                "desc": pl.String,
                "classification": pl.String,
                "partner_iban": pl.String,
                "account1": pl.String,
                "account2": pl.String,
            },
        ).filter(~pl.col("account2").str.contains(uncategorized_pattern))

        filtered = categorized_transactions.filter(
            pl.col("account2").str.contains(uncategorized_pattern)
        ).join(
            man_categorized,
            on=join_cols,
            join_nulls=True,
            how="anti",
        )

        new_man_data = pl.concat([man_categorized, filtered]).sort(
            ["date", "account", "amount"], descending=True
        )
        new_man_data.write_csv(
            self.working_dir / "3_manual" / manual_category_file_name
        )

        enriched_transactions = (
            categorized_transactions.join(
                man_categorized, on=join_cols, how="left", join_nulls=True
            )
            .with_columns(
                account2=pl.when(pl.col("account2_right").is_not_null())
                .then(pl.col("account2_right"))
                .otherwise(pl.col("account2"))
            )
            .drop("account2_right")
        ).sort("date", descending=False)

        return enriched_transactions

    def _4_output(self, enriched_transactions: pl.DataFrame):
        print("Writing output..")
        enriched_transactions.write_csv(self.working_dir / "4_output" / "output.csv")

        # hledger_output_columns = ["date", "desc", "amount", "account1", "account2"]
        # hledger_compatible_transactions = enriched_transactions.select(
        #     hledger_output_columns
        # )
        # hledger_compatible_transactions.write_csv(
        #     self.working_dir / "4_output" / "hledger/imported_hledger.csv"
        # )

    def _5_analyze(self, enriched_transactions: pl.DataFrame):
        print("Analyzing transactions..")
        plots_config = self.config.get("5_analysis", {}).get("plots", {})
        # print(f"Using plots config: {plots_config}")
        TransactionVisualizer(enriched_transactions, **plots_config).run(
            self.working_dir / "5_analysis"
        )

    def run(self):
        imported = self._1_import()
        categorized = self._2_rules(imported)
        manual = self._3_manual(categorized)

        self._4_output(manual)
        self._5_analyze(manual)

        return manual


def main():
    args = parser.parse_args()
    if args.folder == ".":
        args.folder = os.getcwd()
    Main(Path(args.folder)).run()


if __name__ == "__main__":
    main()
