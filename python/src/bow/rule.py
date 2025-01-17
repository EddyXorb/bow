import re
from dataclasses import dataclass
from datetime import datetime
import polars as pl


@dataclass
class Rule:
    """
    All patterns are case insensitive.+, unless otherwise specified through the case_sensitive flag.

    All parts are connected with AND.

    The category_pattern refers to the category field of the transaction.

    base_pattern is applied to every string field of the transaction (account, partner, partner_iban, desc, category), and connected with OR (so one match is enough).
    """

    category: str
    name: str = None

    case_sensitive: bool = False
    date: datetime | None = None
    date_start = datetime.min.date()
    date_end = datetime.max.date()
    amount: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    base: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    account: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    desc: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    partner: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    partner_iban: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)
    classification: re.Pattern = re.compile(r".*", flags=re.IGNORECASE)

    def __init__(
        self,
        category: str,
        name: str = None,
        date: datetime | None = None,
        date_start: datetime = datetime.min.date(),
        date_end: datetime = datetime.max.date(),
        case_sensitive: bool = False,
        amount: str = ".*",
        base: str = ".*",
        account: str = ".*",
        desc: str = ".*",
        partner: str = ".*",
        partner_iban: str = ".*",
        classification: str = ".*",
    ):
        flags = re.NOFLAG if case_sensitive else re.IGNORECASE

        self.category = category
        self.name = name
        self.date = date
        self.date_start = date_start
        self.date_end = date_end
        self.case_sensitive = case_sensitive
        self.amount = re.compile(amount, flags=flags)
        self.base = re.compile(base, flags=flags)
        self.account = re.compile(account, flags=flags)
        self.desc = re.compile(desc, flags=flags)
        self.partner = re.compile(partner, flags=flags)
        self.partner_iban = re.compile(partner_iban, flags=flags)
        self.classification = re.compile(classification, flags=flags)

    def __str__(self):
        return self.name if self.name else self.category

    def matches(
        self,
        date: datetime,
        amount: float,
        account: str | None,
        desc: str | None,
        partner: str | None,
        partner_iban: str | None,
        classification: str | None,
    ) -> bool:
        """
        Legacy. Use filter_dataframe instead.
        """
        try:
            if self.date and date != self.date:
                return False
            if date < self.date_start or date > self.date_end:
                return False
            if not self.amount.match(str(amount)):
                return False

            patterns = {
                "account": (account, self.account),
                "desc": (desc, self.desc),
                "partner": (partner, self.partner),
                "partner_iban": (partner_iban, self.partner_iban),
                "classification": (classification, self.classification),
            }

            for _, (field, matcher) in patterns.items():
                if not field and matcher.pattern != ".*":
                    return False
                if field and matcher:
                    if not matcher.match(field):
                        return False

            if not any(
                [field and self.base.match(field) for field, _ in patterns.values()]
            ):
                return False

            return True

        except Exception as e:
            print(f"Error in rule {self}: {e}")
            return False

    def filter_dataframe(self, df: pl.DataFrame) -> pl.DataFrame:
        case_insensitive_flag = "(?i)" if not self.case_sensitive else ""
        patterns = {
            "account": self.account,
            "desc": self.desc,
            "partner": self.partner,
            "partner_iban": self.partner_iban,
            "classification": self.classification,
        }

        single_pattern_filter = pl.lit(True)
        for field, matcher in patterns.items():
            if matcher.pattern == ".*":
                continue
            single_pattern_filter &= pl.col(field).is_not_null() & pl.col(
                field
            ).str.contains(f"{case_insensitive_flag}{matcher.pattern}")

        base_pattern_filter = (
            pl.lit(True) if self.base.pattern == ".*" else pl.lit(False)
        )
        if self.base.pattern != ".*":
            for field, _ in patterns.items():
                base_pattern_filter |= pl.col(field).is_not_null() & pl.col(
                    field
                ).str.contains(f"{case_insensitive_flag}{self.base.pattern}")

        pattern_filter = single_pattern_filter & base_pattern_filter

        return df.filter(
            (pl.col("date") >= self.date_start)
            & (pl.col("date") <= self.date_end)
            & pl.col("amount")
            .cast(pl.String)
            .str.contains(f"{case_insensitive_flag}{self.amount.pattern}")
            & pattern_filter
        )
