pub fn time_to_chrono(time_date: time::OffsetDateTime) -> chrono::DateTime<chrono::Utc> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Since chrono has lower precision than time, truncating is the only option."
    )]
    chrono::TimeZone::timestamp_nanos(
        &chrono::Utc,
        time_date.to_utc().unix_timestamp_nanos() as i64,
    )
}

#[cfg(test)]
mod test {
    use super::time_to_chrono;

    #[test]
    fn works_across_timezones() {
        let time_date = time::OffsetDateTime::new_in_offset(
            time::Date::from_calendar_date(2025, time::Month::February, 15).unwrap(),
            time::Time::from_hms(5, 1, 17).unwrap(),
            time::UtcOffset::from_hms(1, 0, 0).unwrap(),
        )
        .to_offset(time::UtcOffset::UTC);

        let chrono_time = time_to_chrono(time_date);

        assert_eq!(time_date.offset(), time::UtcOffset::UTC);
        assert_eq!(chrono_time.offset(), &chrono::Utc);

        assert_eq!(time_date.unix_timestamp(), chrono_time.timestamp());
    }
}
