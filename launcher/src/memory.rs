//! How much memory the game may be given on this computer.
//!
//! A Java game that is given nearly all of the computer's memory makes the whole computer
//! stall: the system, the browser and the game's own native memory (textures, the JIT, the
//! garbage collector's tables) all live outside the heap. [`Limits`] says how far the slider
//! goes: 82% of what the computer reports, the other 18% (never under 1 GiB) left for
//! everything else — a 16 GiB computer (most report a little under that, around 15.6-15.9 GiB
//! usable) tops out close to 12.6 GiB, an 8 GiB one around 6.3 GiB, a 32 GiB one around 26 GiB.

/// The slider's step, in MiB.
pub const STEP_MB: u32 = 512;
/// The least the game is offered: Minecraft doesn't start well on less.
pub const MIN_MB: u32 = 1024;
/// The slider never goes past this: a bigger heap only makes the garbage collector's pauses
/// longer.
pub const CEILING_MB: u32 = 32 * 1024;

/// What the slider offers on one computer, all in MiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The computer's memory; none when it couldn't be read.
    pub total_mb: Option<u32>,
    /// The most the game should get without starving the system.
    pub safe_max_mb: u32,
    /// What a new instance starts with.
    pub recommended_mb: u32,
}

impl Limits {
    /// The limits of a computer with `total_mb` of memory.
    pub fn for_total(total_mb: u32) -> Limits {
        // 18% held back for the system, never less than 1 GiB of it: on a small computer a
        // percentage alone would leave too little, so the floor takes over there.
        let reserve = (total_mb.saturating_mul(18) / 100).max(1024);
        // The safe maximum is exact, not rounded to a step: the step is only how far apart the
        // slider's own marks sit (see `positions`), not a limit on the number itself.
        let safe_max_mb = total_mb.saturating_sub(reserve).clamp(MIN_MB, CEILING_MB);
        // A quarter of the memory, between 2 and 8 GiB, to the nearest step.
        let quarter = (total_mb / 4).clamp(2048, 8192);
        let recommended_mb = ((quarter + STEP_MB / 2) / STEP_MB * STEP_MB).min(safe_max_mb);
        Limits {
            total_mb: Some(total_mb),
            safe_max_mb,
            recommended_mb,
        }
    }

    /// This computer's limits; a computer whose memory can't be read gets 8 GiB's.
    pub fn detect() -> Limits {
        match total_mb() {
            Some(total) => Limits::for_total(total),
            None => Limits {
                total_mb: None,
                ..Limits::for_total(8 * 1024)
            },
        }
    }

    /// `mb` brought into what the slider offers, on a step.
    pub fn clamp(&self, mb: u32) -> u32 {
        let stepped = (mb + STEP_MB / 2) / STEP_MB * STEP_MB;
        // The top is the safe maximum itself, even when it isn't on a step.
        if mb >= self.safe_max_mb {
            self.safe_max_mb
        } else {
            stepped.clamp(MIN_MB, self.safe_max_mb)
        }
    }

    /// Whether `mb` is more than the system can spare.
    pub fn over(&self, mb: u32) -> bool {
        mb > self.safe_max_mb
    }

    /// The slider's positions, each a value it can take: from [`MIN_MB`] by [`STEP_MB`] to the
    /// safe maximum (itself the last when it falls between steps).
    pub fn positions(&self) -> Vec<u32> {
        let mut out: Vec<u32> = (MIN_MB..=self.safe_max_mb)
            .step_by(STEP_MB as usize)
            .collect();
        if out.last() != Some(&self.safe_max_mb) {
            out.push(self.safe_max_mb);
        }
        out
    }

    /// The part of the line past the safe maximum, for the slider to show as "too much": the
    /// steps up to the computer's own memory (never past [`CEILING_MB`]).
    pub fn danger_positions(&self) -> Vec<u32> {
        let top = self.total_mb.unwrap_or(0).min(CEILING_MB);
        let mut out = Vec::new();
        let mut at = self.safe_max_mb / STEP_MB * STEP_MB + STEP_MB;
        while at <= top {
            out.push(at);
            at += STEP_MB;
        }
        out
    }
}

