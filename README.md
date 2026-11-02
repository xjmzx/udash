# udash

*Formerly **ding**, renamed in 0.2.0: Ubuntu ships an unrelated `ding`
package (a Tk dictionary), and apt replaced ding's .deb with it.*

A dashboard on one screen: clock, date, weather and air quality, two
calendars, and the status of the daemons on this machine. The successor to **pong**'s dashboard mode,
rebuilt as a Tauri app without the Pong animation. Same layout, same
fizx/upleb theme alternation.

**Status:** steps 1–3 of [docs/plan.md](docs/plan.md) are done: the app
shell, the data back end, and pong's dashboard layout — clock view and the
4-week calendar view (toggle at the top right), both themes. Packaging and
releases come next.

For layout work without the app, `npm run dev` in a plain browser shows the
dashboard with made-up data (`src/dev/mock.ts`); add `?theme=upleb` for the
other theme.

## Build and install

```
make deps       # once: npm install + cargo fetch
make dev        # run with hot reload
make check      # typecheck + cargo check
make install    # Linux: binary + .desktop + icon under ~/.local
make deb        # Linux: a .deb for another machine
./install.sh    # macOS: build the .app and install it to /Applications
```

### Releases

[`.github/workflows/release.yml`](.github/workflows/release.yml) builds a
`.deb` for Linux x86_64 and publishes it on a GitHub Release when a `v*` tag
is pushed (a tag with a hyphen, like `v0.2.0-beta.1`, is a pre-release). It
runs the Rust tests first. Set the version in all five files, commit, then
tag:

```
make version V=0.1.1
git commit -am "release: v0.1.1"
git tag v0.1.1 && git push origin main v0.1.1
```

It can also be run by hand from the Actions tab, naming an existing tag.
macOS and Windows builds are not in the workflow yet.

Debug builds keep their config and cache in `udash-dev` folders, so a dev run
never touches the installed app's state.

## Configuration

udash reads its config from the per-OS config folder, the same places pong
used, under `udash` instead of `pong`:

| OS | Config | Cache |
| --- | --- | --- |
| Linux | `$XDG_CONFIG_HOME/udash` or `~/.config/udash` | `$XDG_CACHE_HOME/udash` or `~/.cache/udash` |
| macOS | `~/Library/Application Support/udash` | `~/Library/Caches/udash` |
| Windows | `%APPDATA%\udash` | `%LOCALAPPDATA%\udash` |

On first run udash copies `calendars.json`, `weather.json` and `theme.json`
from the old `ding` config folder, then from pong's, for any file it does not
have yet. The old copies are left in place.

Everything private stays in those folders and never in this repo: calendar
URLs (treat them as passwords), cities, the theme choice and every cache
file. The daemon checks probe `localhost` only.

### Theme (`theme.json`)

```json
{ "mode": "alternate" }
```

`alternate` (the default) flips between fizx and upleb on each launch; the
first launch is fizx. `"fizx"` or `"upleb"` pins one.

## Command line

```
udash --list-cities   # the 162 bundled cities for weather.json
udash --check         # fetch everything once, print it as JSON, no window
```

`--check` reads the same config as the app (a debug build reads `udash-dev`).
Its output includes your calendar events, so treat it as private.

### Weather (`weather.json`)

Up to three cities; the first also drives sunrise/sunset and the 3-day
forecast. An entry is a name (`"Edinburgh"`), coordinates
(`"55.95,-3.19"`), or an object with `name`, `latitude` + `longitude` and
`label`. Bundled names resolve offline; other names are geocoded once via
Open-Meteo and cached. `"model"` optionally pins an Open-Meteo model.
Refreshed every 30 minutes.

### Calendars (`calendars.json`)

```json
{ "calendars": [
  { "name": "Jog", "url": "https://calendar.google.com/calendar/ical/.../basic.ics", "color": "pistachio" }
] }
```

`wash_priority` (optional) lists calendar names whose colour tints a day in
the week strip, highest first; other event days get the theme's mauve:
`"wash_priority": ["Leave", "Run"]`.

Google Calendar's private iCal address (Settings and sharing → Integrate
calendar → Secret address in iCal format). Treat it as a password. `color` is
a Google colour name or `#rrggbb`. Refreshed every 10 minutes; a calendar
whose fetch fails keeps its last events.

### Daemons

Linux: `systemctl` state for bitcoind, electrs, nginx and tor, and
bitcoind's block height via `bitcoin-cli`. macOS shows the same rows, read
from `launchctl list`: a job whose label ends in `.bitcoind`, `.electrs`,
`.nginx` or `.tor` (Homebrew's `homebrew.mxcl.tor` or a hand-written agent
alike). With no such job, bitcoind and electrs fall back to a check of their
default ports. Every 20 seconds, local only.

The block height comes from `bitcoin-cli getblockchaininfo` with its default
datadir. When that fails, for example because the node's datadir is elsewhere
and the CLI finds no cookie, udash asks bitcoind's REST interface
(`http://127.0.0.1:8332/rest/chaininfo.json`, needs `rest=1`). On macOS the
probes also search `/opt/homebrew/bin` and `/usr/local/bin`, which an app
opened from Finder does not have on its `PATH`.

## Keys

**Esc** or **Q** quits.
