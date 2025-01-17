# Todos

- [x] deutsche namen in parser-output ersetzen durch englische
- [x] ordnerstruktur erstellen wie folgt:
  - 1_imports
    - accounts
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
    - special
      - amazon.csv
    - online_balances.csv

    online_balances.csv soll in etwa so aussehen:

    date|account|online_balance
    -----|-----|----------
    2024-12-20|DKB|918.16
    2024-12-20|Erste Bank|403.68

  - 2_rules
  - 3_manual_categorization
  - 4_output
  - 5_analysis
- [x] Unbekannte transktionen ausgeben in 3_manual_categorization als csv (eventuell nach Jahren getrennt für übersicht), damit man händisch Kategorien vergeben kann. Wenn das geparst wird, 
  sollen alle händisch vergebenen Kategorien diejenigen überschreiben, die durch die "rules" davor vergeben wurden. Wenn sich die rules ändern und dementsprechend z.B. weniger unbekannte Kategorien vorliegen, sollen alle händisch vergebenen kategorien (welche, die nicht "unknown" oder "unbekannt" im namen haben, sollte durch config-file definiert werden können) verbleiben, aber die unkategorisierten Buchungen sollten aktualisiert werden
- [x] **ConfigFileBasedParser** schreiben, der eine Datei *parser_config.yml* im selben Ordner (z.B. DKB) liest und es überflüssig macht, für jede Bank individuellen Parser zu schreiben
- [x] Onlinekontostatus sollte mehrere Daten beinhalten können desselben Kontos (mit cum_diff machen), so dass diese Liste immer ergänzt werden kann ohne etwas zu löschen
- [x] die Art wie Regeln angewendet werden, soll effizienter werden. For-Schleife mit allen Regeln, und jede Regel soll über polars.filter angewendet werden und das endergebnis dann mit concat zusammengefasst werden
- [ ] CLI bauen um bookit ohne jupyter nutzbar zu machen
- [ ] alles umschreiben in Rust
