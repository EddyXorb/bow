from rule import Rule
from collections import defaultdict
import polars as pl
from parser import bank_transaction_columns


# if this gets too slow, we could try to use filter upon the main dataframe, going through every rule one after the other an building a new dataframe this way
class RulesApplier:
    def __init__(self, rules: list[Rule]):
        self.rules = rules
        self.rule_to_count = defaultdict(int)

    def categorize_account2(self, row: dict[str, str]) -> str:
        for rule in self.rules:
            if rule.matches(
                date=row["date"],
                amount=row["amount"],
                account=row["account"],
                desc=row["desc"],
                partner=row["partner"],
                partner_iban=row["partner_iban"],
                classification=row["classification"],
            ):
                self.rule_to_count[str(rule)] += 1
                return rule.category

        return "unknown"

    def apply(self, data: pl.DataFrame) -> pl.DataFrame:
        """
        This is faster than the legacy apply method. Use this.
        """
        data_rest = data.clone()
        data_rest = data_rest.with_columns(
            account1=pl.lit("account:") + pl.col("account"), account2=pl.lit("unknown")
        )
        new_data = []
        for rule in self.rules:
            filtered = rule.filter_dataframe(data_rest).with_columns(
                account2=pl.lit(rule.category)
            )
            data_rest = data_rest.join(
                filtered,
                on=[
                    "date",
                    "account",
                    "partner",
                    "desc",
                    "classification",
                    "partner_iban",
                    "amount",
                    "account1",
                ],
                how="anti",
                join_nulls=True,
            )
            new_data.append(filtered)
            if filtered.shape[0] > 0:
                pass
                # print(f"Rule '{rule}' matched {filtered.shape[0]} times")

        result = pl.concat(new_data + [data_rest])
        # print(f"Data rest has {data_rest.shape[0]} rows.")
        return result

    def apply_legacy(self, data: pl.DataFrame) -> pl.DataFrame:
        if not set(bank_transaction_columns).issubset(data.columns):
            raise ValueError(
                f"Dataframe does not contain the expected columns {bank_transaction_columns}"
            )
        data_ext = data.with_columns(
            account1=pl.lit("account:") + pl.col("account"),
            account2=pl.struct(
                "date",
                "account",
                "partner",
                "desc",
                "amount",
                "partner_iban",
                "classification",
            ).map_elements(
                self.categorize_account2,
                return_dtype=pl.String,
                pass_name=True,
            ),
        ).sort("date")

        print(
            f"Uncategorized: {data_ext.filter(pl.col("account2").str.contains("unknown")).shape[0]} rows."
        )
        ordered_matches = sorted(
            self.rule_to_count.items(), key=lambda x: x[1], reverse=True
        )
        for rule, count in ordered_matches:
            print(f"Rule '{rule}' matched {count} times")

        return data_ext
