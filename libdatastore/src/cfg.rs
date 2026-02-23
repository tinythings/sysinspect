use std::{io, time::Duration};

#[derive(Debug, Clone)]
pub struct DataStorageConfig {
    expiration: Option<Duration>,
    max_item_size: Option<u64>,
    max_overall_size: Option<u64>,
}

impl Default for DataStorageConfig {
    fn default() -> Self {
        Self { expiration: None, max_item_size: None, max_overall_size: None }
    }
}

impl DataStorageConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Examples: "3 days", "12h", "90 min", "30s"
    pub fn expiration(mut self, s: &str) -> Result<Self, io::Error> {
        self.expiration = Some(parse_duration(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?);
        Ok(self)
    }

    /// Examples: "1 gb", "512mb", "42k", "100 bytes"
    pub fn max_item_size(mut self, s: &str) -> Result<Self, io::Error> {
        self.max_item_size = Some(parse_size(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?);
        Ok(self)
    }

    pub fn max_overall_size(mut self, s: &str) -> Result<Self, io::Error> {
        self.max_overall_size = Some(parse_size(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?);
        Ok(self)
    }

    pub fn get_max_overall_size(&self) -> Option<u64> {
        self.max_overall_size
    }

    pub fn get_max_item_size(&self) -> Option<u64> {
        self.max_item_size
    }

    pub fn get_expiration(&self) -> Option<Duration> {
        self.expiration
    }
}

fn parse_size(input: &str) -> Result<u64, String> {
    let s = input.trim().to_lowercase().replace(' ', "");
    let (num, unit) = split_num_unit(&s)?;

    let mult: u64 = match unit.as_str() {
        "" | "b" | "byte" | "bytes" => 1,
        "k" | "kb" => 1024,
        "m" | "mb" => 1024_u64.pow(2),
        "g" | "gb" => 1024_u64.pow(3),
        "t" | "tb" => 1024_u64.pow(4),
        _ => return Err(format!("unknown size unit: '{unit}'")),
    };

    num.checked_mul(mult).ok_or("size overflow".into())
}

fn split_num_unit(s: &str) -> Result<(u64, String), String> {
    let mut digits = String::new();
    let mut unit = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            if !unit.is_empty() {
                return Err(format!("bad format: '{s}'"));
            }
            digits.push(c);
        } else if c.is_ascii_whitespace() {
            continue;
        } else {
            unit.push(c);
        }
    }
    if digits.is_empty() {
        return Err(format!("missing number in '{s}'"));
    }
    let num = digits.parse::<u64>().map_err(|_| format!("bad number in '{s}'"))?;
    Ok((num, unit))
}

fn parse_duration(input: &str) -> Result<Duration, String> {
    let s = input.trim().to_lowercase();
    // accept: "3 days", "12h", "90 min", "30s"
    let (num, unit) = split_num_unit(&s)?;

    let secs = match unit.as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => num,
        "m" | "min" | "mins" | "minute" | "minutes" => num * 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => num * 60 * 60,
        "d" | "day" | "days" => num * 60 * 60 * 24,
        _ => return Err(format!("unknown duration unit: '{unit}'")),
    };

    Ok(Duration::from_secs(secs))
}
