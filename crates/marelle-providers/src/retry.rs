// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

//! Trying again when a provider is busy.
//!
//! A provider that answers "too many requests" or "overloaded" is usually
//! healthy a few seconds later, and a long agent run should not die of one
//! such answer. This module decides which failures deserve another try and
//! how long to wait before it. [`Client::chat`](crate::Client::chat) does the
//! sending.
//!
//! What is tried again:
//!
//! * HTTP 408, 409, 425, 429, 500, 502, 503, 504, 520 to 524 and 529;
//! * a connection that could not be made or broke before an answer began;
//! * an error the provider reports inside its answer when that error means
//!   "busy" (`overloaded_error`, Z.ai 1302 and 1305, `finish_reason:
//!   network_error`).
//!
//! What never is:
//!
//! * a quota that is spent. A 429 that carries `insufficient_quota`, or one of
//!   the Z.ai codes for an empty balance, an expired plan or a usage window
//!   that resets hours from now, does not heal in seconds;
//! * a mistake in the request (the other 4xx), a rejected certificate, a
//!   redirect;
//! * a provider that has gone silent after the connection was made. It may be
//!   thinking, and waiting again would only double the silence;
//! * anything once a piece of the answer has reached the caller, because the
//!   piece would then appear twice.
//!
//! The provider's own word counts. `Retry-After` (seconds or a date) and
//! `retry-after-ms` set the pause, and `x-should-retry` overrides the table
//! above in either direction, as it does in the official SDKs.

use std::collections::hash_map::RandomState;
use std::error::Error;
use std::hash::{BuildHasher, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::HeaderMap;

/// How often and how patiently to try again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Tries after the first one. Zero turns retrying off.
    pub max_retries: u32,
    /// The pause before the first retry. Each later pause doubles.
    pub first_delay: Duration,
    /// The longest pause that doubling may reach.
    pub max_delay: Duration,
    /// The most that all pauses together may add up to. A provider that asks
    /// for more than what is left is not waited for.
    pub budget: Duration,
}

impl RetryPolicy {
    /// Never try again.
    pub const fn none() -> Self {
        Self {
            max_retries: 0,
            first_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            budget: Duration::from_secs(60),
        }
    }

    /// The most tries that can be made, the first one included.
    pub const fn attempts(&self) -> u32 {
        self.max_retries.saturating_add(1)
    }

    /// The pause before retry number `retry`, where the first retry is 1.
    ///
    /// `waited` is what earlier pauses already used, `hint` is what the
    /// provider asked for, and `unit` is a number in `[0, 1]` that spreads the
    /// pauses of many clients apart. Returns `None` when it is time to give
    /// up: the retries are used up, or the pause does not fit in the budget.
    pub(crate) fn pause(
        &self,
        retry: u32,
        waited: Duration,
        hint: Option<Duration>,
        unit: f64,
    ) -> Option<Duration> {
        if retry == 0 || retry > self.max_retries {
            return None;
        }
        let pause = match hint {
            Some(hint) => hint,
            None => {
                let step = self
                    .first_delay
                    .saturating_mul(1 << (retry - 1).min(16))
                    .min(self.max_delay);
                let unit = if unit.is_finite() {
                    unit.clamp(0.0, 1.0)
                } else {
                    0.5
                };
                // Half the step is fixed and half is random: the pause always
                // grows, and two clients that failed together still part ways.
                let half = step / 2;
                half + half.mul_f64(unit)
            }
        };
        (pause <= self.budget.saturating_sub(waited)).then_some(pause)
    }
}

impl Default for RetryPolicy {
    /// Four retries after pauses of about 1, 2, 4 and 8 seconds, never more
    /// than 30 seconds apiece and 60 seconds in all.
    fn default() -> Self {
        Self {
            max_retries: 4,
            ..Self::none()
        }
    }
}

/// A try that failed and will be repeated. The client reports it to the sink
/// before it starts to wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retry {
    /// The number of the try that is about to start. The first retry is try 2.
    pub attempt: u32,
    /// The most tries that will be made.
    pub attempts: u32,
    /// How long the client waits before it sends the request again.
    pub wait: Duration,
    /// What went wrong.
    pub cause: RetryCause,
}

