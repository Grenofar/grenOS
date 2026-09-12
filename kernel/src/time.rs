//! A date and a time of day, as the CMOS clock keeps them, with their French
//! names.

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

const DAYS: [&str; 7] = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."];
// The 8x8 font draws é well, but not û: août is written aout.
const MONTHS: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "aout", "sept.", "oct.", "nov.", "déc.",
];

impl DateTime {
    /// The day of the week, Monday being 0 (Sakamoto's method).
    pub fn weekday(self) -> usize {
        const OFFSETS: [u16; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let month = usize::from(self.month.clamp(1, 12) - 1);
        let year = if self.month < 3 { self.year.saturating_sub(1) } else { self.year };
        let sunday_first = (year + year / 4 - year / 100 + year / 400 + OFFSETS[month] + u16::from(self.day)) % 7;
        (usize::from(sunday_first) + 6) % 7
    }

    pub fn day_name(self) -> &'static str {
        DAYS[self.weekday()]
    }

    pub fn month_name(self) -> &'static str {
        MONTHS[usize::from(self.month.clamp(1, 12) - 1)]
    }
}
