//! Zaman yardımcıları. Bütün zaman damgaları yerel saat diliminde ISO 8601
//! biçiminde üretilir.

use chrono::{DateTime, Local, TimeZone, Utc};

/// Şu anki zamanı ISO 8601 biçiminde (yerel saat) döndürür.
pub fn now_iso8601() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Şu anki zamanı Türkçe okunur biçimde döndürür.
pub fn now_readable() -> String {
    let now = Local::now();
    format!("{} {}", now.format("%d.%m.%Y"), now.format("%H:%M:%S"))
}

/// UNIX epoch saniyesini ISO 8601 (yerel) biçime çevirir.
pub fn epoch_to_iso8601(seconds: i64) -> String {
    match Local.timestamp_opt(seconds, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "-".to_string(),
    }
}

/// UNIX epoch saniyesini Türkçe okunur biçime çevirir.
pub fn epoch_to_readable(seconds: i64) -> String {
    match Local.timestamp_opt(seconds, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%d.%m.%Y %H:%M:%S").to_string(),
        _ => "-".to_string(),
    }
}

/// Sistem saat dilimi farkını (saniye) döndürür.
pub fn local_offset_seconds() -> i32 {
    Local::now().offset().local_minus_utc()
}

/// E01 header alanları için libewf uyumlu tarih biçimi (`2026 11 17 20 22 15`).
pub fn ewf_datetime(dt: DateTime<Local>) -> String {
    dt.format("%Y %m %d %H %M %S").to_string()
}

/// E01 için şu anki UTC saniyesini döndürür (libewf epoch kullanır).
pub fn now_epoch() -> i64 {
    Utc::now().timestamp()
}

/// `std::time::SystemTime` değerini okunur biçime çevirir.
pub fn format_system_time(t: &std::time::SystemTime) -> String {
    let dt: DateTime<Local> = (*t).into();
    dt.format("%d.%m.%Y %H:%M:%S").to_string()
}

/// `std::time::SystemTime` değerini ISO 8601 biçimine çevirir.
pub fn format_system_time_iso8601(t: &std::time::SystemTime) -> String {
    let dt: DateTime<Local> = (*t).into();
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Windows FILETIME (100ns, 1601'den beri) değerini okunur biçime çevirir.
pub fn filetime_to_readable(filetime: u64) -> String {
    if filetime == 0 {
        return "-".to_string();
    }
    // Windows epoch (1601-01-01) ile Unix epoch (1970-01-01) arası 11644473600 saniye.
    const EPOCH_DIFF: u64 = 11_644_473_600;
    let secs_since_1601 = filetime / 10_000_000;
    if secs_since_1601 < EPOCH_DIFF {
        return "-".to_string();
    }
    let unix_secs = (secs_since_1601 - EPOCH_DIFF) as i64;
    epoch_to_readable(unix_secs)
}

/// Windows FILETIME değerini ISO 8601 biçimine çevirir.
pub fn filetime_to_iso8601(filetime: u64) -> String {
    if filetime == 0 {
        return "-".to_string();
    }
    const EPOCH_DIFF: u64 = 11_644_473_600;
    let secs_since_1601 = filetime / 10_000_000;
    if secs_since_1601 < EPOCH_DIFF {
        return "-".to_string();
    }
    epoch_to_iso8601((secs_since_1601 - EPOCH_DIFF) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_epoch_conversion() {
        // 2024-01-01 00:00:00 UTC = 1704067200 Unix saniyesi
        // + 11644473600 = 13348540800 saniye -> 133485408000000000 FILETIME
        let ft = 133_485_408_000_000_000u64;
        let iso = filetime_to_iso8601(ft);
        assert!(iso.starts_with("2024-01-01"), "beklenmeyen: {iso}");
    }

    #[test]
    fn filetime_zero_is_dash() {
        assert_eq!(filetime_to_readable(0), "-");
    }

    #[test]
    fn ewf_datetime_format() {
        let dt = Local.with_ymd_and_hms(2026, 11, 17, 20, 22, 15).unwrap();
        assert_eq!(ewf_datetime(dt), "2026 11 17 20 22 15");
    }
}