/// Why a try is repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryCause {
    /// The provider answered with this HTTP status.
    Http(u16),
    /// The provider reported inside its answer that it is busy.
    Busy,
    /// The connection could not be made or broke.
    Network,
    /// No connection could be made in time.
    Timeout,
}

/// What a failure says about trying again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retryable {
    /// Report the failure.
    No,
    /// Send the request again, after the pause the provider asked for if it
    /// asked for one.
    Yes { hint: Option<Duration> },
}

/// A number in `[0, 1)` that differs from call to call.
///
/// Every `RandomState` is seeded by the operating system, which is all the
/// randomness a pause needs and saves a crate.
pub(crate) fn jitter() -> f64 {
    let bits = RandomState::new().build_hasher().finish();
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

/// Judges an HTTP error answer.
///
/// `ids` are the codes and types the body names for the error, such as
/// `1305` or `rate_limit_error`.
pub(crate) fn status(
    status: u16,
    headers: &HeaderMap,
    ids: &[String],
    now: SystemTime,
) -> Retryable {
    let hint = retry_after(headers, now);
    match header_word(headers, "x-should-retry").as_deref() {
        Some("false") => return Retryable::No,
        Some("true") => return Retryable::Yes { hint },
        _ => {}
    }
    let transient = matches!(
        status,
        408 | 409 | 425 | 429 | 500 | 502 | 503 | 504 | 520..=524 | 529
    );
    if !transient || (status == 429 && quota_is_spent(ids)) {
        return Retryable::No;
    }
    Retryable::Yes { hint }
}

/// Judges a failure of the connection itself.
pub(crate) fn network(error: &reqwest::Error) -> Retryable {
    if error.is_builder() || is_tls_rejection(error) {
        return Retryable::No;
    }
    // Silence after the connection was made: the model may be thinking.
    if error.is_timeout() && !error.is_connect() {
        return Retryable::No;
    }
    Retryable::Yes { hint: None }
}

/// Judges an error the provider reported inside an answer.
///
/// `message` is the text the decoders produce: the provider's words followed
/// by its code or type in parentheses.
pub(crate) fn api_message(message: &str) -> Retryable {
    const BUSY: &[&str] = &[
        "overloaded_error",
        "overloaded",
        "rate_limit_error",
        "rate_limit_exceeded",
        "timeout_error",
        "server_error",
        "service_unavailable",
        "1234",
        "1302",
        "1305",
    ];
    let id = message
        .trim_end()
        .strip_suffix(')')
        .and_then(|head| head.rsplit_once('('))
        .map(|(_, id)| id.trim().to_ascii_lowercase());
    let busy = id.is_some_and(|id| BUSY.contains(&id.as_str()))
        || message.ends_with("finish_reason network_error");
    if busy {
        Retryable::Yes { hint: None }
    } else {
        Retryable::No
    }
}

/// Which error cause to tell the caller about.
pub(crate) fn cause(error: &crate::ProviderError) -> Option<RetryCause> {
    use crate::ProviderError;
    match error {
        ProviderError::Http { status, .. } => Some(RetryCause::Http(*status)),
        ProviderError::Api { .. } => Some(RetryCause::Busy),
        ProviderError::Transport { .. } => Some(RetryCause::Network),
        ProviderError::Timeout => Some(RetryCause::Timeout),
        _ => None,
    }
}

/// Whether a 429 says that the quota is gone rather than that the provider is
/// busy. Waiting seconds does not bring a quota back.
fn quota_is_spent(ids: &[String]) -> bool {
    ids.iter().any(|id| {
        let id = id.trim().to_ascii_lowercase();
        if matches!(
            id.as_str(),
            "insufficient_quota" | "billing_not_active" | "billing_hard_limit_reached"
        ) {
            return true;
        }
        // Z.ai: 1113 no balance, 1308 and 1310 a usage window that resets
        // later, 1309 and 1314 an expired plan, 1311 a model outside the
        // plan, 1313 the fair-use limit, 1315 the wrong kind of key, and
        // 1316 to 1321 the windows of the extra-usage plans.
        id.parse::<u32>()
            .is_ok_and(|code| matches!(code, 1113 | 1308..=1311 | 1313..=1321))
    })
}

/// How long the provider asks to wait, from `retry-after-ms` or
/// `retry-after`. The latter is a number of seconds or an HTTP date.
fn retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    if let Some(ms) = header_word(headers, "retry-after-ms").and_then(|text| seconds(&text, 1e-3)) {
        return Some(ms);
    }
    let text = header_word(headers, "retry-after")?;
    seconds(&text, 1.0).or_else(|| {
        let date = parse_http_date(&text)?;
        Some(date.duration_since(now).unwrap_or(Duration::ZERO))
    })
}

