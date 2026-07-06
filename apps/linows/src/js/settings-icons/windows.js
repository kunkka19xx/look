// Per-page Lucide icon for ms-settings:* URIs. Mirrors the catalog in
// apps/windows/LauncherApp/Services/SettingsIconCatalog.cs but maps to our
// inline Lucide SVGs in icons.js instead of Segoe Fluent Icons codepoints.
//
// Lucide chosen over Segoe because: cross-platform-safe (no Windows font
// dependency to surprise us later), already part of our icon vocabulary,
// inherits currentColor so it picks up the user's theme.
//
// Keys are the part after `ms-settings:` (e.g. "display", "easeofaccess-magnifier").
// Lookup with `getSettingsIcon(path)` - falls back to the gear if unmapped or
// if path doesn't start with `ms-settings:`.

import {
  activity,
  alertCircle,
  battery,
  bell,
  bluetooth,
  calendar,
  camera,
  cast,
  clock,
  code,
  cpu,
  download,
  edit3,
  eye,
  fileText,
  folderIcon,
  gamepad,
  globe,
  hardDrive,
  headphones,
  image,
  info,
  keyboard,
  languages,
  lock,
  mail,
  map,
  mapPin,
  mic,
  monitor,
  moon,
  mouse,
  palette,
  phone,
  plane,
  power,
  printer,
  radio,
  refreshCw,
  search,
  settingIcon,
  shieldCheck,
  sun,
  type as typeIcon,
  usb,
  users,
  volume2,
  wifi,
  zoomIn,
} from '../icons.js';

const CATALOG = {
  // Accounts
  yourinfo: users,
  emailandaccounts: mail,
  signinoptions: lock,
  'signinoptions-launchfaceenrollment': users,
  'signinoptions-launchfingerprintenrollment': lock,
  'signinoptions-dynamiclock': bluetooth,
  otherusers: users,
  workplace: users,
  sync: refreshCw,

  // Apps
  appsfeatures: code,
  defaultapps: code,
  startupapps: power,
  optionalfeatures: refreshCw,
  appsforwebsites: globe,
  maps: map,
  videoplayback: monitor,

  // Devices
  bluetooth: bluetooth,
  connecteddevices: usb,
  printers: printer,
  mousetouchpad: mouse,
  'devices-touchpad': mouse,
  'devices-touch': mouse,
  pen: edit3,
  typing: keyboard,
  autoplay: monitor,
  usb: usb,
  camera: camera,
  'mobile-devices': phone,

  // Network
  'network-status': wifi,
  'network-wifi': wifi,
  'network-wifisettings': wifi,
  'network-ethernet': wifi,
  'network-vpn': lock,
  'network-proxy': globe,
  'network-mobilehotspot': wifi,
  'network-airplanemode': plane,
  'network-cellular': phone,
  'network-dialup': phone,
  'network-advancedsettings': settingIcon,

  // Personalization
  personalization: palette,
  'personalization-background': image,
  'personalization-colors': palette,
  themes: palette,
  lockscreen: lock,
  'personalization-start': image,
  taskbar: image,
  fonts: typeIcon,
  'personalization-textinput': keyboard,
  'personalization-touchkeyboard': keyboard,
  'personalization-lighting': sun,

  // Privacy
  privacy: lock,
  'privacy-general': info,
  'privacy-accountinfo': users,
  'privacy-activityhistory': clock,
  'privacy-appdiagnostics': alertCircle,
  'privacy-automaticfiledownloads': download,
  'privacy-backgroundapps': code,
  'privacy-calendar': calendar,
  'privacy-callhistory': phone,
  'privacy-webcam': camera,
  'privacy-contacts': users,
  'privacy-documents': fileText,
  'privacy-downloadsfolder': download,
  'privacy-email': mail,
  'privacy-eyetracker': eye,
  'privacy-feedback': bell,
  'privacy-broadfilesystemaccess': folderIcon,
  'privacy-speechtyping': mic,
  'privacy-location': mapPin,
  'privacy-messaging': mail,
  'privacy-microphone': mic,
  'privacy-musiclibrary': headphones,
  'privacy-notifications': bell,
  'privacy-customdevices': usb,
  'privacy-phonecalls': phone,
  'privacy-pictures': image,
  'privacy-radios': radio,
  'privacy-speech': mic,
  'privacy-tasks': fileText,
  'privacy-videos': monitor,
  'privacy-voiceactivation': mic,

  // Sound
  sound: volume2,
  'sound-devices': headphones,
  'apps-volume': volume2,

  // System
  about: info,
  display: monitor,
  'display-advanced': monitor,
  nightlight: moon,
  notifications: bell,
  quiethours: moon,
  powersleep: power,
  batterysaver: battery,
  energyrecommendations: battery,
  storagesense: hardDrive,
  storagepolicies: hardDrive,
  storagerecommendations: hardDrive,
  savelocations: hardDrive,
  disksandvolumes: hardDrive,
  deviceencryption: lock,
  multitasking: monitor,
  clipboard: fileText,
  remotedesktop: monitor,
  project: cast,
  crossdevice: usb,
  presence: users,
  controlcenter: settingIcon,
  search: search,
  'search-permissions': lock,

  // Time and language
  dateandtime: clock,
  regionlanguage: languages,
  regionformatting: languages,
  keyboard: keyboard,
  speech: mic,

  // Accessibility
  'easeofaccess-display': monitor,
  'easeofaccess-visualeffects': eye,
  'easeofaccess-mousepointer': mouse,
  'easeofaccess-cursor': mouse,
  'easeofaccess-magnifier': zoomIn,
  'easeofaccess-colorfilter': palette,
  'easeofaccess-highcontrast': eye,
  'easeofaccess-narrator': headphones,
  'easeofaccess-audio': volume2,
  'easeofaccess-closedcaptioning': fileText,
  'easeofaccess-speechrecognition': mic,
  'easeofaccess-keyboard': keyboard,
  'easeofaccess-mouse': mouse,
  'easeofaccess-eyecontrol': eye,
  'easeofaccess-hearingaids': headphones,

  // Family
  'family-group': users,

  // Gaming
  'gaming-gamebar': gamepad,
  'gaming-gamedvr': gamepad,
  'gaming-gamemode': gamepad,

  // Update & security
  windowsupdate: refreshCw,
  'windowsupdate-history': clock,
  'windowsupdate-activehours': clock,
  'windowsupdate-options': settingIcon,
  'windowsupdate-optionalupdates': download,
  'windowsupdate-restartoptions': refreshCw,
  'delivery-optimization': globe,
  activation: lock,
  recovery: refreshCw,
  backup: hardDrive,
  troubleshoot: alertCircle,
  developers: code,
  findmydevice: mapPin,
  windowsdefender: shieldCheck,
  windowsinsider: code,
};

