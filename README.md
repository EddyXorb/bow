# BOW - bookings organization workflow

Bookings are meant to be bank transactions.

## Purpose

The general purpose is to get a better overview of your bank transactions for people who like
[plain text accounting](https://plaintextaccounting.org/). If you normally enter all your bank transactions into an excel table to categorize them by hand, this framework may help to reduce the categorization work using automatic rules.

## Working directory

You need a directory (e.g. "banking") that you pass to **bow**.
This will be the working directory of **bow**. It is structured as follows:

- banking 
  - config.yml
  - 1_imports 
    - online_balances.csv
    - bank 
      - bank1 
        - parser_config.yml
        - transaktions bank 1.csv
        - transaktinos bank 1 (part 2).csv
        - ...
      - bank2 
        - parser_config.yml
        - transaktions bank 2.csv
        - ...
      - ...
      - bankN 
        - parser_config.yaml
        - transaktions bank N.csv
        - ...
    - amazon 
      - order infos.csv -> *you will get these from amazon when you ask for your data; it takes a couple of days*
  - 2_rules 
    - income.yml
    - expenses.yml
    - ...
  - 3_manual 
    - manual_categories.csv
  - 4_output 
    - output.csv
  - 5_analysis 
    - plot 1.html
    - plot 2.html
    - ...

To get started, add your bank account-csv's to the imports/bank-folder and specify the parser_config.yml.
The structure of these files will be explained below.

In general, the workflow goes from top to bottom.
So if the program does not work as expected, try to solve the lowest number-step first.

## config.yml

Contains general settings, e.g. the plots can be configured (see [5_analysis](#5_analysis)) for that.
Example:

```yml
3_manual:
  uncategorized_pattern: ".*unknown.*|.*unbekannt.*"
5_analysis:
  plots:
    date_begin: 2020-01-01
    date_end: 2100-01-01
    account_pattern: ".*"
```

**uncategorized_pattern** : *string* that specifies, which categories should be seen as "uncategorized" somehow, which will then be treated as categories to manually specify. Be carefule with that, as **bow** will potentially remove all manually specified categories (see [3_manual](#3_manual)) that match this rule.

## 1_imports

There are two folders herein:

- **bank**
- **amazon** [optional]

**bank** contains all bank-accounts.
You can add your downloaded bank-account-csv's in subfolders of this.
It is not a problem to have overlapping timeframes of these csv's, as long as they contain the same transactions.
**bow** will take care to not import two times the same transactions.

In order for every bank-account-csv to be parsable, you need to specify a `parser_config.yml` for every subfolder of **bank**.

### Examples parser_config.yml for folder **bank**

#### DKB

```yml
read_csv:
  separator: ";"
  decimal_comma: True
  null_values:
    - ""
  encoding: "UTF-8"
  skip_rows: 4
rename:
  date: "Buchungsdatum"
  classification: "Umsatztyp"
  amount: "Betrag (€)"
  desc: "Verwendungszweck"
  partner_iban: "IBAN"
date_format: "%d.%m.%y"
account_settings:
  account_name: "DKB"
partner_settings:
  partner_column_if_amount_negative: "Zahlungsempfänger*in"
  partner_column_if_amount_positive: "Zahlungspflichtige*r"
row_filter:
  date_begin: 2022-01-01
```

#### N26

```yml
read_csv:
  separator: ","
  decimal_comma: False
  null_values:
    - ""
  encoding: "UTF-8"
  try_parse_dates: true
rename:
  date: "Booking Date"
  classification: "Type"
  amount: "Amount (EUR)"
  desc: "Payment Reference"
  partner_iban: "Partner Iban"
  account: "Account Name"
  partner: "Partner Name"
account_settings:
  account_aliases:
    "Hauptkonto": "N26 Hauptkonto"
    "Steuerrücklagen": "N26 Steuerrücklagen"
```

### Explanation parser_config.yml for **bank**

Basically, **read_csv** is the direct input to [polars.read_csv](https://docs.pola.rs/api/python/dev/reference/api/polars.read_csv.html).

**rename** will associate the columns of the input to the entities for *bank* accounts:

| entity         | description                                                  | optional for bank |
| -------------- | ------------------------------------------------------------ | ----------------- |
| account        | Name of the account                                          | No                |
| date           | The date of the transaction                                  | No                |
| amount         | The amount of money involved in the transaction              | No                |
| classification | The type of transaction (e.g. "income", but can be anything) | Yes               |
| desc           | Description or purpose of the transaction                    | Yes               |
| partner        | Name of the transaction partner                              | Yes               |
| partner_iban   | IBAN of the transaction partner                              | Yes               |

Any optional entity not specified will be `null`.

**pre_rename** takes place before rename and is optional with the following optional entries:
lower_columns: *bool* if true, will convert every column to lower case
strip_spaces: *bool* if true, will remove every space in every column (also within the column)

**date_format** : *string* specify your date-format column using the [standard python syntax](https://docs.python.org/3/library/datetime.html#format-codes).

**account_settings** is optional containing the following optional entries ( Will be applied in the end, after every other parser steps are done):

- account_aliases : *dict[str,str]* with names every entry in the account column should be replaced with. If a name is not in this dict, will not change the row.
- account_name : *string* with a fix account name the whole file should have.
- account_name_is_file_name : *bool* if true, will the the filename as the account name (without ".csv"-ending)

**partner_settings** is optional containing the following optional entries ( Will be applied in the end, after every other parser steps are done):

- partner_column_if_amount_negative: *string* pointing to the column which represents the "partner"-entity when the amount is less than 0.
- partner_column_if_amount_positive: *string* same as above, inverted.
- use_other_column_if_partner_empty: *bool* if **true** and one of the two partner_columns above is null, will take the other one, indipendently from the amount.

**row_filter**: optional restriction of rows with the following entries:

- date_begin: *date* of the first transaction included
- date_end: *date* of the first transaction not included

### Special folder **amazon**

The optional folder **amazon** contains transaction data from amazon. **bow** will try to add the product name to every transaction in the **bank**-accounts.
The parser_config.yml for this folder (at the moment) looks as follows:

```yml
expected_out_columns:
  - date
  - account
  - desc_order
  - amazon_order_id
read_csv:
  try_parse_dates: true
pre_rename:
  lower_columns: true
  strip_spaces: true
rename:
  date: "orderdate"
  desc_order: "productname"
  amazon_order_id: "orderid"
account_settings:
  account_name_is_file_name: true
```

**expected_out_columns** normally defaults to

```yml
- date
- account
- partner
- desc
- classification
- partner_iban
- amount
```

which are the columns we get after parsing a normal **bank**-account-csv.
The specification is needed here, as there is an internal check, that after parsing, every column needed is there.
For **amazon** we need the columns specified above.

### online_balance.csv

This optional file sits in the root of the 1_imports-folder.
It is expected to have this structure:

| date       | account        | online_balance |
| ---------- | -------------- | -------------- |
| 2020-01-20 | bank account 1 | 45.54          |
| 2024-12-20 | bank account 1 | 3244.54        |
| 2024-12-20 | bank account 2 | 405.99         |
| ...        | ...            | ...            |

If given, **bow** will create artificial transactions in the accounts specified,
if the balance differs between online_balance and calculatory
balance based on the given import-csv's.

## 2_rules

Having to deal with many transactions, one wants to categorize every one of it in order to get insights for what one spends money (or receives it).
Rules help to automatically categorize transactions.
It is common in plain text accounting software to name the category column "account2" (see [hledger](https://hledger.org/)).
Therefore this step will add two columns "account1" and "acccount2", where "account1" is basically the "account" of the row, only prepended by "account:", and
"account2" represents the target where the money of account1 goes to or comes from (which can be interpreted as a **category**).

The folder **2_rules** contains arbitrarily named (only the ending has to be .yml) rules-yml-files such as:

```yml
defaults:
  amount: "^[-].*"
rules:
  - category: expenses:amazon
    partner: ".*amazon.*"
  - category: expenses:bank
    base: ".*kartenpreis.*|.*depotgeb.*|.*fees.*"
  - category: ausgaben:bank
    base: ".*Abrechnung.*|.*entgelt.*"
    amount: ".*"
    account: ".*DKB.*"
    partner: "DKB AG"
  - category: ausgaben:baumarkt
    partner: ".*bauhaus.*"
```

A typical rules-folder may look like:

- 1_transactions_within_own_accounts.yml
- 2_income.yml
- 3_expenses.yml
- 4_fallback.yml

### Explanation general rule-file

In general, everything in a rule file has to be interpreted as regex, except the *category*, *date_begin* and *date_end* fields.

**defaults** : *string* optional regex that will be applied to every following rule in *rules*, unless not specified by the rule itself
**rules** : *dict* contains a list of rules. Every rule can have these entries:

| entity         | description                                                                                                           | example                                           | optional |
| -------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | -------- |
| category       | category that will be fiven to every transaction matching this rule                                                   | expenses:bank                                     | No       |
| case_sensitive | if false, ignores case in the following regexes, which is the default                                                 | false                                             | Yes      |
| date_begin     | The earliest date of the transaction                                                                                  | 2022-01-01                                        | Yes      |
| date_end       | The latest date of the transaction  (less than this date)                                                             | 2023-01-01                                        | Yes      |
| amount         | regex restricting amount of money involved in the transaction (useful to restrict to negative or big/small values)    | "^[-].*"                                          | Yes      |
| account        | regex restricting the account names                                                                                   | ".*DKB.*\|.*Sparkasse.*   "                       | Yes      |
| classification | regex restricting type of transaction (e.g. "income", but can be anything)                                            | ".*income.*"                                      | Yes      |
| desc           | regex restricting description or purpose of the transaction                                                           | "Grocery shopping"                                | Yes      |
| partner        | regex restricting name of the transaction partner                                                                     | "Amazon"                                          | Yes      |
| partner_iban   | regex restricting IBAN of the transaction partner                                                                     | "DE89370400440532013000"                          | Yes      |
| base           | regex matching any of account, classification, desc, partner, partner_iban. If it matches any of these, returns true. | ".*Grocery.*\|.*amazon.*\|DE89370400440532013000" | Yes      |

All fields specified will be connected with **AND**, such that if any field given does not match, the rule is not applied.

## 3_manual

Contains two files `todo.csv` and `done.csv`. They are automatically created, if not present.
`todo.csv` contains all transactions with unknown categorizatio, wheras `done.csv` those manually categorized.

Sometimes, you have to categorize by hand, because it would be to cumbersome to create new rules for every single-type-transaction.
In order to make this easy, the `todo.csv`'s "account2" column (which is the categorization column) can be manipulated.
Every category not containing "unknown" (or a pattern that can be specified in [config.yml](#configyml)) in its name, will then by taken as a user-defined categorization and will overwrite any rule-based categorization.

If you change your rules and there are less *unknown*-categories, the `todo.csv` will be updated to contain only the user-defined categories and the new uncategorized transactions after the rules have been applied.

## 4_output

Will write the combined and cleaned transactions to a file *output.csv*.


## 5_analysis

Will contain plots visualizing the balances and the categories over the years for all accounts.
Can be restricted to subsets of the data by specifying in the root directories `config.yml`
this:

```yml
5_analysis:
  plots:
    date_begin: 2020-01-01
    date_end: 2023-01-01
    account_pattern: ".*DKB.*"
```

### Hledger

The `output.csv` can be easily read by other plain-text-accounting software such as hledger. For that to work, create a `output.csv.rules` in *4_output* such as

```python
# skip the headings line:
skip 1

# use the first three CSV fields for hledger's transaction date, description and amount:
fields date,,,description,,,amount,account1,account2

# since the CSV amounts have no currency symbol, add one:
currency €
```

In order to create a hledger-journal directly in the *5_analysis*-folder, call 

```bash
hledger import -f .journal ../4_output/output.csv
```

from there. You can also create a file `hledger.conf` with

```bash
-f .journal
```

to avoid having to retype -f .journal all the time.
With a `hledger.directives`-file such as

```bash
account account         ; type: A
account ausgaben        ; type: L
account equity          ; type: E
account einnahmen       ; type: R
```
you can associate your manual-created categories to hledgers "assets","liabilities", "expenses" etc categories.