/// A non-negative number, in units of `scale` seconds.
fn seconds(text: &str, scale: f64) -> Option<Duration> {
    let number: f64 = text.parse().ok()?;
    Duration::try_from_secs_f64(number * scale).ok()
}

fn header_word(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?.trim();
    (!value.is_empty()).then(|| value.to_ascii_lowercase())
}

/// Reads an HTTP date in any of the three formats the standard allows:
/// `Sun, 06 Nov 1994 08:49:37 GMT`, `Sunday, 06-Nov-94 08:49:37 GMT` and
/// `Sun Nov  6 08:49:37 1994`.
fn parse_http_date(text: &str) -> Option<SystemTime> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let lower = text.to_ascii_lowercase();
    let tokens: Vec<&str> = lower
        .split([' ', ',', '-', ':'])
        .filter(|token| !token.is_empty())
        .collect();
    let month_at = tokens
        .iter()
        .position(|token| token.len() >= 3 && MONTHS.contains(&&token[..3]))?;
    let month = MONTHS.iter().position(|m| *m == &tokens[month_at][..3])? as i64 + 1;
    let number = |index: usize| tokens.get(index)?.parse::<i64>().ok();
    let (day, year, clock) = match month_at {
        // `Sun, 06 Nov 1994 08:49:37 GMT` and `Sunday, 06-Nov-94 08:49:37 GMT`.
        2 => (number(1)?, number(3)?, 4),
        // `Sun Nov  6 08:49:37 1994`.
        1 => (number(2)?, number(6)?, 3),
        _ => return None,
    };
    // A two-digit year is read the way POSIX reads it.
    let year = match year {
        0..=68 if tokens[month_at + 1].len() <= 2 => year + 2000,
        69..=99 if tokens[month_at + 1].len() <= 2 => year + 1900,
        other => other,
    };
    let (hour, minute, second) = (number(clock)?, number(clock + 1)?, number(clock + 2)?);
    if !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let epoch = days * 86_400 + hour * 3_600 + minute * 60 + second;
    u64::try_from(epoch)
        .ok()
        .and_then(|secs| UNIX_EPOCH.checked_add(Duration::from_secs(secs)))
}

