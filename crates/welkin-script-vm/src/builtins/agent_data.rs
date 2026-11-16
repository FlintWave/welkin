// SPDX-License-Identifier: LGPL-3.0-or-later

#[derive(Debug, Clone, PartialEq)]
pub struct AgentDataRequest {
    pub prim_id: uuid::Uuid,
    pub request_id: String,
    pub agent_id: uuid::Uuid,
    pub data_type: i32,
}

pub const DATA_ONLINE: i32 = 1;
pub const DATA_NAME: i32 = 2;
pub const DATA_BORN: i32 = 3;

pub fn needs_lookup(data_type: i32) -> bool {
    data_type == DATA_NAME || data_type == DATA_BORN
}

pub fn format_name(first: &str, last: &str) -> String {
    if last.is_empty() {
        first.to_owned()
    } else {
        format!("{first} {last}")
    }
}

pub fn format_born(created_unix: i64) -> String {
    if created_unix <= 0 {
        return "1970-01-01".to_owned();
    }

    let days = created_unix.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 {
        z
    } else {
        z - 146_096
    } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_born_epoch() {
        assert_eq!(format_born(0), "1970-01-01");
    }

    #[test]
    fn format_born_2006() {
        assert_eq!(format_born(1_136_073_600), "2006-01-01");
    }

    #[test]
    fn format_born_2023() {
        assert_eq!(format_born(1_700_000_000), "2023-11-14");
    }

    #[test]
    fn format_born_leap_day() {
        assert_eq!(format_born(951_782_400), "2000-02-29");
    }

    #[test]
    fn format_born_negative() {
        assert_eq!(format_born(-5), "1970-01-01");
    }

    #[test]
    fn needs_lookup_truth_table() {
        assert!(!needs_lookup(DATA_ONLINE));
        assert!(needs_lookup(DATA_NAME));
        assert!(needs_lookup(DATA_BORN));
        assert!(!needs_lookup(0));
        assert!(!needs_lookup(4));
        assert!(!needs_lookup(-1));
    }

    #[test]
    fn format_name_cases() {
        assert_eq!(format_name("Foo", "Bar"), "Foo Bar");
        assert_eq!(format_name("Foo", ""), "Foo");
        assert_eq!(format_name("Foo", "Resident"), "Foo Resident");
        assert_eq!(format_name("", "Bar"), " Bar");
    }
}
