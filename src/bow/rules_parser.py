import yaml
from pathlib import Path
from rule import Rule


class RulesParser:
    def parse(self, rules_folder: Path) -> list[Rule]:
        rules = []
        for yaml_file in sorted(rules_folder.glob("*.yml")):
            rules_raw, defaults = self._read_single_rule_file(yaml_file)
            rules_of_single_file = self._parse_rules_of_single_file(rules_raw, defaults)
            rules += rules_of_single_file

            # print(f"Loaded {len(rules_of_single_file)} rules from {yaml_file}.")

        print(f"    Loaded {len(rules)} rules in total")

        return rules

    def _parse_rules_of_single_file(self, rules_raw, defaults):
        rules_single_file = []
        for rule_raw in rules_raw:
            base = {}
            base.update(defaults)
            base.update(rule_raw)
            rules_single_file.append(Rule(**rule_raw))

        return rules_single_file

    def _read_single_rule_file(self, yaml_file):
        with open(yaml_file, mode="r", encoding="utf-8") as file:
            rules_dict = yaml.load(file, Loader=yaml.FullLoader)

        rules_raw: list[dict[str, str]] = (
            rules_dict["rules"] if "rules" in rules_dict and rules_dict["rules"] else []
        )
        defaults = (
            rules_dict["defaults"]
            if "defaults" in rules_dict and rules_dict["defaults"]
            else {}
        )

        if type(defaults) is not dict:
            raise ValueError(f"defaults must be a dict, not {type(defaults)}")
        if type(rules_raw) is not list:
            raise ValueError(f"rules must be a list, not {type(rules_raw)}")
        return rules_raw, defaults