/// Days from 1970-01-01 to the given day of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Whether the failure is TLS refusing the server, which no pause will fix.
fn is_tls_rejection(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.is::<rustls::Error>() {
            return true;
        }
        // An `io::Error` hides the error it wraps from `source`.
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            if io
                .get_ref()
                .is_some_and(|inner| inner.is::<rustls::Error>())
            {
                return true;
            }
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    fn ids(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn now() -> SystemTime {
        // Sun, 06 Nov 1994 08:49:37 GMT
        UNIX_EPOCH + Duration::from_secs(784_111_777)
    }

    fn yes(hint: Option<Duration>) -> Retryable {
        Retryable::Yes { hint }
    }

    #[test]
    fn pauses_double_from_the_first_delay_and_stop_at_the_cap() {
        let policy = RetryPolicy {
            max_retries: 8,
            first_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            budget: Duration::from_secs(1_000),
        };
        let whole = |retry| policy.pause(retry, Duration::ZERO, None, 1.0).unwrap();
        let least = |retry| policy.pause(retry, Duration::ZERO, None, 0.0).unwrap();
        assert_eq!(whole(1), Duration::from_secs(1));
        assert_eq!(whole(2), Duration::from_secs(2));
        assert_eq!(whole(3), Duration::from_secs(4));
        assert_eq!(whole(4), Duration::from_secs(8));
        assert_eq!(whole(5), Duration::from_secs(10), "the cap");
        assert_eq!(whole(8), Duration::from_secs(10));
        // Half of each step is fixed, so a pause is never shorter than that.
        assert_eq!(least(1), Duration::from_millis(500));
        assert_eq!(least(3), Duration::from_secs(2));
        assert_eq!(least(8), Duration::from_secs(5));
    }

    #[test]
    fn the_jitter_stays_inside_its_range_whatever_it_is_given() {
        let policy = RetryPolicy::default();
        for unit in [-3.0, 0.0, 0.25, 0.999, 1.0, 7.0, f64::NAN, f64::INFINITY] {
            let pause = policy.pause(1, Duration::ZERO, None, unit).unwrap();
            assert!(pause >= Duration::from_millis(500), "{unit}: {pause:?}");
            assert!(pause <= Duration::from_secs(1), "{unit}: {pause:?}");
        }
        for _ in 0..200 {
            let unit = jitter();
            assert!((0.0..1.0).contains(&unit), "{unit}");
        }
        let mut seen: Vec<u64> = (0..50).map(|_| jitter().to_bits()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert!(
            seen.len() > 40,
            "the numbers should differ from call to call"
        );
    }

    #[test]
    fn retries_run_out() {
        let policy = RetryPolicy {
            max_retries: 2,
            ..RetryPolicy::default()
        };
        assert!(policy.pause(1, Duration::ZERO, None, 0.5).is_some());
        assert!(policy.pause(2, Duration::ZERO, None, 0.5).is_some());
        assert!(policy.pause(3, Duration::ZERO, None, 0.5).is_none());
        assert!(policy.pause(0, Duration::ZERO, None, 0.5).is_none());
        assert_eq!(policy.attempts(), 3);
        assert_eq!(RetryPolicy::none().attempts(), 1);
        assert!(
            RetryPolicy::none()
                .pause(1, Duration::ZERO, None, 0.5)
                .is_none()
        );
        let all = RetryPolicy {
            max_retries: u32::MAX,
            ..RetryPolicy::default()
        };
        assert_eq!(all.attempts(), u32::MAX, "no overflow");
        assert!(all.pause(u32::MAX, Duration::ZERO, None, 0.5).is_some());
    }

    #[test]
    fn the_budget_limits_what_all_pauses_add_up_to() {
        let policy = RetryPolicy {
            max_retries: 10,
            first_delay: Duration::from_secs(4),
            max_delay: Duration::from_secs(30),
            budget: Duration::from_secs(10),
        };
        // 4 s fits, then 8 s does not fit in the 6 s that are left.
        assert_eq!(
            policy.pause(1, Duration::ZERO, None, 1.0),
            Some(Duration::from_secs(4))
        );
        assert_eq!(policy.pause(2, Duration::from_secs(4), None, 1.0), None);
        // Exactly what is left is allowed.
        assert_eq!(
            policy.pause(2, Duration::from_secs(2), None, 1.0),
            Some(Duration::from_secs(8))
        );
        // An exhausted budget never underflows.
        assert_eq!(policy.pause(1, Duration::from_secs(99), None, 1.0), None);
    }

    #[test]
    fn what_the_provider_asks_for_replaces_the_doubling_within_the_budget() {
        let policy = RetryPolicy::default();
        let ask = |secs| policy.pause(1, Duration::ZERO, Some(Duration::from_secs(secs)), 0.0);
        assert_eq!(ask(0), Some(Duration::ZERO), "retry at once is allowed");
        assert_eq!(ask(7), Some(Duration::from_secs(7)));
        // More than the longest doubling pause, still inside the budget.
        assert_eq!(ask(45), Some(Duration::from_secs(45)));
        // More than the budget: the provider is not waited for.
        assert_eq!(ask(61), None);
        assert_eq!(ask(3_600), None);
        assert_eq!(
            policy.pause(
                2,
                Duration::from_secs(50),
                Some(Duration::from_secs(11)),
                0.0
            ),
            None
        );
    }

    #[test]
    fn transient_statuses_are_tried_again_and_mistakes_are_not() {
        let none = HeaderMap::new();
        for code in [
            408, 409, 425, 429, 500, 502, 503, 504, 520, 521, 522, 523, 524, 529,
        ] {
            assert_eq!(status(code, &none, &[], now()), yes(None), "{code}");
        }
        for code in [
            400, 401, 402, 403, 404, 405, 413, 422, 501, 505, 511, 301, 307,
        ] {
            assert_eq!(status(code, &none, &[], now()), Retryable::No, "{code}");
        }
    }

    #[test]
    fn a_spent_quota_is_not_a_busy_provider() {
        let none = HeaderMap::new();
        // Z.ai codes that the documentation ties to a balance, a plan or a
        // usage window.
        for code in [
            "1113", "1308", "1309", "1310", "1311", "1313", "1314", "1315", "1316", "1318", "1321",
        ] {
            assert_eq!(
                status(429, &none, &ids(&[code]), now()),
                Retryable::No,
                "{code}"
            );
        }
        for id in ["insufficient_quota", "billing_hard_limit_reached"] {
            assert_eq!(
                status(429, &none, &ids(&["error", id]), now()),
                Retryable::No,
                "{id}"
            );
        }
        // The codes that mean "slow down" and "overloaded", and unknown ones.
        for id in [
            "1302",
            "1305",
            "rate_limit_exceeded",
            "rate_limit_error",
            "9999",
        ] {
            assert_eq!(status(429, &none, &ids(&[id]), now()), yes(None), "{id}");
        }
        // The quota codes only mean something for a 429.
        assert_eq!(status(503, &none, &ids(&["1308"]), now()), yes(None));
    }

    #[test]
    fn the_provider_can_overrule_the_table_in_both_directions() {
        let no = headers(&[("x-should-retry", "false")]);
        assert_eq!(status(503, &no, &[], now()), Retryable::No);
        assert_eq!(status(429, &no, &[], now()), Retryable::No);
        let yes_header = headers(&[("x-should-retry", " TRUE "), ("retry-after", "3")]);
        assert_eq!(
            status(400, &yes_header, &[], now()),
            yes(Some(Duration::from_secs(3)))
        );
        let other = headers(&[("x-should-retry", "maybe")]);
        assert_eq!(status(503, &other, &[], now()), yes(None));
        assert_eq!(status(400, &other, &[], now()), Retryable::No);
    }

    #[test]
    fn retry_after_in_seconds_and_milliseconds() {
        let hint = |pairs: &[(&'static str, &str)]| retry_after(&headers(pairs), now());
        assert_eq!(hint(&[]), None);
        assert_eq!(hint(&[("retry-after", "7")]), Some(Duration::from_secs(7)));
        assert_eq!(hint(&[("retry-after", "0")]), Some(Duration::ZERO));
        assert_eq!(
            hint(&[("retry-after", " 2.5 ")]),
            Some(Duration::from_millis(2_500))
        );
        assert_eq!(
            hint(&[("retry-after-ms", "250")]),
            Some(Duration::from_millis(250))
        );
        // The millisecond header is the more exact one and wins.
        assert_eq!(
            hint(&[("retry-after", "9"), ("retry-after-ms", "1500")]),
            Some(Duration::from_millis(1_500))
        );
        for junk in ["soon", "", "-4", "NaN", "inf", "1e999", "--"] {
            assert_eq!(hint(&[("retry-after", junk)]), None, "{junk:?}");
        }
        // A broken millisecond header falls back to the seconds one.
        assert_eq!(
            hint(&[("retry-after", "4"), ("retry-after-ms", "later")]),
            Some(Duration::from_secs(4))
        );
    }

    #[test]
    fn retry_after_as_a_date_counts_from_now() {
        let at = |text: &str| retry_after(&headers(&[("retry-after", text)]), now());
        // Sixty seconds after `now()`, in the three formats of the standard.
        let minute = Some(Duration::from_secs(60));
        assert_eq!(at("Sun, 06 Nov 1994 08:50:37 GMT"), minute);
        assert_eq!(at("Sunday, 06-Nov-94 08:50:37 GMT"), minute);
        assert_eq!(at("Sun Nov  6 08:50:37 1994"), minute);
        // A date in the past means "now".
        assert_eq!(at("Sun, 06 Nov 1994 08:49:00 GMT"), Some(Duration::ZERO));
        assert_eq!(at("Thu, 01 Jan 1970 00:00:00 GMT"), Some(Duration::ZERO));
        // Nonsense is not a date.
        for junk in [
            "Sun, 32 Nov 1994 08:49:37 GMT",
            "Sun, 06 Nov 1994 25:49:37 GMT",
            "Sun, 06 Foo 1994 08:49:37 GMT",
            "Sun, 06 Nov 1994",
            "Sun, 06 Nov 1994 08:49 GMT",
            "yesterday at noon",
        ] {
            assert_eq!(at(junk), None, "{junk:?}");
        }
    }

    #[test]
    fn dates_are_turned_into_the_right_instant() {
        let epoch = |text: &str| {
            parse_http_date(text)
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        };
        assert_eq!(epoch("Sun, 06 Nov 1994 08:49:37 GMT"), 784_111_777);
        assert_eq!(epoch("Thu, 01 Jan 1970 00:00:00 GMT"), 0);
        // A leap day, the turn of a century that is a leap year, and one that
        // is not.
        assert_eq!(epoch("Tue, 29 Feb 2000 00:00:00 GMT"), 951_782_400);
        assert_eq!(epoch("Wed, 01 Mar 2000 00:00:00 GMT"), 951_868_800);
        assert_eq!(epoch("Mon, 01 Mar 2100 00:00:00 GMT"), 4_107_542_400);
        // A leap second is read as the last second of the minute.
        assert_eq!(epoch("Fri, 31 Dec 2021 23:59:60 GMT"), 1_640_995_200);
        // Two-digit years: below 69 is this century, the rest the last one.
        assert_eq!(epoch("Saturday, 01-Jan-00 00:00:00 GMT"), 946_684_800);
        assert_eq!(epoch("Sunday, 06-Nov-94 08:49:37 GMT"), 784_111_777);
        // Before 1970 there is nothing to wait for.
        assert_eq!(parse_http_date("Wed, 31 Dec 1969 23:59:59 GMT"), None);
    }

    #[test]
    fn errors_inside_an_answer_are_judged_by_their_code() {
        for busy in [
            "Overloaded (overloaded_error)",
            "The service may be temporarily overloaded (1305)",
            "Too fast (1302)",
            "network error (1234)",
            "slow down (rate_limit_exceeded)",
            "oops (server_error)",
            "the provider ended the answer with finish_reason network_error",
            "x (Overloaded_Error) ",
        ] {
            assert_eq!(api_message(busy), yes(None), "{busy}");
        }
        for fatal in [
            "bad (invalid_request_error)",
            "insufficient balance (1113)",
            "usage limit reached (1308)",
            "the provider ended the answer with finish_reason error",
            "overloaded but with no code at all",
            "bad (",
            "",
            ")",
        ] {
            assert_eq!(api_message(fatal), Retryable::No, "{fatal}");
        }
    }

    #[test]
    fn failures_have_a_cause_to_report() {
        use crate::ProviderError;
        assert_eq!(
            cause(&ProviderError::Http {
                status: 503,
                message: String::new()
            }),
            Some(RetryCause::Http(503))
        );
        assert_eq!(
            cause(&ProviderError::Api {
                message: String::new()
            }),
            Some(RetryCause::Busy)
        );
        assert_eq!(
            cause(&ProviderError::Transport {
                detail: String::new()
            }),
            Some(RetryCause::Network)
        );
        assert_eq!(cause(&ProviderError::Timeout), Some(RetryCause::Timeout));
        assert_eq!(cause(&ProviderError::InvalidApiKey), None);
    }

    #[derive(Debug)]
    struct Layer(std::io::Error);

    impl std::fmt::Display for Layer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a layer")
        }
    }

    impl Error for Layer {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn a_rejected_certificate_is_found_inside_an_io_error() {
        let rejected = Layer(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            rustls::Error::General("the certificate is wrong".into()),
        ));
        assert!(is_tls_rejection(&rejected));
        let reset = Layer(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection reset by peer",
        ));
        assert!(!is_tls_rejection(&reset));
        assert!(is_tls_rejection(&rustls::Error::General("x".into())));
        assert!(!is_tls_rejection(&std::io::Error::from(
            std::io::ErrorKind::TimedOut
        )));
    }
}
