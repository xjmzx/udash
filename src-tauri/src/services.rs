//! Daemon status for the chain and network cards, ported from pong.
//!
//! Both cards show the same rows on every OS: BTCD, ELEC and BLOCK on the
//! chain card, NGNX and TOR on the network card. (pong also checked
//! openclaw's port; dropped 2026-10-24 as unused, which leaves the network
//! card's third row free.)
//!
//! Linux reads `systemctl` state per unit. macOS reads `launchctl list`,
//! which covers Homebrew services and hand-written launch agents alike, and
//! falls back to a port check for a daemon with no launchd job. The block
//! height comes from `bitcoin-cli`, or from bitcoind's REST interface when
//! the CLI cannot reach the node. Every read is unprivileged and local-only;
//! a probe that fails reads as unknown ("—"), so one dead daemon never blanks
//! a card.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;

pub const REFRESH: Duration = Duration::from_secs(20);
const TIMEOUT: Duration = Duration::from_secs(4);

/// verificationprogress at or above this reads as synced.
const BTC_SYNCED: f64 = 0.9999;

/// bitcoind's REST chain info, on the default mainnet RPC port. Needs
/// `rest=1`; it has no auth and answers on localhost only.
const REST_CHAININFO: &str = "http://127.0.0.1:8332/rest/chaininfo.json";

/// The value's colour, as a palette slot so it follows the theme.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Accent,
    Auburn,
    Muted,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Row {
    pub caption: String,
    pub value: String,
    pub tone: Tone,
}

fn row(caption: &str, value: impl Into<String>, tone: Tone) -> Row {
    Row { caption: caption.to_string(), value: value.into(), tone }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServicesState {
    pub chain: Vec<Row>,
    pub net: Vec<Row>,
}

impl ServicesState {
    /// Before the first poll: captions with "—".
    pub fn starting() -> Self {
        let dash = |c: &str| row(c, "—", Tone::Muted);
        ServicesState {
            chain: vec![dash("BTCD"), dash("ELEC"), dash("BLOCK")],
            net: vec![dash("NGNX"), dash("TOR")],
        }
    }
}

// ---------------------------------------------------------------------------
// Turning probe output into rows (pure, tested)

/// systemd (ActiveState, SubState) -> row, as pong's _unit_state.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn unit_row(caption: &str, active: &str, sub: &str) -> Row {
    match active {
        "active" if sub == "running" => row(caption, "UP", Tone::Ok),
        "active" => {
            let s: String = sub.chars().take(5).collect::<String>().to_uppercase();
            row(caption, if s.is_empty() { "ACTIVE".into() } else { s }, Tone::Accent)
        }
        "failed" => row(caption, "FAIL", Tone::Auburn),
        "inactive" => row(caption, "OFF", Tone::Muted),
        _ => row(caption, "—", Tone::Muted),
    }
}

/// One line of `launchctl list`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Job {
    label: String,
    /// Set while the job is running.
    pid: Option<u32>,
    /// The last exit status; a negative number is the signal that ended it.
    status: i32,
}

/// `launchctl list` output ("PID\tStatus\tLabel" lines) -> jobs.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launchd_jobs(raw: &str) -> Vec<Job> {
    raw.lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let (pid, status, label) = (f.next()?, f.next()?, f.next()?);
            Some(Job {
                label: label.trim().to_string(),
                pid: pid.trim().parse().ok(),
                status: status.trim().parse().ok()?,
            })
        })
        .collect()
}

/// The job for a daemon: its label ends in `.<name>`, whoever wrote the
/// plist (`homebrew.mxcl.tor`, `org.bitcoin.bitcoind`). A running match wins
/// over a stopped one.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn find_job<'a>(jobs: &'a [Job], names: &[&str]) -> Option<&'a Job> {
    let hits = || {
        jobs.iter().filter(|j| {
            names.iter().any(|n| j.label.rsplit('.').next() == Some(n) && j.label.contains('.'))
        })
    };
    hits().find(|j| j.pid.is_some()).or_else(|| hits().next())
}

/// A launchd job (or, without one, a port check) -> row. A clean exit or
/// SIGTERM reads as stopped on purpose; any other exit as a failure.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn job_row(caption: &str, job: Option<&Job>, listening: Option<bool>) -> Row {
    match job {
        Some(Job { pid: Some(_), .. }) => row(caption, "UP", Tone::Ok),
        Some(Job { status: 0 | -15, .. }) => row(caption, "OFF", Tone::Muted),
        Some(_) => row(caption, "FAIL", Tone::Auburn),
        None => match listening {
            Some(true) => row(caption, "UP", Tone::Ok),
            Some(false) => row(caption, "OFF", Tone::Muted),
            None => row(caption, "—", Tone::Muted),
        },
    }
}

