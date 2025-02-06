use serde_yml;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn read_yaml_file(config_file: &PathBuf) -> Option<HashMap<String, serde_yml::Value>> {
    if !config_file.exists() {
        println!("No config file found in {:}", config_file.display());
        return None;
    }

    println!("Found config file in {:}", config_file.display());
    let file = fs::File::open(config_file).expect(
        "yaml file {config_file:?} should be readable. Check if you have the right permissions.",
    );
    serde_yml::from_reader(file).expect(&format!(
        "yaml file {config_file:?} should be parseable. Check if it is valid yaml."
    ))
}
