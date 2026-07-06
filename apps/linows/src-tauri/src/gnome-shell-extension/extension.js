import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const IFACE = `
<node>
  <interface name="com.look.ShellIntegration">
    <method name="FocusApp">
      <arg type="s" direction="in" name="desktop_id"/>
      <arg type="b" direction="out" name="success"/>
    </method>
    <method name="GetPointer">
      <arg type="i" direction="out" name="x"/>
      <arg type="i" direction="out" name="y"/>
    </method>
    <method name="ListWindowedApps">
      <arg type="as" direction="out" name="desktop_ids"/>
    </method>
  </interface>
</node>`;

export default class LookIntegration extends Extension {
    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this);
        this._dbus.export(Gio.DBus.session, '/com/look/ShellIntegration');
        this._owner = Gio.bus_own_name(
            Gio.BusType.SESSION,
            'com.look.ShellIntegration',
            Gio.BusNameOwnerFlags.NONE,
            null, null, null,
        );
    }

    disable() {
        if (this._dbus) {
            this._dbus.unexport();
            this._dbus = null;
        }
        if (this._owner) {
            Gio.bus_unown_name(this._owner);
            this._owner = null;
        }
    }

    GetPointer() {
        const [x, y] = global.get_pointer();
        return [x, y];
    }

    ListWindowedApps() {
        // Shell.AppSystem.get_running() puts an app in RUNNING state once it
        // owns any tracked window - including skip-taskbar / utility surfaces
        // some tray apps (flameshot, fcitx5 indicator) create. To match what
        // a user calls a "running app", we require at least one NORMAL window
        // that isn't skip-taskbar, matching GNOME's own window switcher.
        const appSys = Shell.AppSystem.get_default();
        const ids = [];
        for (const app of appSys.get_running()) {
            const wins = app.get_windows();
            const hasSwitchableWindow = wins.some(w => {
                if (w.get_window_type() !== Meta.WindowType.NORMAL)
                    return false;
                if (w.is_skip_taskbar())
                    return false;
                return true;
            });
            if (hasSwitchableWindow)
                ids.push(app.get_id());
        }
        return ids;
    }

    FocusApp(desktop_id) {
        if (!desktop_id.endsWith('.desktop'))
            desktop_id += '.desktop';

        const appSys = Shell.AppSystem.get_default();
        const app = appSys.lookup_app(desktop_id);

        if (app && app.get_n_windows() > 0) {
            // Use activate_window with current timestamp to bypass
            // Mutter's focus-stealing prevention (same as Activities does)
            const wins = app.get_windows();
            const mostRecent = wins[0];
            if (mostRecent) {
                const workspace = mostRecent.get_workspace();
                if (workspace)
                    workspace.activate_with_focus(mostRecent, global.get_current_time());
                else
                    mostRecent.activate(global.get_current_time());
            }
            return true;
        }
        return false;
    }
}
