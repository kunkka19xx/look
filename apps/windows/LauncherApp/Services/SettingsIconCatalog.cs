using System.Collections.Generic;

namespace LauncherApp.Services;

// Per-page Segoe Fluent Icons glyph for ms-settings:* targets. Real per-page PNG icons
// aren't reliably exposed by Windows - Settings page art lives inside the SystemSettings
// package's protected resource bundles and isn't enumerable as ICON resources, so the
// IShellItemImageFactory path collapses every page to the same generic Settings icon.
// Mapping each curated catalog target to a Fluent glyph gives a per-category visual cue
// using the same icon font Windows itself ships, with no fragile resource extraction.
//
// Keys are the part after `ms-settings:` (e.g. "display", "easeofaccess-magnifier"). When
// a target isn't in this map, the row falls back to the default settings gear glyph.
//
// Code points: Segoe Fluent Icons reference
// https://learn.microsoft.com/en-us/windows/apps/design/style/segoe-fluent-icons-font
public static class SettingsIconCatalog
{
    public const string DefaultGlyph = ""; // Setting (gear)

    public static string GetGlyph(string? target)
    {
        if (string.IsNullOrWhiteSpace(target))
            return DefaultGlyph;

        string key = target;
        const string scheme = "ms-settings:";
        if (key.StartsWith(scheme, System.StringComparison.OrdinalIgnoreCase))
        {
            key = key.Substring(scheme.Length);
        }

        return GlyphByTarget.TryGetValue(key, out string? glyph) ? glyph : DefaultGlyph;
    }

