º# Changelog

All notable changes to Soplos Gaming Panel are documented in this file.

## [3.0.0] - 2026-11-18

First stable release.

### Added

- Rewrite in the stack of the 3.0 programs of Soplos Linux (Tauri 2, Svelte 4,
  TypeScript, Tailwind), replacing the Python prototype.
- Window without system frame, with the Soplos palette, light and dark themes
  that follow the desktop, and the eight languages of the other programs:
  English, Español, Deutsch, Français, Italiano, Português, Română and Русский.
- Tray icon that keeps the program alive with the window closed, and the menu
  that shows or hides the panel, turns the overlay on or off and quits.
- Global shortcuts: heard by the program itself on X11; registered in the
  desktop on Wayland (GNOME custom keybinding and Xfce keyboard shortcut).
- CLI flags `--toggle`, `--show` and `--hud-toggle`, used by the shortcuts and
  by a second launch.
- `config.json` with the choices of the user, written whole at a time, and the
  page of settings that changes the shortcuts, the session and the overlay.
- Overlay over the game in two shapes: a small card in a corner and a panel of
  the height of the screen. Its cells are ticked one by one: processor use,
  temperature and watts, clocks, graphics use, temperature and VRAM, memory,
  FPS, frametime and network. A file of the version before the list keeps the
  cells it had.
- MangoHud page that offers the keys of its file as boxes and fields, grouped
  in sections that open and close, adds a key by hand, and points MangoHud at a
  log of its own so the panel can read the frames per second. Comments and
  unknown keys of the file are never touched.
- Library of the games installed for the user, read from the desktop menu and
  from the manifests of Steam without opening a client: it starts them the way
  the menu does, keeps a cover the user chooses, and gives each one its profile
  and its scheduler. The time played comes from the daemon.
- Monitor with the use of the processor, the graphics card, the memory and the
  swap, the temperatures, the fans, the voltages and the watts, the network and
  the disks, the processes with the most load, and a chart of the last minute,
  five minutes, fifteen minutes or hour.
- GameMode control and supervision: profiles, the levers of the kernel with
  their value while a game runs, an automatic profile when a game starts, one
  profile per game, and the history and the total time per game.
- sched_ext schedulers switched over D-Bus (`org.scx.Loader`), with one
  scheduler per game and the previous one given back when the game closes.
- VCache of AMD as a lever, with the value each game asks for.
- One password for a whole administrator step, through pkexec and the polkit
  action of the package; the files it touches are copied first to
  `/var/lib/soplos-gaming-panel/backup`.
- Start with the session through `~/.config/autostart`.
- Fallback installation of GameMode and MangoHud, and of the overlay tools of
  Soplos Welcome, when they are missing.
- Debian packaging: desktop entry, AppStream metadata, manual page, polkit
  policy and systemd unit of the daemon.
- Five screenshots of the pages of the program, taken with the interface in
  English, in `src-tauri/resources/screenshots/`, in the README, in the
  AppStream metadata and in the package.

### Fixed

- Copyright: the file named `soplos-theme-manager` as the upstream package and
  pointed at its repository; it now names `soplos-gaming-panel`.
- Manual page: it documents the `--daemon` and `--daemon-restore` flags of the
  systemd unit, the covers of the library, the log of MangoHud and the folder
  of the backup, and its header carries the date of the release.
- The panel took itself for a game: its entry says `Game`, and its daemon is
  the same program, so the automatic profile went on as soon as the service
  started and never went off. The panel and the clients of the stores (Steam,
  Heroic, Lutris, Bottles, itch and others) are no longer games, neither for
  the daemon nor in the library.
- The games of Steam were never found while they ran: their program is not in
  any entry of the menu. They are found by the `reaper` Steam starts each game
  under, named after their manifest, and the profile given to them in the
  library is used.
- The systemd unit could not keep its state: `/run/soplos-gaming-panel` did not
  exist at boot and `/var/lib/soplos-gaming-panel` was read only under
  `ProtectSystem=strict`, so the automatic settings and the history were never
  saved. systemd now makes both (`RuntimeDirectory=`, `StateDirectory=`).
