use opentelemetry_proto::tonic::common::v1::any_value::Value as OtelValue;
use base64::Engine;

pub fn map_otel_value_to_serdejson_value(value: Option<OtelValue>) -> serde_json::Value {
    if let Some(v) = value {
        return match v {
            OtelValue::StringValue(s) => serde_json::Value::String(s),
            OtelValue::BoolValue(b) => serde_json::Value::Bool(b),
            OtelValue::IntValue(i) => serde_json::Value::Number(serde_json::Number::from(i)),
            OtelValue::DoubleValue(d) => serde_json::Value::Number(serde_json::Number::from_f64(d).unwrap()),
            OtelValue::ArrayValue(a) =>
                serde_json::Value::Array(
                    a.values
                        .into_iter()
                        .map(|v| map_otel_value_to_serdejson_value(v.value))
                        .collect()
                ),
                OtelValue::KvlistValue(kv) => {
                let mut map = serde_json::Map::new();
                for entry in kv.values {
                    if let Some(vv) = entry.value {
                        map.insert(entry.key, map_otel_value_to_serdejson_value(vv.value));
                    } else {
                        map.insert(entry.key, serde_json::Value::Null);
                    }
                }
                serde_json::Value::Object(map)
            },
            OtelValue::BytesValue(b) => serde_json::Value::String(base64::engine::general_purpose::URL_SAFE.encode(b))
        };
    }
    return serde_json::Value::Null;
}