const SCHEME = 'ms-settings:';
const CMD_SCHEME = 'look-cmd://';

// Map control_panel.rs program names → Lucide SVG. Keep keys lowercased and
// in sync with the Rust catalog so every entry gets a category icon. The key
// is the part after `look-cmd://` up to (but not including) the optional `?`.
const CMD_CATALOG = {
  // rundll32 sysdm.cpl,EditEnvironmentVariables - special-case below
  'devmgmt.msc': cpu,
  'services.msc': settingIcon,
  'regedit.exe': code,
  'taskmgr.exe': activity,
  'eventvwr.msc': fileText,
  'diskmgmt.msc': hardDrive,
  'compmgmt.msc': settingIcon,
  'ncpa.cpl': wifi,
  'appwiz.cpl': download,
  'sysdm.cpl': info,
  'netplwiz.exe': users,
  'msconfig.exe': power,
  'msinfo32.exe': info,
  'resmon.exe': activity,
  'dxdiag.exe': monitor,
  'perfmon.msc': activity,
};

/**
 * Look up a Lucide SVG string for a Settings-style candidate path. Handles
 * both `ms-settings:` (Win11 Settings pages) and `look-cmd://` (classic
 * Control Panel applets emitted by core/engine/.../control_panel.rs).
 * Returns null when the path doesn't match either scheme so the caller can
 * fall back to the default icon pipeline. Unmapped keys fall back to the gear.
 */
export function getSettingsIcon(path) {
  if (typeof path !== 'string') return null;
  const lower = path.toLowerCase();

  if (lower.startsWith(SCHEME)) {
    const key = path.slice(SCHEME.length).toLowerCase();
    return CATALOG[key] || settingIcon;
  }

  if (lower.startsWith(CMD_SCHEME)) {
    const rest = path.slice(CMD_SCHEME.length);
    const qIdx = rest.indexOf('?');
    const program = (qIdx >= 0 ? rest.slice(0, qIdx) : rest).toLowerCase();
    const args = qIdx >= 0 ? rest.slice(qIdx + 1).toLowerCase() : '';
    // Env-vars dialog: rundll32 sysdm.cpl,EditEnvironmentVariables - show
    // a code icon since this is the path/PATH editor.
    if (program === 'rundll32.exe' && args.includes('editenvironmentvariables')) {
      return code;
    }
    return CMD_CATALOG[program] || settingIcon;
  }

  return null;
}
