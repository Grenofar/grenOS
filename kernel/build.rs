use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    // Tell cargo to pass the linker script to the linker..
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");
    // ..and to re-run if it changes.
    println!("cargo:rerun-if-changed=linker-{arch}.ld");

    // Which build this is, for Paramètres > Mise à jour and for `uname`. In
    // CI it is the commit the image was made from; on a desktop, "local".
    let build = std::env::var("GITHUB_SHA")
        .map(|sha| sha.chars().take(7).collect::<String>())
        .unwrap_or_else(|_| "local".to_string());
    println!("cargo:rustc-env=GRENOS_BUILD={build}");
    println!("cargo:rustc-env=GRENOS_BUILT_AT={}", today());
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
}

/// Today's date in UTC, as YYYY-MM-DD, without pulling in a date crate.
fn today() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0);
    let days = (seconds / 86_400) as i64;
    // Howard Hinnant's civil_from_days, with 1970-01-01 as day zero.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}
