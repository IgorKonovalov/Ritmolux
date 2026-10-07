//! The operating system's UDP counters that sit in front of `recv_from`, which
//! the player's own listener counters cannot see (backlog 0219, ADR-0265).
//!
//! A datagram that `send_to` sent and the listener never returned leaves every
//! counter on the player's side at zero, so where it went is a question only
//! the kernel can answer:
//!
//! - **Linux:** the listener socket's own `drops` column in `/proc/net/udp`,
//!   matched by its bound local port — the clean, per-socket reading — plus the
//!   system-wide `Udp: InErrors` and `RcvbufErrors` from `/proc/net/snmp`, and
//!   `/proc/sys/net/core/rmem_default`, which is the receive buffer the socket
//!   got because nothing sets `SO_RCVBUF`.
//! - **Windows:** the system-wide `Receive Errors` from `netstat -s -p udp`.
//!   Windows has no per-socket figure.
//!
//! The system-wide counters move with every other process on the machine, so a
//! delta in them is supporting evidence only; the per-socket `drops` is the
//! reading that convicts.
//!
//! Every source is read as text and every reader yields `None` — printed
//! "unavailable" — for a source that is missing, unreadable or shaped other than
//! expected, so a report taken on a failure path can never itself panic.

use std::fmt::Write as _;

/// One reading of every counter this platform exposes. `None` is a source that
/// could not be read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UdpCounters {
    /// The `drops` column summed over every `/proc/net/udp` row bound to the
    /// port (Linux).
    pub socket_drops: Option<u64>,
    /// `Udp: InErrors` from `/proc/net/snmp` (Linux, system-wide).
    pub in_errors: Option<u64>,
    /// `Udp: RcvbufErrors` from `/proc/net/snmp` (Linux, system-wide).
    pub rcvbuf_errors: Option<u64>,
    /// `/proc/sys/net/core/rmem_default`, in bytes (Linux). A setting, not a
    /// counter: reported as read, never as a delta.
    pub rmem_default: Option<u64>,
    /// `Receive Errors` from `netstat -s -p udp` (Windows, system-wide).
    pub receive_errors: Option<u64>,
}

impl UdpCounters {
    /// Read every source this platform has for the socket bound to `port`.
    pub fn take(port: u16) -> Self {
        #[cfg(target_os = "linux")]
        {
            let read = |path: &str| std::fs::read_to_string(path).ok();
            Self::from_sources(
                port,
                read("/proc/net/udp").as_deref(),
                read("/proc/net/snmp").as_deref(),
                read("/proc/sys/net/core/rmem_default").as_deref(),
                None,
            )
        }
        #[cfg(windows)]
        {
            let netstat = std::process::Command::new("netstat")
                .args(["-s", "-p", "udp"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok());
            Self::from_sources(port, None, None, None, netstat.as_deref())
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            Self::from_sources(port, None, None, None, None)
        }
    }

    /// The reading the four texts carry, each `None` when its source was not
    /// read. The parsing half of [`take`](Self::take), apart so it is testable
    /// against literal fixture text on any platform.
    pub fn from_sources(
        port: u16,
        proc_net_udp: Option<&str>,
        proc_net_snmp: Option<&str>,
        rmem_default: Option<&str>,
        netstat_udp: Option<&str>,
    ) -> Self {
        Self {
            socket_drops: proc_net_udp.and_then(|text| socket_drops(text, port)),
            in_errors: proc_net_snmp.and_then(|text| snmp_udp(text, "InErrors")),
            rcvbuf_errors: proc_net_snmp.and_then(|text| snmp_udp(text, "RcvbufErrors")),
            rmem_default: rmem_default.and_then(|text| text.trim().parse().ok()),
            receive_errors: netstat_udp.and_then(netstat_receive_errors),
        }
    }

    /// What moved between `self` (taken before the send) and `later`, as one
    /// line: each counter's delta, or "unavailable" when either reading of it is
    /// missing, and `rmem_default` as read.
    pub fn deltas(&self, later: &Self) -> String {
        let delta = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(a), Some(b)) => format!("+{}", b.saturating_sub(a)),
            _ => "unavailable".to_owned(),
        };
        let mut line = String::from("os udp: ");
        let _ = write!(
            line,
            "socket drops {}, InErrors {}, RcvbufErrors {}, rmem_default {}, \
             Receive Errors {}",
            delta(self.socket_drops, later.socket_drops),
            delta(self.in_errors, later.in_errors),
            delta(self.rcvbuf_errors, later.rcvbuf_errors),
            later
                .rmem_default
                .map_or_else(|| "unavailable".to_owned(), |bytes| bytes.to_string()),
            delta(self.receive_errors, later.receive_errors),
        );
        line
    }
}

/// The `drops` column summed over the `/proc/net/udp` rows whose local port is
/// `port`, or `None` when no row is bound to it.
///
/// A row's columns are whitespace-separated and `drops` is the last; the header
/// line names `tx_queue rx_queue` and `tr tm->when` as four words where a row
/// carries two, so columns are never located through the header. The local
/// address is `ADDR:PORT` in hex; the address half is in the kernel's byte
/// order, but the port half is the port number itself.
fn socket_drops(text: &str, port: u16) -> Option<u64> {
    let mut found = None;
    for row in text.lines().skip(1) {
        let mut columns = row.split_whitespace();
        let Some(local) = columns.nth(1) else {
            continue;
        };
        let Some((_, hex_port)) = local.rsplit_once(':') else {
            continue;
        };
        if u16::from_str_radix(hex_port, 16).ok() != Some(port) {
            continue;
        }
        let Some(drops) = columns.next_back().and_then(|d| d.parse::<u64>().ok()) else {
            continue;
        };
        found = Some(found.unwrap_or(0) + drops);
    }
    found
}

/// One named counter of the `Udp:` pair of lines in `/proc/net/snmp`: a header
/// line of names, then a line of values in the same order.
fn snmp_udp(text: &str, name: &str) -> Option<u64> {
    let mut udp = text.lines().filter(|line| line.starts_with("Udp:"));
    let names = udp.next()?;
    let values = udp.next()?;
    let index = names.split_whitespace().position(|n| n == name)?;
    values.split_whitespace().nth(index)?.parse().ok()
}

/// `Receive Errors = N` from `netstat -s -p udp`'s block. `-p udp` prints the
/// IPv4 block only, so the first such line is the one.
fn netstat_receive_errors(text: &str) -> Option<u64> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .find(|(label, _)| label.trim() == "Receive Errors")
        .and_then(|(_, value)| value.trim().parse().ok())
}
