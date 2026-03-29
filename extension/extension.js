/*
  Vitals - Thin D-Bus client for GNOME Shell panel integration.
  Reads sensor data from the vitals-daemon D-Bus service.
*/

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const DBUS_NAME = 'com.corecoding.Vitals';
const DBUS_PATH = '/com/corecoding/Vitals';
const DBUS_IFACE = 'com.corecoding.Vitals.Sensors';

const VitalsDBusInterface = `
<node>
  <interface name="${DBUS_IFACE}">
    <method name="GetReadings">
      <arg type="a{s(sdss)}" direction="out"/>
    </method>
    <method name="GetTextReadings">
      <arg type="a{s(ssss)}" direction="out"/>
    </method>
    <signal name="ReadingsChanged">
      <arg type="a{s(sdss)}"/>
    </signal>
  </interface>
</node>`;

let vitalsMenu = null;

class VitalsMenuButton extends PanelMenu.Button {
    static {
        GObject.registerClass(this);
    }

    _init(extension) {
        super._init(0.5, 'Vitals');
        this._extension = extension;
        this._settings = extension.getSettings();
        this._proxy = null;
        this._readings = {};
        this._textReadings = {};

        // Panel display box
        this._panelBox = new St.BoxLayout({ style_class: 'panel-status-menu-box' });
        this.add_child(this._panelBox);

        // Status label
        this._label = new St.Label({
            text: 'Vitals',
            y_align: 2, // CENTER
        });
        this._panelBox.add_child(this._label);

        // Build menu
        this._buildMenu();

        // Connect to D-Bus
        this._connectDBus();

        // Poll timer
        this._timerId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT,
            this._settings.get_int('update-time'),
            () => {
                this._refresh();
                return GLib.SOURCE_CONTINUE;
            });
    }

    _connectDBus() {
        try {
            const ProxyWrapper = Gio.DBusProxy.makeProxyWrapper(VitalsDBusInterface);
            this._proxy = new ProxyWrapper(
                Gio.DBus.session,
                DBUS_NAME,
                DBUS_PATH,
                (proxy, error) => {
                    if (error) {
                        log(`Vitals: D-Bus connection error: ${error.message}`);
                        return;
                    }
                    this._refresh();
                }
            );
        } catch (e) {
            log(`Vitals: Failed to connect to D-Bus: ${e.message}`);
        }
    }

    _refresh() {
        if (!this._proxy) return;

        try {
            this._proxy.GetReadingsRemote((result) => {
                if (result && result[0]) {
                    this._readings = result[0];
                    this._updateMenu();
                    this._updatePanel();
                }
            });

            this._proxy.GetTextReadingsRemote((result) => {
                if (result && result[0]) {
                    this._textReadings = result[0];
                    this._updateMenu();
                }
            });
        } catch (e) {
            // D-Bus not available yet
        }
    }

    _buildMenu() {
        this.menu.removeAll();

        // Sensor groups will be populated dynamically
        this._menuGroups = {};

        // Bottom separator and controls
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        let controlsItem = new PopupMenu.PopupBaseMenuItem({ reactive: false });
        let controlsBox = new St.BoxLayout({ style_class: 'vitals-controls' });

        let refreshBtn = new St.Button({
            child: new St.Icon({ icon_name: 'view-refresh-symbolic', style_class: 'popup-menu-icon' }),
            style_class: 'button',
        });
        refreshBtn.connect('clicked', () => this._refresh());
        controlsBox.add_child(refreshBtn);

        controlsItem.add_child(controlsBox);
        this.menu.addMenuItem(controlsItem);
    }

    _updateMenu() {
        // Group readings by category
        let groups = {};

        for (let key in this._readings) {
            let [label, value, category, format] = this._readings[key];
            if (!(category in groups)) groups[category] = [];
            groups[category].push({ label, value: value.toString(), key });
        }

        for (let key in this._textReadings) {
            let [label, value, category, format] = this._textReadings[key];
            if (!(category in groups)) groups[category] = [];
            groups[category].push({ label, value, key });
        }

        // Update or create menu items for each group
        for (let category in groups) {
            if (!(category in this._menuGroups)) {
                let subMenu = new PopupMenu.PopupSubMenuMenuItem(category, true);
                this.menu.addMenuItem(subMenu, Object.keys(this._menuGroups).length);
                this._menuGroups[category] = { subMenu, items: {} };
            }

            let group = this._menuGroups[category];
            for (let reading of groups[category]) {
                if (reading.key in group.items) {
                    group.items[reading.key].label.text = `${reading.label}: ${reading.value}`;
                } else {
                    let item = new PopupMenu.PopupMenuItem(`${reading.label}: ${reading.value}`);
                    group.subMenu.menu.addMenuItem(item);
                    group.items[reading.key] = item;
                }
            }
        }
    }

    _updatePanel() {
        let hotSensors = this._settings.get_strv('hot-sensors');
        let displayParts = [];

        for (let hotKey of hotSensors) {
            if (hotKey in this._readings) {
                let [label, value, , format] = this._readings[hotKey];
                displayParts.push(`${value}`);
            }
        }

        this._label.text = displayParts.length > 0 ? displayParts.join(' | ') : 'Vitals';
    }

    destroy() {
        if (this._timerId) {
            GLib.source_remove(this._timerId);
            this._timerId = null;
        }
        this._proxy = null;
        super.destroy();
    }
}

export default class VitalsExtension extends Extension {
    enable() {
        vitalsMenu = new VitalsMenuButton(this);
        Main.panel.addToStatusArea('vitalsMenu', vitalsMenu, 1, 'right');
    }

    disable() {
        if (vitalsMenu) {
            vitalsMenu.destroy();
            vitalsMenu = null;
        }
    }
}