/// "932145" -> "932,145".
fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `getblockchaininfo` (CLI) or `chaininfo.json` (REST) -> the BLOCK row.
/// Both carry `blocks` and `verificationprogress`.
fn block_row(info: &str) -> Row {
    let parsed = serde_json::from_str::<Value>(info).ok().and_then(|v| {
        Some((v.get("blocks")?.as_u64()?, v.get("verificationprogress")?.as_f64()?))
    });
    match parsed {
        Some((blocks, progress)) => row(
            "BLOCK",
            thousands(blocks),
            if progress >= BTC_SYNCED { Tone::Ok } else { Tone::Accent },
        ),
        None => row("BLOCK", "—", Tone::Muted),
    }
}

// ---------------------------------------------------------------------------
// Probes

/// Run a short local command; trimmed stdout, or "" on any failure.
async fn probe(cmd: &str, args: &[&str], timeout: Duration) -> String {
    let mut command = Command::new(cmd);
    command.args(args).kill_on_drop(true);
    // An app opened from Finder gets PATH=/usr/bin:/bin:/usr/sbin:/sbin, with
    // neither Homebrew nor /usr/local/bin, where bitcoin-cli usually lives.
    #[cfg(target_os = "macos")]
    {
        let path = std::env::var("PATH").unwrap_or_default();
        command.env("PATH", format!("{path}:/opt/homebrew/bin:/usr/local/bin"));
    }
    match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(out)) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => String::new(),
    }
}

