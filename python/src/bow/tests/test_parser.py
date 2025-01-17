from pathlib import Path
import sys
import polars as pl
from datetime import datetime

sys.path.append(str(Path(__file__).parent.parent))

from parser import Parser


class TestParser(Parser):
    def __init__(
        self,
        names_to_unique_konto_names: dict[str, str] = {},
        konto_names_to_iban: dict[str, str] = {},
    ):
        super().__init__(names_to_unique_konto_names, konto_names_to_iban)

    def parse_single_file(self, file: Path) -> pl.DataFrame:
        return pl.read_csv(file, try_parse_dates=True)


def test_overlapping_timeranges_same_account_are_filtered():
    parser = TestParser()
    folder = Path(__file__).parent / "test_files" / "unique_account"
    df = parser.parse(folder)
    assert len(df) == 33
    assert df.unique().shape[0] == 33
    assert df["datum"].min() == datetime(2001, 11, 30).date()
    assert df["datum"].max() == datetime(2003, 8, 25).date()


def test_overlapping_timeranges_different_accounts_are_not_filtered():
    parser = TestParser()
    folder = Path(__file__).parent / "test_files" / "multiple_accounts_complete_overlap"
    df = parser.parse(folder)
    assert len(df) == 46
    assert df.unique().shape[0] == 46
    assert df["datum"].min() == datetime(2001, 11, 30).date()
    assert df["datum"].max() == datetime(2003, 1, 3).date()
