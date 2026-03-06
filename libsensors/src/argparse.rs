use crate::sspec::SensorConf;

pub trait SensorArgs {
    fn arg_str(&self, key: &str) -> Option<String>;
    fn arg_u64(&self, key: &str) -> Option<u64>;
    fn arg_bool(&self, key: &str) -> Option<bool>;
    fn arg_str_array(&self, key: &str) -> Option<Vec<String>>;
    fn arg_duration(&self, key: &str) -> Option<std::time::Duration>;
}

impl SensorArgs for SensorConf {
    fn arg_str(&self, key: &str) -> Option<String> {
        self.args().get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    }

    fn arg_u64(&self, key: &str) -> Option<u64> {
        self.args().get(key).and_then(|v| v.as_i64()).map(|i| i as u64)
    }

    fn arg_bool(&self, key: &str) -> Option<bool> {
        self.args().get(key).and_then(|v| v.as_bool())
    }

    fn arg_str_array(&self, key: &str) -> Option<Vec<String>> {
        self.args()
            .get(key)?
            .as_sequence()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string()).collect::<Vec<_>>())
            .filter(|v| !v.is_empty())
    }

    fn arg_duration(&self, key: &str) -> Option<std::time::Duration> {
        self.args().get(key).and_then(|v| v.as_str()).and_then(|s| humantime::parse_duration(s).ok())
    }
}