/// `4096` as `4 GB`, `12800` as `12.5 GB`.
pub fn gb(mb: u32) -> String {
    if mb.is_multiple_of(1024) {
        format!("{} GB", mb / 1024)
    } else {
        format!("{:.1} GB", mb as f64 / 1024.0)
    }
}

/// The most a game may be started with whatever was asked for: never more than the computer
/// has, less what the system needs to stay alive. Used at launch, so a value set by hand
/// (the command line takes any number) can't freeze the computer.
pub fn fit(requested_mb: u32) -> u32 {
    match total_mb() {
        Some(total) => requested_mb.min(total.saturating_sub(2048).max(MIN_MB)),
        None => requested_mb,
    }
}

/// The computer's memory in MiB.
pub fn total_mb() -> Option<u32> {
    read_total_bytes().map(|b| u32::try_from(b >> 20).unwrap_or(u32::MAX))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn read_total_bytes() -> Option<u64> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

/// `MemTotal:       16318844 kB` in bytes.
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(target_os = "macos")]
fn read_total_bytes() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(windows)]
fn read_total_bytes() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
    let mut status = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0,
        total_phys: 0,
        avail_phys: 0,
        total_page_file: 0,
        avail_page_file: 0,
        total_virtual: 0,
        avail_virtual: 0,
        avail_extended_virtual: 0,
    };
    // SAFETY: `status` is a live, correctly sized MEMORYSTATUSEX with its length set, which is
    // all the call asks of its argument.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then_some(status.total_phys)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "macos",
    windows
)))]
fn read_total_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_16_gib_computer_tops_out_around_12_and_a_half() {
        let l = Limits::for_total(16 * 1024);
        assert_eq!(l.safe_max_mb, 13435);
        // Windows commonly reports a 16 GiB stick as a little under that.
        let l = Limits::for_total(15_360);
        assert_eq!(l.safe_max_mb, 12596);
    }

    #[test]
    fn small_and_large_computers_stay_sane() {
        assert_eq!(Limits::for_total(8 * 1024).safe_max_mb, 6718);
        assert_eq!(Limits::for_total(4 * 1024).safe_max_mb, 3072);
        assert_eq!(Limits::for_total(32 * 1024).safe_max_mb, 26870);
        // Below this, even holding back only the 1 GiB floor leaves too little to start on.
        assert_eq!(Limits::for_total(1024).safe_max_mb, MIN_MB);
        assert_eq!(Limits::for_total(128 * 1024).safe_max_mb, CEILING_MB);
    }

    #[test]
    fn recommended_is_a_quarter_within_reason() {
        assert_eq!(Limits::for_total(16 * 1024).recommended_mb, 4096);
        assert_eq!(Limits::for_total(64 * 1024).recommended_mb, 8192);
        assert_eq!(Limits::for_total(4 * 1024).recommended_mb, 2048);
    }

    #[test]
    fn clamp_never_passes_the_safe_maximum() {
        let l = Limits::for_total(16 * 1024);
        assert_eq!(l.clamp(99_999), l.safe_max_mb);
        assert_eq!(l.clamp(100), MIN_MB);
        assert_eq!(l.clamp(4000), 4096);
        assert!(l.over(14 * 1024));
        assert!(!l.over(12 * 1024));
    }

    #[test]
    fn positions_end_at_the_safe_maximum() {
        let l = Limits::for_total(15_900);
        let p = l.positions();
        assert_eq!(p.first(), Some(&MIN_MB));
        assert_eq!(p.last(), Some(&l.safe_max_mb));
        assert!(p.windows(2).all(|w| w[0] < w[1]));
        let danger = l.danger_positions();
        assert!(danger.iter().all(|&d| d > l.safe_max_mb));
        assert!(danger.last().is_some_and(|&d| d <= 15_900));
    }

    #[test]
    fn names_sizes_in_gigabytes() {
        assert_eq!(gb(4096), "4 GB");
        assert_eq!(gb(12_800), "12.5 GB");
    }

    #[test]
    fn reads_meminfo() {
        let text = "MemTotal:       16318844 kB\nMemFree:  100 kB\n";
        assert_eq!(parse_meminfo(text), Some(16_318_844 * 1024));
        assert_eq!(parse_meminfo("nothing"), None);
    }

    #[test]
    fn fit_leaves_the_system_room() {
        if let Some(total) = total_mb() {
            assert!(fit(u32::MAX) < total);
        }
    }
}