    private static readonly Dictionary<string, string> GlyphByTarget = new(System.StringComparer.OrdinalIgnoreCase)
    {
        // ---------- Accounts ----------
        ["yourinfo"] = "",                                  // Contact
        ["emailandaccounts"] = "",                          // Mail
        ["signinoptions"] = "",                             // Lock
        ["signinoptions-launchfaceenrollment"] = "",        // Contact (face)
        ["signinoptions-launchfingerprintenrollment"] = "", // Fingerprint
        ["signinoptions-dynamiclock"] = "",                 // Bluetooth (proximity)
        ["otherusers"] = "",                                // People
        ["workplace"] = "",                                 // Work / Workplace
        ["sync"] = "",                                      // Sync

        // ---------- Apps ----------
        ["appsfeatures"] = "",                              // AllApps
        ["defaultapps"] = "",                               // OpenWith
        ["startupapps"] = "",                               // Power (boot)
        ["optionalfeatures"] = "",                          // Repair
        ["appsforwebsites"] = "",                           // Globe
        ["maps"] = "",                                      // Map
        ["videoplayback"] = "",                             // Video

        // ---------- Devices ----------
        ["bluetooth"] = "",                                 // Bluetooth
        ["connecteddevices"] = "",                          // Devices
        ["printers"] = "",                                  // Print
        ["mousetouchpad"] = "",                             // Mouse
        ["devices-touchpad"] = "",                          // Mouse (closest)
        ["devices-touch"] = "",                             // Touch (TouchPointer)
        ["pen"] = "",                                       // Edit (pen)
        ["typing"] = "",                                    // Keyboard
        ["autoplay"] = "",                                  // Play / video
        ["usb"] = "",                                       // USB
        ["camera"] = "",                                    // Camera
        ["mobile-devices"] = "",                            // CellPhone

        // ---------- Network ----------
        ["network-status"] = "",                            // NetworkConnected
        ["network-wifi"] = "",                              // Wifi
        ["network-wifisettings"] = "",                      // Wifi
        ["network-ethernet"] = "",                          // NetworkConnected
        ["network-vpn"] = "",                               // Lock (VPN)
        ["network-proxy"] = "",                             // NetworkAdapter
        ["network-mobilehotspot"] = "",                     // CellularData
        ["network-airplanemode"] = "",                      // Airplane
        ["network-cellular"] = "",                          // SignalBars
        ["network-dialup"] = "",                            // Phone
        ["network-advancedsettings"] = "",                  // Setting

        // ---------- Personalization ----------
        ["personalization"] = "",                           // Color
        ["personalization-background"] = "",                // Picture
        ["personalization-colors"] = "",                    // Color
        ["themes"] = "",                                    // Color (themes)
        ["lockscreen"] = "",                                // Lock
        ["personalization-start"] = "",                     // Tiles
        ["taskbar"] = "",                                   // AllApps (taskbar)
        ["fonts"] = "",                                     // FontColor
        ["personalization-textinput"] = "",                 // Keyboard
        ["personalization-touchkeyboard"] = "",             // Keyboard
        ["personalization-lighting"] = "",                  // Lightbulb

        // ---------- Privacy ----------
        ["privacy"] = "",                                   // Lock
        ["privacy-general"] = "",                           // Info
        ["privacy-accountinfo"] = "",                       // Contact
        ["privacy-activityhistory"] = "",                   // History
        ["privacy-appdiagnostics"] = "",                    // Diagnostic
        ["privacy-automaticfiledownloads"] = "",            // Download
        ["privacy-backgroundapps"] = "",                    // AllApps
        ["privacy-calendar"] = "",                          // Calendar
        ["privacy-callhistory"] = "",                       // Phone
        ["privacy-webcam"] = "",                            // Camera
        ["privacy-contacts"] = "",                          // People
        ["privacy-documents"] = "",                         // Document
        ["privacy-downloadsfolder"] = "",                   // Download
        ["privacy-email"] = "",                             // Mail
        ["privacy-eyetracker"] = "",                        // RedEye
        ["privacy-feedback"] = "",                          // Feedback
        ["privacy-broadfilesystemaccess"] = "",             // Folder
        ["privacy-speechtyping"] = "",                      // Inking (CC alt)
        ["privacy-location"] = "",                          // MapPin
        ["privacy-messaging"] = "",                         // Message
        ["privacy-microphone"] = "",                        // Microphone
        ["privacy-musiclibrary"] = "",                      // MusicNote
        ["privacy-notifications"] = "",                     // Ringer
        ["privacy-customdevices"] = "",                     // Devices
        ["privacy-phonecalls"] = "",                        // Phone
        ["privacy-pictures"] = "",                          // Picture
        ["privacy-radios"] = "",                            // Radio
        ["privacy-speech"] = "",                            // Microphone
        ["privacy-tasks"] = "",                             // CheckList
        ["privacy-videos"] = "",                            // Video
        ["privacy-voiceactivation"] = "",                   // Microphone

        // ---------- Sound ----------
        ["sound"] = "",                                     // Volume
        ["sound-devices"] = "",                             // Headphone
        ["apps-volume"] = "",                               // Volume

        // ---------- System ----------
        ["about"] = "",                                     // Info
        ["display"] = "",                                   // TVMonitor
        ["display-advanced"] = "",                          // TVMonitor
        ["nightlight"] = "",                                // Brightness
        ["notifications"] = "",                             // Ringer
        ["quiethours"] = "",                                // QuietHours
        ["powersleep"] = "",                                // Power
        ["batterysaver"] = "",                              // Battery
        ["energyrecommendations"] = "",                     // Lightbulb
        ["storagesense"] = "",                              // DiskStorage
        ["storagepolicies"] = "",                           // DiskStorage
        ["storagerecommendations"] = "",                    // Lightbulb
        ["savelocations"] = "",                             // Save
        ["disksandvolumes"] = "",                           // DiskStorage
        ["deviceencryption"] = "",                          // Lock
        ["multitasking"] = "",                              // Tablet (multi window)
        ["clipboard"] = "",                                 // Paste
        ["remotedesktop"] = "",                             // RemoteDesktop
        ["project"] = "",                                   // Cast
        ["crossdevice"] = "",                               // Devices
        ["presence"] = "",                                  // Contact
        ["controlcenter"] = "",                             // ActionCenter
        ["search"] = "",                                    // Search
        ["search-permissions"] = "",                        // Lock

        // ---------- Time and language ----------
        ["dateandtime"] = "",                               // Calendar
        ["regionlanguage"] = "",                            // Globe
        ["regionformatting"] = "",                          // Globe
        ["keyboard"] = "",                                  // Keyboard
        ["speech"] = "",                                    // Microphone

        // ---------- Accessibility ----------
        ["easeofaccess-display"] = "",                      // TVMonitor
        ["easeofaccess-visualeffects"] = "",                // Color
        ["easeofaccess-mousepointer"] = "",                 // Mouse
        ["easeofaccess-cursor"] = "",                       // CaretSolid
        ["easeofaccess-magnifier"] = "",                    // Zoom
        ["easeofaccess-colorfilter"] = "",                  // Color
        ["easeofaccess-highcontrast"] = "",                 // Contrast
        ["easeofaccess-narrator"] = "",                     // Headphone (read aloud)
        ["easeofaccess-audio"] = "",                        // Volume
        ["easeofaccess-closedcaptioning"] = "",             // CC
        ["easeofaccess-speechrecognition"] = "",            // Microphone
        ["easeofaccess-keyboard"] = "",                     // Keyboard
        ["easeofaccess-mouse"] = "",                        // Mouse
        ["easeofaccess-eyecontrol"] = "",                   // RedEye
        ["easeofaccess-hearingaids"] = "",                  // Headphone

        // ---------- Family ----------
        ["family-group"] = "",                              // People

        // ---------- Gaming ----------
        ["gaming-gamebar"] = "",                            // XboxLogo
        ["gaming-gamedvr"] = "",                            // Video
        ["gaming-gamemode"] = "",                           // XboxLogo

        // ---------- Update & security ----------
        ["windowsupdate"] = "",                             // Sync
        ["windowsupdate-history"] = "",                     // History
        ["windowsupdate-activehours"] = "",                 // Clock
        ["windowsupdate-options"] = "",                     // Setting
        ["windowsupdate-optionalupdates"] = "",             // Download
        ["windowsupdate-restartoptions"] = "",              // Refresh
        ["delivery-optimization"] = "",                     // World
        ["activation"] = "",                                // Permissions
        ["recovery"] = "",                                  // Refresh (alt)
        ["backup"] = "",                                    // Save (backup)
        ["troubleshoot"] = "",                              // Diagnostic
        ["developers"] = "",                                // Code
        ["findmydevice"] = "",                              // MapPin
        ["windowsdefender"] = "",                           // Shield
        ["windowsinsider"] = "",                            // Insider
    };
}
