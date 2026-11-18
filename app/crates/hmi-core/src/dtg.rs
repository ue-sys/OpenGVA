//! The date-time group: at the left of the status information bar, and
//! wherever else the HMI says when.

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// `DDHHMMZ MON YY` in Zulu time, e.g. `041230Z OCT 24`.
///
/// Time zones other than Zulu are not handled yet.
pub fn date_time_group(unix_s: u64) -> String {
    let days = (unix_s / 86_400) as i64;
    let secs = unix_s % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{:02}{:02}{:02}Z {} {:02}",
        day,
        secs / 3_600,
        secs % 3_600 / 60,
        MONTHS[(month - 1) as usize],
        year.rem_euclid(100)
    )
}

/// `DDHHMMZ`: the day and time of a date-time group, for where its month and
/// year would only take room.
pub fn day_time_group(unix_s: u64) -> String {
    let mut group = date_time_group(unix_s);
    group.truncate(7);
    group
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian
/// calendar. Howard Hinnant's `civil_from_days`; done here rather than with a
/// date crate because this is the only calendar arithmetic the HMI has.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_time_group_format() {
        assert_eq!(date_time_group(0), "010000Z JAN 70");
        assert_eq!(date_time_group(1_728_045_000), "041230Z OCT 24");
        assert_eq!(date_time_group(951_782_399), "282359Z FEB 00");
        assert_eq!(date_time_group(951_782_400), "290000Z FEB 00"); // leap day
        assert_eq!(date_time_group(4_102_444_799), "312359Z DEC 99"); // 2099
    }

    #[test]
    fn the_day_and_time_of_a_date_time_group() {
        assert_eq!(day_time_group(1_728_045_000), "041230Z");
        assert_eq!(day_time_group(951_782_400), "290000Z");
    }
}
