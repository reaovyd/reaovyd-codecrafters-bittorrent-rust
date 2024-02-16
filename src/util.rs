use anyhow::Result;
use serde_bencode::{self, value::Value};
use serde_json::Value as SerdeJsonValue;

pub fn decode_bencoded_value(encoded_value: &str) -> Result<String> {
    let root_val = serde_bencode::from_str::<Value>(encoded_value)?;
    Ok(convert_value_to_string(root_val)?.to_string())
}

pub fn convert_value_to_string(val: Value) -> Result<SerdeJsonValue> {
    Ok(match val {
        Value::Bytes(bytes) => SerdeJsonValue::String(String::from_utf8(bytes)?),
        Value::Int(num) => SerdeJsonValue::Number(serde_json::Number::from(num)),
        Value::List(lst) => {
            let mut res = SerdeJsonValue::Array(Vec::new());
            for val in lst {
                let string = convert_value_to_string(val)?;
                res.as_array_mut().unwrap().push(string);
            }
            res
        }
        Value::Dict(dct) => {
            let mut map = SerdeJsonValue::Object(serde_json::Map::new());
            for (key, val) in dct {
                map.as_object_mut()
                    .unwrap()
                    .insert(String::from_utf8(key)?, convert_value_to_string(val)?);
            }
            map
        }
    })
}