- The GameMode page broke as soon as a game was running (`scx.available()`
  on the plain data that comes from the daemon).
- Restoring the dirty page limits wrote `vm.dirty_bytes=0`, which the kernel
  refuses, and the state stayed stuck. The ratio of each pair is saved and
  restored instead.
- A profile turned on by hand went off three seconds later with the automatic
  profile on, choosing another profile while one was on did nothing, and a
  profile turned off by hand during a game came back by itself. The daemon now
  only takes away the profile it put on for a game, switches profiles by
  restoring first, and leaves a game turned off by hand alone until it ends.
- The CPU governor and the energy preference were written only to the first
  core; each core has its own cpufreq policy, and they are written to all.
  The mode of `amd_pstate` is no longer switched while the machine runs: that
  starts every policy again and the restore could not put them back in order.
- The disk scheduler was set on the first disk found, which could be a USB
  stick, and to `none` on hard disks too. Only internal disks without moving
  parts are used, and the udev rules of the tweak trigger only the disks.
- The daemon kept answering one client at a time with no time limit, and its
  socket stayed open to every user until the service restarted. Requests have
  a time limit and a size limit, and the socket goes to the user of the panel
  as soon as there is one.
- The frames per second of MangoHud were read from a log fixed when the
  program started, read whole every second, and with the names of the columns
  expected on its first line. The log is looked up every time, only its start
  and its end are read, and the names are looked for among the first lines.
- The library left out every game and emulator of Flatpak and took the command
  of an action of an entry for the one of the game. The program of a Flatpak
  comes from its `--command=`, variables before a program are skipped, and only
  the main group of an entry is read. The library shows only the games of the
  user that opens it.
- Eleven names of levers and the group of the network showed their key, and two
  errors came in Spanish; they are now in the eight languages and in English.
- The kernels page never found BTF (`DEBUG_INFO_BTF` without its `CONFIG_`).
- The game mode could not be turned off on machines with an energy
  preference (amd-pstate, intel_pstate): the restore put the preference back
  while the governor was still at `performance`, which those drivers refuse,
  so the restore failed and the profile stayed. The preference now goes before
  the governor, so it comes back after it, and whatever a restore cannot write
  is tried again once the rest is back, which also frees a state saved by the
  older order.
- An error of the GameMode page vanished at once: the reading of the daemon,
  every two seconds, wiped it. An error of something the user did now stays
  until the next action.

### Changed

- The overlay is a widget of the desktop, not a window. On Wayland it is a
  layer-shell surface (Plasma, and Xfce 4.22) anchored to its corner or its
  side, which no tiling takes; on GNOME, which does not speak layer-shell, the
  extension of the package (`soplos-gaming-overlay@soplos.org`) keeps it above,
  on every workspace and in its place, and Forge is told to let it float; on
  X11 it is marked as a panel, as before. New dependency: `libgtk-layer-shell0`.
- The games of Lutris, native or Flatpak, come from Lutris itself, with their
  covers, and start as Lutris starts them.
- When a game was started through GameMode, the automatic profile leaves the
  CPU governor to GameMode: both wrote it, and the one that finished last won.
- The global shortcuts are registered in Plasma on Wayland, as launchers with
  their combination in `kglobalshortcutsrc`, instead of an error.
- The footer shows the desktop and the session, as the other 3.0 programs.
- The machine is read only while the window or the overlay is shown, and the
  overlay asks the daemon for its state every five seconds instead of every
  second: it walked every entry and every process each time.
- Maintainer scripts: the service is stopped before the package is removed, so
  the levers go back to their values, restarted on an upgrade, and its state
  is removed on purge. New dependencies: `pciutils`, `x11-utils`, `xdotool`
  and `wmctrl`, which the overlay and the monitor use.
- The schedulers of sched_ext come with the panel: it depends on `scx-loader`,
  the D-Bus service it asks to start and switch them, and on `scx-scheds`, the
  schedulers. Debian packages neither; Soplos builds them (scx-deb). They only
  run on a kernel with sched_ext, the gaming kernels of Soplos.
