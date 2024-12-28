from pathlib import Path
import polars as pl
import altair as alt
from datetime import datetime


class TransactionVisualizer:
    def __init__(
        self,
        transactions: pl.DataFrame,
        date_begin: datetime = datetime.min,
        date_end: datetime = datetime.max,
        account_pattern=".*",
    ):
        alt.data_transformers.enable("vegafusion")
        self.transactions = transactions.sort("date")
        self.date_begin = date_begin
        self.date_end = date_end
        self.accounts_pattern = account_pattern

        self.data_filter = (
            pl.col("date") >= self.date_begin,
            pl.col("date") < self.date_end,
            pl.col("account").str.contains(self.accounts_pattern),
        )

    def run(self, target_dir: Path):
        self.get_combined_plots().save(
            target_dir
            / f"plot from {datetime.now().date()}, years {self.date_begin.year}-{self.date_end.year}, {self.transactions.filter(self.data_filter)["account"].unique().shape[0]} accounts.html"
        )

    def get_accountwise_balances_plot(self):
        daily_balances_final = (
            self.transactions.with_columns(
                stand=pl.cum_sum("amount").over("account1"),
                date=pl.col("date").cast(pl.Datetime),
            )
            .sort("account1", "date")
            .filter(self.data_filter)
        )

        return (
            alt.Chart(
                daily_balances_final,
                width=300,
                title=alt.Title(
                    "Balances over time for all accounts", anchor="middle", fontSize=25
                ),
            )
            .mark_line()
            .encode(
                x="date:T",
                y="stand",
                facet=alt.Facet(
                    "account1", columns=4, header=alt.Header(labelFontSize=15)
                ),
                tooltip=["date", "account1", "stand"],
            )
            .resolve_scale(y="independent")
            .interactive()
        )

    def get_overall_balance_plot(self):
        daily_balances_all_my_accounts = (
            self.transactions.with_columns(
                stand=pl.cum_sum("amount"),
                date=pl.col("date").cast(pl.Datetime),
            )
            .sort("date")
            .filter(self.data_filter)
        )

        return (
            alt.Chart(
                daily_balances_all_my_accounts,
                width=1200,
                title=alt.Title("Gesamtvermögen", anchor="middle", fontSize=25),
            )
            .mark_line()
            .encode(
                x="date:T",
                y="stand",
                tooltip=["date", "stand"],
            )
            .interactive()
        )

    def get_yearly_category_plot(self, accounts: list[str], indipendent_scale=True):
        daily_balances_categories = (
            self.transactions.filter(self.data_filter)
            .filter(pl.col("account").is_in(accounts))
            .with_columns(year=pl.col("date").dt.year())
            .group_by("year", "account2")
            .agg(stand=pl.sum("amount").abs().round(2))
            .sort("account2", "year")
        )
        plot = (
            alt.Chart(
                daily_balances_categories,
                width=175,
                height=125,
                title=alt.Title(
                    "Yearly category balances for " + ", ".join(accounts),
                    anchor="middle",
                    fontSize=25,
                ),
            )
            .mark_bar()
            .encode(
                x="year:O",
                y="stand",
                facet=alt.Facet(
                    "account2", columns=5, header=alt.Header(labelFontSize=15)
                ),
                color="account2",
                tooltip=["year", "stand"],
            )
        )
        if indipendent_scale:
            plot = plot.resolve_scale(y="independent")
        return plot

    def get_combined_plots(self):
        bank_accounts = (
            self.transactions.filter(self.data_filter)["account"].unique().to_list()
        )

        plots = [
            self.get_overall_balance_plot(),
            self.get_accountwise_balances_plot(),
            self.get_yearly_category_plot(accounts=bank_accounts),
        ]
        for account in bank_accounts:
            plots.append(self.get_yearly_category_plot([account]))

        return alt.vconcat(*plots)
