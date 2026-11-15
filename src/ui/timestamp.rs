//! Windows converts UTC instants with its configured timezone and historical DST rules.
//! Other platforms are development targets and deliberately display UTC.
const DAY_MS: u64 = 86_400_000;

pub(super) fn clock(timestamp: u64) -> String {
    #[cfg(windows)]
    if let Some(local) = local_parts(timestamp) {
        return format!("{:02}:{:02}", local.wHour, local.wMinute);
    }
    let minutes = timestamp / 60_000;
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

pub(super) fn day_key(timestamp: u64) -> u64 {
    #[cfg(windows)]
    if let Some(local) = local_parts(timestamp) {
        return u64::from(local.wYear) * 10_000
            + u64::from(local.wMonth) * 100
            + u64::from(local.wDay);
    }
    timestamp / DAY_MS
}

pub(super) fn day_label(timestamp: u64) -> String {
    let now = super::rich::now_ms();
    #[cfg(windows)]
    if let (Some(local), Some(today)) = (local_parts(timestamp), local_parts(now)) {
        let date = (local.wYear, local.wMonth, local.wDay);
        let current = (today.wYear, today.wMonth, today.wDay);
        return if date == current {
            "HEUTE".into()
        } else if date == previous_date(current) {
            "GESTERN".into()
        } else {
            format!("{:02}.{:02}.{:04}", date.2, date.1, date.0)
        };
    }
    let day = timestamp / DAY_MS;
    let today = now / DAY_MS;
    if day == today {
        "HEUTE".into()
    } else if day.checked_add(1) == Some(today) {
        "GESTERN".into()
    } else if day < today {
        format!("VOR {} TAGEN", today - day)
    } else {
        "ZUKÜNFTIGES DATUM".into()
    }
}

pub(super) fn zone_label() -> &'static str {
    if cfg!(windows) {
        "Lokale Uhrzeit"
    } else {
        "Uhrzeit in UTC"
    }
}

#[cfg(windows)]
fn previous_date((year, month, day): (u16, u16, u16)) -> (u16, u16, u16) {
    if day > 1 {
        return (year, month, day - 1);
    }
    if month == 1 {
        return (year.saturating_sub(1), 12, 31);
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let previous = month - 1;
    let days = match previous {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (year, previous, days)
}

#[cfg(windows)]
fn local_parts(timestamp: u64) -> Option<windows_sys::Win32::Foundation::SYSTEMTIME> {
    use windows_sys::Win32::{
        Foundation::{FILETIME, SYSTEMTIME},
        System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTimeEx},
    };
    // FILETIME counts 100 ns since 1601; our persisted times count ms since 1970.
    let ticks = timestamp
        .checked_mul(10_000)?
        .checked_add(116_444_736_000_000_000)?;
    let file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc: SYSTEMTIME = unsafe { std::mem::zeroed() };
    let mut local: SYSTEMTIME = unsafe { std::mem::zeroed() };
    // SAFETY: initialized input/output structs; null timezone selects the system's
    // current dynamic timezone. Both documented conversion results are checked.
    if unsafe { FileTimeToSystemTime(&file, &mut utc) } == 0
        || unsafe { SystemTimeToTzSpecificLocalTimeEx(std::ptr::null(), &utc, &mut local) } == 0
    {
        return None;
    }
    Some(local)
}
