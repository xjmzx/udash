# Changelog

## Unreleased

- macOS: the chain and network cards show the same rows as on Linux (BTCD,
  ELEC, BLOCK · NGNX, TOR), read from `launchctl list` instead of listing
  every `brew` service. A daemon with no launchd job falls back to a port
  check.
- BLOCK falls back to bitcoind's REST interface (`/rest/chaininfo.json`,
  `rest=1`) when `bitcoin-cli` cannot reach the node, on every OS.
- macOS: the daemon probes find `bitcoin-cli` in `/opt/homebrew/bin` and
  `/usr/local/bin` when udash is opened from Finder.

## 0.2.0 — 2026-10-28

- Renamed **ding → udash**: binary, package, `.desktop`, icon, config and
  cache folders (`udash`, `udash-dev`), identifier `uk.fizx.udash`. Ubuntu
  ships an unrelated `ding` package (a Tk dictionary, 1.9) whose higher
  version number made apt replace ding 0.1.0's .deb with it.
- First run copies `calendars.json`, `weather.json`, `theme.json` from the
  old `ding` config folder (then pong's, as before) for any file udash lacks.

## 0.1.0 — 2026-10-24

- App shell from dnsubs: Tauri 2, React 19, Tailwind v3.
- Per-OS config and cache folders (`ding`, `ding-dev` for debug builds).
- First-run import of pong's `calendars.json`, `weather.json`, `theme.json`.
- fizx/upleb themes, alternating per launch or pinned in `theme.json`.
- Single instance: a second launch focuses the running window.
- Esc / Q quit.
- Data back end ported from pong to Rust: Open-Meteo weather + air quality
  (162 bundled cities, geocode cache), Google Calendar ICS feeds (own ICS
  reader; repeats, exclusions, moved and cancelled instances, multi-day
  events), daemon status (systemd + bitcoind on Linux, brew on macOS).
- Calendar matches pong's per-day output on a real feed. Changes from pong:
  a failed fetch keeps the calendar's last events; all-day events and
  floating times are read in local time; cancelled events are left out.
- `ding --check` prints one fetch of everything as JSON; `ding --list-cities`.
- Dropped pong's openclaw port check (unused); the network card shows
  nginx and tor.
- Tailwind font families for pong's three faces: Ubuntu, Google Sans Flex,
  DejaVu Sans.
- Dashboard layout ported from pong on its 1920×1080 canvas, scaled to the
  window: mini-tile lattice (one element per tile), weekend brackets, group
  outlines, card tints, palette card, user@host card, colon-pinned clock,
  the five bottom-band cards, the week strip with day washes and event
  chips, the 4-week calendar view and its toggle, static paddles. Checked
  against offscreen pong renders of the same made-up data, both themes.
- Event chips use DejaVu Sans Condensed Bold, which is what pong's
  "dejavu sans" bold resolves to.
- `wash_priority` in calendars.json replaces pong's hard-coded calendar
  names for the day tints.
- Browser dev mode with made-up data (`src/dev/mock.ts`, `?theme=upleb`).
- Live cues, so the dashboard never looks frozen: the clock's colon blinks
  each second (lit for the first half), and the paddles rest on the
  lattice's baseline, the left one rising and falling on each minute
  change and the right one marking the clock face: full height on the
  hour, half on the half hour, a quarter on the quarters, a small bounce
  on other 5-minute marks. The clock now ticks on
  real second boundaries. Both respect reduced-motion.
- Bundled pong's clock face (static Google Sans Flex Bold, SIL OFL 1.1) for
  the clock, card values and date numerals. The clock stays one run of text
  so the font raises the colon between the digits, as in pong; the blink
  dims the colon with a mask.
- Release workflow: a `v*` tag runs the tests, builds the `.deb` (Linux
  x86_64) and publishes it on a GitHub Release, as dstack's does. macOS and
  Windows jobs to follow, in nping's shape.
