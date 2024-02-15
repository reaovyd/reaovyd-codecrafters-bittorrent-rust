use std::env;

// Available if you need it!
use anyhow::Result;
use serde_bencode::{self, value::Value};

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> Result<Value> {
    Ok(serde_bencode::from_str::<Value>(encoded_value)?)
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        if let Ok(decoded_value) = decoded_value {
            let res = match decoded_value {
                Value::Bytes(bytes) => String::from_utf8(bytes).unwrap(),
                Value::Int(num) => {
                    format!("{}", num)
                }
                Value::List(_val) => "brih".to_owned(),
                Value::Dict(_map) => "map".to_owned(),
            };
            println!("\"{}\"", res);
        } else {
            eprintln!("Failed!");
        }
    } else {
        println!("unknown command: {}", args[1])
    }
}
