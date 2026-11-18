// The overlay of Soplos Gaming Panel as a widget of GNOME.
//
// On Wayland a program cannot place its own window, and GNOME does not speak
// the layer-shell protocol the other desktops use for their panels. In GNOME
// the Shell is the window manager, so this extension does what a panel needs:
// it keeps the window of the overlay above the others, on every workspace, in
// the corner or against the edge the settings of the panel ask for, and puts
// it there again when those settings change.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

// The title the panel gives the window of the overlay.
const TITLE = 'soplos-gaming-overlay';
// The space between the small card and the edges of the screen.
const MARGIN = 24;

export default class SoplosGamingOverlay extends Extension {
    enable() {
        this._watched = new Map();
        this._created = global.display.connect('window-created', (_display, window) => this._watch(window));
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);

        // The panel writes its settings whole, through a temporary file, so
        // the folder is watched rather than the file.
        const folder = Gio.File.new_for_path(
            GLib.build_filenamev([GLib.get_user_config_dir(), 'soplos-gaming-panel']));
        try {
            this._monitor = folder.monitor_directory(Gio.FileMonitorFlags.NONE, null);
            this._monitorId = this._monitor.connect('changed', (_monitor, file) => {
                if (file.get_basename() === 'config.json')
                    this._placeAll();
            });
        } catch (error) {
            this._monitor = null;
        }
    }

    disable() {
        global.display.disconnect(this._created);
        for (const [window, ids] of this._watched) {
            for (const id of ids)
                window.disconnect(id);
        }
        this._watched.clear();
        if (this._monitor) {
            this._monitor.disconnect(this._monitorId);
            this._monitor.cancel();
            this._monitor = null;
        }
        if (this._later) {
            GLib.source_remove(this._later);
            this._later = 0;
        }
    }

    _watch(window) {
        if (!window || this._watched.has(window))
            return;
        // The title can arrive after the window: it is looked at again then.
        const check = () => {
            if (window.get_title() === TITLE)
                this._placeSoon(window);
        };
        const ids = [
            window.connect('notify::title', check),
            window.connect('unmanaged', () => {
                for (const id of this._watched.get(window) ?? [])
                    window.disconnect(id);
                this._watched.delete(window);
            }),
        ];
        this._watched.set(window, ids);
        check();
    }

    _placeAll() {
        for (const window of this._watched.keys()) {
            if (window.get_title() === TITLE)
                this._place(window);
        }
    }

    // Once now and once when the window has been mapped and sized by its
    // program, which may move it after the first frame.
    _placeSoon(window) {
        this._place(window);
        if (this._later)
            GLib.source_remove(this._later);
        this._later = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
            this._later = 0;
            if (this._watched.has(window))
                this._place(window);
            return GLib.SOURCE_REMOVE;
        });
    }

    // What the settings of the panel ask for. A file that is missing or broken
    // gives the defaults of the panel.
    _settings() {
        const hud = {mode: 'simple', corner: 'top-left', scale: 100};
        try {
            const path = GLib.build_filenamev(
                [GLib.get_user_config_dir(), 'soplos-gaming-panel', 'config.json']);
            const [, bytes] = GLib.file_get_contents(path);
            const saved = JSON.parse(new TextDecoder().decode(bytes)).hud ?? {};
            if (saved.mode === 'simple' || saved.mode === 'advanced')
                hud.mode = saved.mode;
            if (['top-left', 'top-right', 'bottom-left', 'bottom-right'].includes(saved.corner))
                hud.corner = saved.corner;
            if (Number.isFinite(saved.scale))
                hud.scale = Math.min(200, Math.max(50, saved.scale));
        } catch (error) {
            // The defaults stay.
        }
        return hud;
    }

    _place(window) {
        const hud = this._settings();
        const area = window.get_work_area_current_monitor();
        const factor = hud.scale / 100;
        const right = hud.corner.endsWith('right');
        const bottom = hud.corner.startsWith('bottom');
        let width, height, x, y;
        if (hud.mode === 'advanced') {
            width = Math.min(Math.round(420 * factor), area.width);
            height = area.height;
            x = right ? area.x + area.width - width : area.x;
            y = area.y;
        } else {
            width = Math.min(Math.round(360 * factor), area.width);
            height = Math.min(Math.round(300 * factor), area.height);
            x = right ? area.x + area.width - width - MARGIN : area.x + MARGIN;
            y = bottom ? area.y + area.height - height - MARGIN : area.y + MARGIN;
        }
        if (!window.is_above())
            window.make_above();
        if (!window.is_on_all_workspaces())
            window.stick();
        window.move_resize_frame(false, x, y, width, height);
    }
}
