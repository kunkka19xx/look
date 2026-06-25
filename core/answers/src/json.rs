//! Small typed accessors over `serde_json::Value` so the sources can read fields
//! without the repetitive `.get(key).and_then(|v| v.as_x())` dance. Each returns
//! `None` when the key is missing or the value isn't the requested type.

use serde_json::Value;

pub trait ValueExt {
    fn get_str(&self, key: &str) -> Option<&str>;
    fn get_f64(&self, key: &str) -> Option<f64>;
    fn get_i64(&self, key: &str) -> Option<i64>;
    fn get_arr(&self, key: &str) -> Option<&Vec<Value>>;
}

impl ValueExt for Value {
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }
    fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.as_f64()
    }
    fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_i64()
    }
    fn get_arr(&self, key: &str) -> Option<&Vec<Value>> {
        self.get(key)?.as_array()
    }
}
