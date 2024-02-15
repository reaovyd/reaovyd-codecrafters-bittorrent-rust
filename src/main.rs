// Available if you need it!
use anyhow::Result;
use serde_bencode::{self, value::Value};
use std::{collections::HashMap, env};
use thiserror::Error;

// #[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> Result<String> {
    let root_val = serde_bencode::from_str::<Value>(encoded_value)?;
    convert_value_to_string(root_val)
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        if let Ok(decoded_value) = decoded_value {
            println!("{}", decoded_value);
        } else {
            eprintln!("Failed!");
        }
    } else {
        println!("unknown command: {}", args[1])
    }
}

fn convert_value_to_string(val: Value) -> Result<String> {
    Ok(match val {
        Value::Bytes(bytes) => {
            format!("\"{}\"", String::from_utf8(bytes)?)
        }
        Value::Int(num) => {
            format!("{}", num)
        }
        Value::List(lst) => {
            let mut res = Vec::new();
            for val in lst {
                let string = convert_value_to_string(val)?;
                res.push(string);
            }
            format!("{:?}", res)
        }
        Value::Dict(dct) => {
            let mut map = HashMap::new();
            for (key, val) in dct {
                map.insert(key, convert_value_to_string(val)?);
            }
            format!("{:?}", map)
        }
    })
}

// impl TryFrom<Value> for String {
//     type Error = TorrentError;
//     fn try_from(value: Value) -> Result<Self, Self::Error> {
//         match value {
//             Value::Bytes(bytes) => {
//                 format!("\"{}\"", String::from_utf8(bytes).unwrap())
//             }
//             Value::Int(_) => todo!(),
//             Value::List(_) => todo!(),
//             Value::Dict(_) => todo!(),
//         }
//     }
// }
//
// #[derive(Error)]
// enum TorrentError {
//     #[error("Error converting the value!")]
//     ConversionError,
// }