/// Chain info for the BLOCK row: `bitcoin-cli` with its default datadir and
/// cookie, else bitcoind's REST interface (for a node whose datadir is
/// elsewhere, so the CLI finds no cookie).
async fn chain_info() -> String {
    let cli = probe("bitcoin-cli", &["getblockchaininfo"], TIMEOUT).await;
    if !cli.is_empty() {
        return cli;
    }
    probe("curl", &["-sf", "--max-time", "3", REST_CHAININFO], TIMEOUT).await
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use super::*;

    /// (caption, systemd unit) for the four unit rows.
    const UNITS: [(&str, &str); 4] =
        [("BTCD", "bitcoind"), ("ELEC", "electrs"), ("NGNX", "nginx"), ("TOR", "tor@default")];

    async fn unit(caption: &str, unit: &str) -> Row {
        let active = probe("systemctl", &["is-active", unit], TIMEOUT).await;
        let sub = probe("systemctl", &["show", unit, "--property=SubState", "--value"], TIMEOUT).await;
        unit_row(caption, if active.is_empty() { "unknown" } else { &active }, &sub)
    }

    pub async fn fetch() -> ServicesState {
        let [btcd, elec, ngnx, tor] = UNITS;
        let (btcd, elec, ngnx, tor, info) = tokio::join!(
            unit(btcd.0, btcd.1),
            unit(elec.0, elec.1),
            unit(ngnx.0, ngnx.1),
            unit(tor.0, tor.1),
            chain_info(),
        );
        ServicesState {
            chain: vec![btcd, elec, block_row(&info)],
            net: vec![ngnx, tor],
        }
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use super::*;

    /// (caption, launchd label suffixes, default ports). The ports are only
    /// checked when no launchd job matches, e.g. bitcoind started by hand.
    const DAEMONS: [(&str, &[&str], &[u16]); 4] = [
        ("BTCD", &["bitcoind", "bitcoin"], &[8332, 8333]),
        ("ELEC", &["electrs"], &[50001]),
        ("NGNX", &["nginx"], &[]),
        ("TOR", &["tor"], &[9050]),
    ];

    /// Anything listening on any of these ports; None with no ports to check.
    async fn listening(ports: &[u16]) -> Option<bool> {
        if ports.is_empty() {
            return None;
        }
        for p in ports {
            let spec = format!("-iTCP:{p}");
            if !probe("lsof", &["-nP", &spec, "-sTCP:LISTEN"], TIMEOUT).await.is_empty() {
                return Some(true);
            }
        }
        Some(false)
    }

    async fn daemon(jobs: &[Job], (caption, names, ports): (&str, &[&str], &[u16])) -> Row {
        let job = find_job(jobs, names);
        let listening = if job.is_some() { None } else { listening(ports).await };
        job_row(caption, job, listening)
    }

    pub async fn fetch() -> ServicesState {
        let (list, info) = tokio::join!(probe("launchctl", &["list"], TIMEOUT), chain_info());
        let jobs = launchd_jobs(&list);
        let [btcd, elec, ngnx, tor] = DAEMONS;
        let (btcd, elec, ngnx, tor) = tokio::join!(
            daemon(&jobs, btcd),
            daemon(&jobs, elec),
            daemon(&jobs, ngnx),
            daemon(&jobs, tor),
        );
        ServicesState {
            chain: vec![btcd, elec, block_row(&info)],
            net: vec![ngnx, tor],
        }
    }
}

pub use backend::fetch;

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCKCHAININFO: &str = include_str!("../tests/fixtures/bitcoin-getblockchaininfo.json");
    const REST_CHAININFO: &str = include_str!("../tests/fixtures/bitcoin-rest-chaininfo.json");
    const LAUNCHCTL: &str = include_str!("../tests/fixtures/launchctl-list.txt");

    #[test]
    fn unit_states() {
        assert_eq!(unit_row("TOR", "active", "running"), row("TOR", "UP", Tone::Ok));
        assert_eq!(unit_row("TOR", "active", "exited"), row("TOR", "EXITE", Tone::Accent));
        assert_eq!(unit_row("TOR", "active", ""), row("TOR", "ACTIVE", Tone::Accent));
        assert_eq!(unit_row("TOR", "failed", "failed"), row("TOR", "FAIL", Tone::Auburn));
        assert_eq!(unit_row("TOR", "inactive", "dead"), row("TOR", "OFF", Tone::Muted));
        assert_eq!(unit_row("TOR", "unknown", ""), row("TOR", "—", Tone::Muted));
    }

    #[test]
    fn block_height() {
        assert_eq!(block_row(BLOCKCHAININFO), row("BLOCK", "932,145", Tone::Ok));
        let syncing = BLOCKCHAININFO.replace("0.99999812", "0.42");
        assert_eq!(block_row(&syncing).tone, Tone::Accent);
        assert_eq!(block_row(""), row("BLOCK", "—", Tone::Muted));
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn block_height_from_rest() {
        // REST gives a synced node's progress as a bare 1
        assert_eq!(block_row(REST_CHAININFO), row("BLOCK", "932,150", Tone::Ok));
    }

    #[test]
    fn launchd_list() {
        let jobs = launchd_jobs(LAUNCHCTL);
        assert_eq!(jobs.len(), 7, "header line skipped");
        assert_eq!(
            find_job(&jobs, &["bitcoind"]),
            Some(&Job { label: "org.example.bitcoind".into(), pid: Some(4101), status: 0 })
        );
        // a running match wins over a stopped one
        assert_eq!(find_job(&jobs, &["tor"]).unwrap().pid, Some(816));
        // the whole last label segment must match: "monitor" ends in "tor"
        // but is not tor
        assert!(find_job(&jobs, &["itor"]).is_none());
        assert!(find_job(&jobs, &["nginx"]).is_none());
    }

    #[test]
    fn launchd_rows() {
        let jobs = launchd_jobs(LAUNCHCTL);
        let r = |names: &[&str], listening| job_row("X", find_job(&jobs, names), listening);
        assert_eq!(r(&["bitcoind"], None), row("X", "UP", Tone::Ok));
        assert_eq!(r(&["electrs"], None), row("X", "OFF", Tone::Muted)); // SIGTERM
        assert_eq!(r(&["unbound"], None), row("X", "FAIL", Tone::Auburn));
        assert_eq!(r(&["mysql"], None), row("X", "OFF", Tone::Muted));
        // no job: the port check decides
        assert_eq!(r(&["nginx"], Some(true)), row("X", "UP", Tone::Ok));
        assert_eq!(r(&["nginx"], Some(false)), row("X", "OFF", Tone::Muted));
        assert_eq!(r(&["nginx"], None), row("X", "—", Tone::Muted));
    }

    #[test]
    fn starting_rows_match_both_cards() {
        let s = ServicesState::starting();
        let caps = |rows: &[Row]| rows.iter().map(|r| r.caption.clone()).collect::<Vec<_>>();
        assert_eq!(caps(&s.chain), ["BTCD", "ELEC", "BLOCK"]);
        assert_eq!(caps(&s.net), ["NGNX", "TOR"]);
    }
}
