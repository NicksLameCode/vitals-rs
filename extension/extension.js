/*
  Vitals (D-Bus Client) - Thin GNOME Shell extension that reads sensor
  data from the vitals-daemon D-Bus service and renders it in the panel.

  Matches the original Vitals extension's UI design language.
*/

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import { Extension, gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';

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
    <method name="GetTimeSeries">
      <arg type="s" name="key" direction="in"/>
      <arg type="a(dd)" direction="out"/>
    </method>
    <signal name="ReadingsChanged">
      <arg type="a{s(sdss)}"/>
    </signal>
  </interface>
</node>`;

// Sensor category metadata matching the original extension
const SENSOR_ICONS = {
    'temperature': { icon: 'temperature-symbolic.svg' },
    'voltage':     { icon: 'voltage-symbolic.svg' },
    'fan':         { icon: 'fan-symbolic.svg' },
    'memory':      { icon: 'memory-symbolic.svg' },
    'processor':   { icon: 'processor-symbolic.svg', fallback: 'cpu-symbolic.svg' },
    'system':      { icon: 'system-symbolic.svg' },
    'network':     { icon: 'network-symbolic.svg',
                     'icon-rx': 'network-download-symbolic.svg',
                     'icon-tx': 'network-upload-symbolic.svg' },
    'storage':     { icon: 'storage-symbolic.svg' },
    'battery':     { icon: 'battery-symbolic.svg' },
    'gpu':         { icon: 'gpu-symbolic.svg' },
};

const CATEGORY_ORDER = [
    'temperature', 'voltage', 'fan', 'memory', 'processor',
    'system', 'network', 'storage', 'battery',
];

const FORMAT_LABELS = {
    'percent': '%', 'temp': '\u00b0', 'fan': ' RPM', 'in': ' V',
    'hertz': ' Hz', 'memory': '', 'storage': '', 'speed': '/s',
    'load': '', 'watt': ' W', 'watt-gpu': ' W',
};

const GRAPH_BAR_COUNT = 60;
const GRAPH_HEIGHT = 80;
const GRAPH_BAR_WIDTH = 3;

let vitalsMenu = null;

// ---------- Custom MenuItem (icon + label + right-aligned value) ----------

const VitalsMenuItem = GObject.registerClass({
    Signals: { 'toggle': { param_types: [Clutter.Event.$gtype] } },
}, class VitalsMenuItem extends PopupMenu.PopupBaseMenuItem {
    _init(gicon, key, label, value, checked) {
        super._init({ reactive: true });
        this._checked = checked;
        this._key = key;
        this._gIcon = gicon;
        this._updateOrnament();

        // Icon
        this.add_child(new St.Icon({ style_class: 'popup-menu-icon', gicon: this._gIcon }));

        // Label
        this._labelActor = new St.Label({ text: label });
        this.add_child(this._labelActor);

        // Value (right-aligned)
        this._valueLabel = new St.Label({ text: value });
        this._valueLabel.set_x_align(Clutter.ActorAlign.END);
        this._valueLabel.set_x_expand(true);
        this._valueLabel.set_y_expand(true);
        this.add_child(this._valueLabel);
    }

    get checked() { return this._checked; }
    get key() { return this._key; }
    get gicon() { return this._gIcon; }
    get label() { return this._labelActor.text; }

    set value(v) { this._valueLabel.text = v; }
    get value() { return this._valueLabel.text; }

    activate(event) {
        this._checked = !this._checked;
        this._updateOrnament();
        this.emit('toggle', event);
    }

    _updateOrnament() {
        this.setOrnament(this._checked
            ? PopupMenu.Ornament.CHECK
            : PopupMenu.Ornament.NONE);
    }
});

// ---------- Main Panel Button ----------

class VitalsMenuButton extends PanelMenu.Button {
    static { GObject.registerClass(this); }

    _init(extension) {
        super._init(Clutter.ActorAlign.FILL);

        this._extension = extension;
        this._settings = extension.getSettings();
        this._proxy = null;
        this._readings = {};
        this._textReadings = {};
        this._sensorMenuItems = {};
        this._hotLabels = {};
        this._hotItems = {};
        this._groups = {};
        this._widths = {};

        this._iconPath = extension.path + '/icons/original/';

        // Panel layout (horizontal box with icon+value pairs)
        this._menuLayout = new St.BoxLayout({
            vertical: false,
            clip_to_allocation: true,
            x_align: Clutter.ActorAlign.START,
            y_align: Clutter.ActorAlign.CENTER,
            reactive: true,
            x_expand: true,
            style_class: 'vitals-panel-menu',
        });
        this.add_child(this._menuLayout);

        this._initializeMenu();
        this._drawPanelItems();
        this._connectDBus();

        // Poll timer
        let updateTime = this._settings.get_int('update-time');
        this._timerId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, updateTime, () => {
            this._refresh();
            return GLib.SOURCE_CONTINUE;
        });
    }

    // ---------- D-Bus ----------

    _connectDBus() {
        try {
            const ProxyWrapper = Gio.DBusProxy.makeProxyWrapper(VitalsDBusInterface);
            this._proxy = new ProxyWrapper(Gio.DBus.session, DBUS_NAME, DBUS_PATH,
                (proxy, error) => {
                    if (error) {
                        log(`Vitals: D-Bus connection error: ${error.message}`);
                        return;
                    }
                    this._refresh();
                });
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
                    this._updateDisplay();
                }
            });
            this._proxy.GetTextReadingsRemote((result) => {
                if (result && result[0]) {
                    this._textReadings = result[0];
                    this._updateDisplay();
                }
            });
        } catch (e) { /* D-Bus not available yet */ }
    }

    // ---------- Menu Structure ----------

    _initializeMenu() {
        // Create category submenus
        for (let category of CATEGORY_ORDER) {
            this._initializeMenuGroup(category);
        }
        // GPU groups (up to 4)
        for (let i = 1; i <= 4; i++)
            this._initializeMenuGroup('gpu#' + i, 'gpu');

        // Separator
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        // Bottom button row (compact round buttons, matching original)
        let item = new PopupMenu.PopupBaseMenuItem({
            reactive: false,
            style_class: 'vitals-menu-button-container',
        });

        let buttonBox = new St.BoxLayout({
            style_class: 'vitals-button-box',
            vertical: false,
            clip_to_allocation: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
            reactive: true,
            x_expand: true,
        });

        // Refresh
        let refreshBtn = this._createRoundButton('view-refresh-symbolic');
        refreshBtn.connect('clicked', () => this._refresh());
        buttonBox.add_child(refreshBtn);

        // System Monitor
        let monitorBtn = this._createRoundButton('org.gnome.SystemMonitor-symbolic');
        monitorBtn.connect('clicked', () => {
            this.menu.close();
            Util.spawn(this._settings.get_string('monitor-cmd').split(' '));
        });
        buttonBox.add_child(monitorBtn);

        // Preferences
        let prefsBtn = this._createRoundButton('preferences-system-symbolic');
        prefsBtn.connect('clicked', () => {
            this.menu.close();
            this._extension.openPreferences();
        });
        buttonBox.add_child(prefsBtn);

        item.add_child(buttonBox);
        this.menu.addMenuItem(item);

        // Refresh on menu open, hide graph on close
        this.menu.connect('open-state-changed', (_self, isOpen) => {
            if (isOpen) this._refresh();
            else this._hideGraphPopout();
        });
    }

    _initializeMenuGroup(groupName, iconCategory) {
        let cat = iconCategory || groupName;
        let displayName = this._ucFirst(groupName);
        let group = new PopupMenu.PopupSubMenuMenuItem(displayName, true);

        // Set category icon
        let iconFile = this._sensorIconPath(cat);
        if (iconFile)
            group.icon.gicon = Gio.icon_new_for_string(iconFile);

        // Status label (shows summary value, e.g. "45°C")
        group._statusLabel = new St.Label({
            text: '',
            y_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
            style: 'padding-left: 8px;',
        });
        let actor = group.actor ?? group;
        actor.insert_child_at_index(group._statusLabel, 4);

        // Hide until data arrives
        actor.hide();

        this._groups[groupName] = group;
        this.menu.addMenuItem(group);
    }

    _createRoundButton(iconName) {
        let button = new St.Button({
            style_class: 'message-list-clear-button button vitals-button-action',
        });
        button.child = new St.Icon({ icon_name: iconName });
        return button;
    }

    // ---------- Panel Items (hot sensors) ----------

    _drawPanelItems() {
        let hotSensors = this._settings.get_strv('hot-sensors');
        if (hotSensors.length === 0) {
            this._createHotItem('_default_icon_');
        } else {
            for (let key of hotSensors)
                this._createHotItem(key);
        }
    }

    _createHotItem(key, value) {
        let item = new St.BoxLayout({ style_class: 'vitals-panel-item' });
        this._hotItems[key] = item;
        this._menuLayout.add_child(item);

        // Add category icon
        if (!this._settings.get_boolean('hide-icons') || key === '_default_icon_') {
            let icon = this._defaultIcon(key);
            item.add_child(icon);
        }

        if (key === '_default_icon_') return;

        let label = new St.Label({
            style_class: 'vitals-panel-label',
            text: value || '\u2026',
            y_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        label.get_clutter_text().ellipsize = 0;
        this._hotLabels[key] = label;
        item.add_child(label);
    }

    _removeHotItem(key) {
        if (this._hotItems[key]) {
            this._hotItems[key].destroy();
            delete this._hotItems[key];
        }
        delete this._hotLabels[key];
        delete this._widths[key];
    }

    _defaultIcon(key) {
        let category = this._categoryFromKey(key);
        let iconStyle = 'vitals-panel-icon-' + (category || 'default');
        let iconPath = this._sensorIconPath(category || 'system');
        let gicon = iconPath ? Gio.icon_new_for_string(iconPath) : null;

        return new St.Icon({
            gicon: gicon,
            style_class: 'system-status-icon ' + iconStyle,
        });
    }

    // ---------- Display Update ----------

    _updateDisplay() {
        // Merge numeric and text readings
        let allReadings = {};

        for (let key in this._readings) {
            let [label, value, category, format] = this._readings[key];
            allReadings[key] = { label, value: this._formatValue(value, format), rawValue: value, category, format };
        }
        for (let key in this._textReadings) {
            let [label, value, category, format] = this._textReadings[key];
            allReadings[key] = { label, value, rawValue: value, category, format };
        }

        for (let key in allReadings) {
            let { label, value, category, format } = allReadings[key];

            // Skip group summary entries (they update the group header)
            if (category.endsWith('-group')) {
                let groupName = category.replace('-group', '');
                if (this._groups[groupName]) {
                    this._groups[groupName]._statusLabel.text = value;
                    let actor = this._groups[groupName].actor ?? this._groups[groupName];
                    actor.show();
                }
                continue;
            }

            // Show the group container
            let groupName = category;
            if (this._groups[groupName]) {
                let actor = this._groups[groupName].actor ?? this._groups[groupName];
                actor.show();
            }

            // Update existing menu item or create new one
            if (this._sensorMenuItems[key]) {
                this._sensorMenuItems[key].value = value;
            } else {
                this._appendMenuItem(key, label, value, category, format);
            }

            // Update panel hot label
            if (this._hotLabels[key]) {
                this._hotLabels[key].set_text(value);

                if (this._settings.get_boolean('fixed-widths')) {
                    let w = this._hotLabels[key].get_clutter_text().width;
                    if (!this._widths[key] || w > this._widths[key]) {
                        this._hotLabels[key].set_width(w);
                        this._widths[key] = w;
                    }
                }
            }
        }
    }

    _appendMenuItem(key, label, value, category, format) {
        let group = this._groups[category];
        if (!group) return;

        let iconName = SENSOR_ICONS[category.replace(/#\d+$/, '')]?.icon || 'system-symbolic.svg';
        let iconPath = this._extension.path + '/icons/original/' + iconName;
        let gicon = null;
        try { gicon = Gio.icon_new_for_string(iconPath); } catch (e) {}

        let isHot = this._hotLabels[key] !== undefined;
        let item = new VitalsMenuItem(gicon, key, label, value, isHot);

        item.connect('notify::hover', () => {
            if (item.hover)
                this._showGraphPopout(item, key, format);
            else
                this._hideGraphPopout();
        });

        item.connect('toggle', (self) => {
            let hotSensors = this._settings.get_strv('hot-sensors');
            if (self.checked) {
                hotSensors.push(self.key);
                this._createHotItem(self.key, self.value);
            } else {
                hotSensors.splice(hotSensors.indexOf(self.key), 1);
                this._removeHotItem(self.key);
            }
            if (hotSensors.length <= 0) {
                hotSensors.push('_default_icon_');
                this._createHotItem('_default_icon_');
            } else {
                let idx = hotSensors.indexOf('_default_icon_');
                if (idx >= 0) {
                    hotSensors.splice(idx, 1);
                    this._removeHotItem('_default_icon_');
                }
            }
            this._settings.set_strv('hot-sensors', hotSensors.filter(
                (item, pos, arr) => arr.indexOf(item) === pos));
        });

        // Alphabetize
        if (this._settings.get_boolean('alphabetize')) {
            let menuItems = group.menu._getMenuItems();
            let pos = menuItems.length;
            for (let i = 0; i < menuItems.length; i++) {
                if (menuItems[i].label &&
                    menuItems[i].label.localeCompare(label, undefined, { numeric: true, sensitivity: 'base' }) > 0) {
                    pos = i;
                    break;
                }
            }
            group.menu.addMenuItem(item, pos);
        } else {
            group.menu.addMenuItem(item);
        }

        this._sensorMenuItems[key] = item;
    }

    // ---------- History Graph Popout ----------

    _showGraphPopout(menuItem, key, format) {
        if (!this._settings.get_boolean('show-sensor-history-graph'))
            return;
        if (!this._proxy)
            return;

        this._hideGraphPopout();

        // Generation counter to discard stale D-Bus callbacks after hover-out
        this._graphGeneration = (this._graphGeneration || 0) + 1;
        let gen = this._graphGeneration;

        this._proxy.GetTimeSeriesRemote(key, (result) => {
            if (gen !== this._graphGeneration)
                return;
            if (!result || !result[0] || result[0].length === 0)
                return;

            let points = result[0]; // array of [timestamp, value] pairs

            // Extract numeric values from GVariant tuples
            let rawVals = points.map(p => {
                let v = p.deep_unpack ? p.deep_unpack() : p;
                return Array.isArray(v) ? v[1] : v;
            });

            // Apply format-specific scaling for display
            rawVals = rawVals.map(v => {
                switch (format) {
                    case 'temp': return v / 1000;
                    case 'in': return v / 1000;
                    case 'watt': return v / 1000000;
                    case 'percent': return v * 100;
                    default: return v;
                }
            });

            // Take last GRAPH_BAR_COUNT values, or pad front with zeros
            let bars;
            if (rawVals.length >= GRAPH_BAR_COUNT) {
                bars = rawVals.slice(rawVals.length - GRAPH_BAR_COUNT);
            } else {
                bars = new Array(GRAPH_BAR_COUNT - rawVals.length).fill(0).concat(rawVals);
            }

            let min = Math.min(...bars.filter(v => v > 0));
            let max = Math.max(...bars);
            if (!isFinite(min)) min = 0;
            if (!isFinite(max) || max === min) max = min + 1;

            // Build the popout widget
            let popout = new St.BoxLayout({
                vertical: true,
                style_class: 'vitals-history-popout',
            });

            // Title label
            let title = new St.Label({
                text: menuItem.label,
                style_class: 'vitals-history-popout-label',
            });
            popout.add_child(title);

            // Graph area: Y-axis + bars
            let graphRow = new St.BoxLayout({
                vertical: false,
                style_class: 'vitals-history-graph-row',
            });

            // Y-axis labels
            let yAxis = new St.BoxLayout({
                vertical: true,
                style_class: 'vitals-history-y-axis',
                y_expand: true,
            });
            let maxLabel = new St.Label({
                text: this._shortNum(max, format),
                style_class: 'vitals-history-popout-axis',
                y_align: Clutter.ActorAlign.START,
            });
            let minLabel = new St.Label({
                text: this._shortNum(min, format),
                style_class: 'vitals-history-popout-axis',
                y_align: Clutter.ActorAlign.END,
                y_expand: true,
            });
            yAxis.add_child(maxLabel);
            yAxis.add_child(minLabel);
            graphRow.add_child(yAxis);

            // Bar container
            let graphBox = new St.BoxLayout({
                vertical: false,
                style_class: 'vitals-history-graph',
                y_expand: true,
                clip_to_allocation: true,
            });
            let barsBox = new St.BoxLayout({
                vertical: false,
                style_class: 'vitals-history-graph-bars',
                y_align: Clutter.ActorAlign.END,
                y_expand: true,
            });

            for (let v of bars) {
                let frac = (v - min) / (max - min);
                if (!isFinite(frac) || frac < 0) frac = 0;
                let h = Math.max(1, Math.round(frac * GRAPH_HEIGHT));
                let bar = new St.Widget({
                    style_class: 'vitals-history-graph-bar',
                    width: GRAPH_BAR_WIDTH,
                    height: h,
                });
                barsBox.add_child(bar);
            }
            graphBox.add_child(barsBox);
            graphRow.add_child(graphBox);
            popout.add_child(graphRow);

            // X-axis labels (time range)
            let xWrap = new St.BoxLayout({
                vertical: false,
                style_class: 'vitals-history-x-wrap',
            });
            let xSpacer = new St.Widget({ style_class: 'vitals-history-x-spacer' });
            xWrap.add_child(xSpacer);

            let xRow = new St.BoxLayout({
                vertical: false,
                style_class: 'vitals-history-x-row',
                x_expand: true,
            });
            let duration = this._settings.get_int('sensor-history-duration');
            let minsAgo = Math.round(duration / 60);
            xRow.add_child(new St.Label({
                text: minsAgo + 'm ago',
                style_class: 'vitals-history-popout-axis',
                x_align: Clutter.ActorAlign.START,
            }));
            xRow.add_child(new St.Label({
                text: 'now',
                style_class: 'vitals-history-popout-axis',
                x_align: Clutter.ActorAlign.END,
                x_expand: true,
            }));
            xWrap.add_child(xRow);
            popout.add_child(xWrap);

            // Position popout to the left of the menu.
            // Compute width from constants since the widget isn't laid out yet.
            let popoutWidth = GRAPH_BAR_COUNT * GRAPH_BAR_WIDTH + 80; // bars + y-axis + padding
            let menuActor = this.menu.actor ?? this.menu;
            let [menuX] = menuActor.get_transformed_position();
            let [, itemY] = menuItem.get_transformed_position();

            this._graphPopout = popout;
            Main.layoutManager.addChrome(popout);
            popout.set_position(Math.max(0, Math.round(menuX - popoutWidth - 6)),
                                Math.round(itemY));
        });
    }

    _hideGraphPopout() {
        this._graphGeneration = (this._graphGeneration || 0) + 1;
        if (this._graphPopout) {
            Main.layoutManager.removeChrome(this._graphPopout);
            this._graphPopout.destroy();
            this._graphPopout = null;
        }
    }

    _shortNum(value, format) {
        switch (format) {
            case 'temp': {
                let suffix = '\u00b0C';
                let v = value;
                if (this._settings.get_int('unit') === 1) {
                    v = (9 / 5) * v + 32;
                    suffix = '\u00b0F';
                }
                return Math.round(v) + suffix;
            }
            case 'percent': return Math.round(value) + '%';
            case 'fan': return Math.round(value) + ' RPM';
            case 'in': return value.toFixed(1) + ' V';
            case 'watt': return value.toFixed(1) + ' W';
            case 'load': return value.toFixed(1);
            default: {
                if (Math.abs(value) >= 1e9) return (value / 1e9).toFixed(1) + 'G';
                if (Math.abs(value) >= 1e6) return (value / 1e6).toFixed(1) + 'M';
                if (Math.abs(value) >= 1e3) return (value / 1e3).toFixed(1) + 'K';
                return Math.round(value).toString();
            }
        }
    }

    // ---------- Value Formatting ----------

    _formatValue(rawValue, format) {
        let value = rawValue;
        let unit = 1000;
        let hp = this._settings.get_boolean('use-higher-precision');

        switch (format) {
            case 'percent':
                value = Math.min(value * 100, 100);
                return hp ? value.toFixed(1) + '%' : Math.round(value) + '%';
            case 'temp': {
                value = value / 1000;
                let suffix = '\u00b0C';
                if (this._settings.get_int('unit') === 1) {
                    value = (9 / 5) * value + 32;
                    suffix = '\u00b0F';
                }
                return hp ? value.toFixed(1) + suffix : Math.round(value) + suffix;
            }
            case 'fan':
                return Math.round(value) + ' RPM';
            case 'in':
                value = value / 1000;
                return (value >= 0 ? '+' : '') + (hp ? value.toFixed(2) : value.toFixed(1)) + ' V';
            case 'hertz':
                return this._scaleUnit(value, 1000, ['Hz', 'KHz', 'MHz', 'GHz', 'THz'], hp ? 2 : 1);
            case 'memory':
                return this._scaleMemory(value, this._settings.get_int('memory-measurement'), hp);
            case 'storage':
                return this._scaleStorage(value, this._settings.get_int('storage-measurement'), hp);
            case 'speed': {
                let bps = this._settings.get_int('network-speed-format') === 1;
                let v = bps ? value * 8 : value;
                let units = ['B', 'KB', 'MB', 'GB', 'TB'];
                let suffix = bps ? 'bps' : '/s';
                if (v <= 0) return '0 ' + (bps ? 'bps' : 'B/s');
                let exp = Math.floor(Math.log(v) / Math.log(1000));
                exp = Math.min(exp, units.length - 1);
                v = v / Math.pow(1000, exp);
                let u = bps ? units[exp].replace('B', 'bps') : units[exp] + '/s';
                return (hp ? v.toFixed(1) : Math.round(v)) + ' ' + u;
            }
            case 'uptime':
            case 'runtime':
                return this._formatDuration(value, format !== 'runtime' && (hp || value < 60));
            case 'watt':
                value = value / 1000000;
                return (value > 0 ? '+' : '') + (hp ? value.toFixed(2) : value.toFixed(1)) + ' W';
            case 'watt-gpu':
                return (hp ? value.toFixed(2) : value.toFixed(1)) + ' W';
            case 'watt-hour':
                value = value / 1000000;
                return (hp ? value.toFixed(2) : value.toFixed(1)) + ' Wh';
            case 'milliamp':
                return (hp ? (value / 1000).toFixed(1) : Math.round(value / 1000)) + ' mA';
            case 'milliamp-hour':
                return (hp ? (value / 1000).toFixed(1) : Math.round(value / 1000)) + ' mAh';
            case 'load':
                return hp ? value.toFixed(2) : value.toFixed(1);
            default:
                return String(value);
        }
    }

    _scaleUnit(value, base, units, decimals) {
        if (value <= 0) return '0 ' + units[0];
        let exp = Math.floor(Math.log(value) / Math.log(base));
        exp = Math.min(exp, units.length - 1);
        let v = value / Math.pow(base, exp);
        return v.toFixed(decimals) + ' ' + units[exp];
    }

    _scaleMemory(value, measurement, hp) {
        let base = measurement ? 1000 : 1024;
        let units = measurement
            ? ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
            : ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
        let v = value * base;
        if (v <= 0) return '0 ' + units[0];
        let exp = Math.floor(Math.log(v) / Math.log(base));
        exp = Math.min(exp, units.length - 1);
        v = v / Math.pow(base, exp);
        return (hp ? v.toFixed(2) : v.toFixed(1)) + ' ' + units[exp];
    }

    _scaleStorage(value, measurement, hp) {
        let base = measurement ? 1000 : 1024;
        let units = measurement
            ? ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
            : ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];
        if (value <= 0) return '0 ' + units[0];
        let exp = Math.floor(Math.log(value) / Math.log(base));
        exp = Math.min(exp, units.length - 1);
        let v = value / Math.pow(base, exp);
        return (hp ? v.toFixed(2) : v.toFixed(1)) + ' ' + units[exp];
    }

    _formatDuration(seconds, showSecs) {
        let s = Math.round(Math.abs(seconds));
        let d = Math.floor(s / 86400);
        let h = Math.floor((s % 86400) / 3600);
        let m = Math.floor((s % 3600) / 60);
        let sec = s % 60;
        let parts = [];
        if (d > 0) parts.push(d + 'd');
        if (h > 0) parts.push(h + 'h');
        if (m > 0) parts.push(m + 'm');
        if (showSecs && (sec > 0 || parts.length === 0)) parts.push(sec + 's');
        return parts.join(' ') || '0s';
    }

    // ---------- Helpers ----------

    _sensorIconPath(category, iconKey) {
        iconKey = iconKey || 'icon';
        let cat = category.replace(/#\d+$/, '');
        let meta = SENSOR_ICONS[cat];
        if (!meta) return null;
        let filename = meta[iconKey] || meta.icon;
        let path = this._extension.path + '/icons/original/' + filename;
        try {
            if (GLib.file_test(path, GLib.FileTest.EXISTS)) return path;
        } catch (e) {}
        // Try fallback
        if (meta.fallback) {
            path = this._extension.path + '/icons/original/' + meta.fallback;
            try { if (GLib.file_test(path, GLib.FileTest.EXISTS)) return path; } catch (e) {}
        }
        return null;
    }

    _categoryFromKey(key) {
        for (let cat of [...CATEGORY_ORDER, 'gpu']) {
            if (key.includes('_' + cat + '_') || key.includes('_' + cat + '#'))
                return cat;
        }
        // Check for network sub-types
        if (key.includes('network')) return 'network';
        return null;
    }

    _ucFirst(str) {
        let name = str.replace(/#\d+$/, '');
        if (name === 'gpu') return 'GPU' + str.replace('gpu', '');
        return name.charAt(0).toUpperCase() + name.slice(1);
    }

    // ---------- Cleanup ----------

    destroy() {
        this._hideGraphPopout();
        if (this._timerId) {
            GLib.source_remove(this._timerId);
            this._timerId = null;
        }
        this._proxy = null;
        super.destroy();
    }
}

// ---------- Extension Entry Point ----------

export default class VitalsExtension extends Extension {
    enable() {
        // Copy icons from data dir if not already present
        let iconsDir = this.path + '/icons';
        if (!GLib.file_test(iconsDir, GLib.FileTest.IS_DIR)) {
            // Icons should be bundled with the extension
            log('Vitals: icons directory not found at ' + iconsDir);
        }

        vitalsMenu = new VitalsMenuButton(this);
        let position = this._positionInPanel();
        Main.panel.addToStatusArea('vitalsMenu', vitalsMenu, position[1], position[0]);
    }

    disable() {
        if (vitalsMenu) {
            vitalsMenu.destroy();
            vitalsMenu = null;
        }
    }

    _positionInPanel() {
        let start = this.getSettings().get_int('position-in-panel');
        let positions = ['left', 'center', 'right'];
        let position = positions[start < positions.length ? start : 2];
        let index = position === 'right' ? 0 : -1;
        return [position, index];
    }
}
