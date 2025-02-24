use chrono::{DateTime, Datelike, Timelike, Utc};

pub fn format_iso8601(now: DateTime<Utc>) -> String {
    // We need to do manual formatting due to: https://github.com/chronotope/chrono/issues/94
    // See more about benchmarking: https://github.com/chronotope/chrono/pull/1155/files
    return format!(
        "{}-{:0>2}-{:0>2}T{:0>2}:{:0>2}:{:0>2}.{:0>3}Z",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.timestamp_subsec_millis()
    );
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn test_format_iso8601() {
        let now = Utc.with_ymd_and_hms(2025, 2, 25, 11, 2, 33)
            .unwrap()
            .with_nanosecond(456_000_000)
            .unwrap();

        assert_eq!(format_iso8601(now), "2025-02-25T11:02:33.456Z");
    }
}