using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.UI.Dispatching;
using Windows.ApplicationModel.DataTransfer;

namespace LauncherApp.Services;

public sealed record ClipboardHistoryEntry(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("content")] string Content,
    [property: JsonPropertyName("capturedAt")] DateTimeOffset CapturedAt);

public sealed class ClipboardHistoryService : IDisposable
{
    // Parity with macOS AppConstants.Launcher.Clipboard - caps history at 10 entries to
    // keep the picker scannable and limit how much sensitive captured text sits on disk.
    // Reducing MaxEntries naturally truncates an existing larger history on next load
    // because LoadPersisted applies Take(MaxEntries).
    private const int MaxEntries = 10;
    private const int MaxContentChars = 30_000;
    private const uint WM_CLIPBOARDUPDATE = 0x031D;
    private const int ClipboardSubclassId = 2;

    private readonly IntPtr _hwnd;
    private readonly DispatcherQueue _dispatcher;
    private readonly List<ClipboardHistoryEntry> _entries = [];
    private readonly object _entriesLock = new();
    private readonly string _persistencePath;
    private SubclassProc? _subclassProc;
    private bool _listenerRegistered;
    private bool _suppressNextCapture;
    private bool _disposed;

    public event EventHandler? Changed;

    public IReadOnlyList<ClipboardHistoryEntry> Snapshot()
    {
        lock (_entriesLock)
        {
            return _entries.ToArray();
        }
    }

    public ClipboardHistoryService(IntPtr hwnd, DispatcherQueue dispatcher)
    {
        _hwnd = hwnd;
        _dispatcher = dispatcher;
        _persistencePath = ResolvePersistencePath();

        LoadPersisted();

        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        _subclassProc = ClipboardSubclassProc;
        if (!SetWindowSubclass(hwnd, _subclassProc, (UIntPtr)ClipboardSubclassId, IntPtr.Zero))
        {
            Debug.WriteLine("[ClipboardHistoryService] SetWindowSubclass failed");
            return;
        }

        if (AddClipboardFormatListener(hwnd))
        {
            _listenerRegistered = true;
        }
        else
        {
            Debug.WriteLine("[ClipboardHistoryService] AddClipboardFormatListener failed");
        }
    }

    public bool RemoveEntry(string id)
    {
        if (string.IsNullOrEmpty(id))
        {
            return false;
        }

        bool removed;
        lock (_entriesLock)
        {
            int index = _entries.FindIndex(e => string.Equals(e.Id, id, StringComparison.Ordinal));
            if (index < 0)
            {
                return false;
            }
            _entries.RemoveAt(index);
            removed = true;
        }

        if (removed)
        {
            Persist();
            _dispatcher.TryEnqueue(() => Changed?.Invoke(this, EventArgs.Empty));
        }
        return removed;
    }

    public void SuppressNextCapture()
    {
        _suppressNextCapture = true;
    }

    private IntPtr ClipboardSubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, IntPtr dwRefData)
    {
        if (uMsg == WM_CLIPBOARDUPDATE)
        {
            if (_suppressNextCapture)
            {
                _suppressNextCapture = false;
            }
            else
            {
                _ = CaptureCurrentClipboardAsync();
            }
        }

        return DefSubclassProc(hWnd, uMsg, wParam, lParam);
    }

    private async System.Threading.Tasks.Task CaptureCurrentClipboardAsync()
    {
        try
        {
            DataPackageView view = Clipboard.GetContent();
            if (view is null || !view.Contains(StandardDataFormats.Text))
            {
                return;
            }

            // Skip clipboard payloads that carry a file/folder reference. Explorer's "Copy"
            // puts StorageItems on the clipboard AND synthesizes a text path; without this
            // guard, every file copy pollutes history with the raw path string. Mirrors
            // macOS pasteboardCarriesFileReference (ClipboardHistoryStore.swift:136).
            if (view.Contains(StandardDataFormats.StorageItems))
            {
                return;
            }

            string text = await view.GetTextAsync();
            if (string.IsNullOrEmpty(text))
            {
                return;
            }

            if (text.Length > MaxContentChars)
            {
                return;
            }

            bool changed = false;
            lock (_entriesLock)
            {
                int existing = _entries.FindIndex(e => string.Equals(e.Content, text, StringComparison.Ordinal));
                if (existing >= 0)
                {
                    ClipboardHistoryEntry moved = _entries[existing] with { CapturedAt = DateTimeOffset.Now };
                    _entries.RemoveAt(existing);
                    _entries.Insert(0, moved);
                    changed = existing != 0;
                }
                else
                {
                    _entries.Insert(0, new ClipboardHistoryEntry(Guid.NewGuid().ToString("N"), text, DateTimeOffset.Now));
                    if (_entries.Count > MaxEntries)
                    {
                        _entries.RemoveRange(MaxEntries, _entries.Count - MaxEntries);
                    }
                    changed = true;
                }
            }

            if (!changed)
            {
                return;
            }

            Persist();
            _dispatcher.TryEnqueue(() => Changed?.Invoke(this, EventArgs.Empty));
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ClipboardHistoryService] capture failed: {ex.Message}");
        }
    }

    private void LoadPersisted()
    {
        try
        {
            if (!File.Exists(_persistencePath))
            {
                return;
            }

            string json = File.ReadAllText(_persistencePath);
            if (string.IsNullOrWhiteSpace(json))
            {
                return;
            }

            List<ClipboardHistoryEntry>? loaded = JsonSerializer.Deserialize<List<ClipboardHistoryEntry>>(json);
            if (loaded is null)
            {
                return;
            }

            lock (_entriesLock)
            {
                _entries.Clear();
                foreach (ClipboardHistoryEntry entry in loaded.Take(MaxEntries))
                {
                    _entries.Add(entry);
                }
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ClipboardHistoryService] load failed: {ex.Message}");
        }
    }

    private void Persist()
    {
        try
        {
            string? dir = Path.GetDirectoryName(_persistencePath);
            if (!string.IsNullOrWhiteSpace(dir))
            {
                Directory.CreateDirectory(dir);
            }

            ClipboardHistoryEntry[] snapshot;
            lock (_entriesLock)
            {
                snapshot = _entries.ToArray();
            }

            string json = JsonSerializer.Serialize(snapshot);
            File.WriteAllText(_persistencePath, json);
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ClipboardHistoryService] persist failed: {ex.Message}");
        }
    }

    private static string ResolvePersistencePath()
    {
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string baseDir = string.IsNullOrWhiteSpace(localAppData) ? Path.GetTempPath() : Path.Combine(localAppData, "look");
        return Path.Combine(baseDir, "clipboard-history.json");
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }
        _disposed = true;

        try
        {
            if (_listenerRegistered)
            {
                RemoveClipboardFormatListener(_hwnd);
                _listenerRegistered = false;
            }
            if (_subclassProc is not null)
            {
                RemoveWindowSubclass(_hwnd, _subclassProc, (UIntPtr)ClipboardSubclassId);
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ClipboardHistoryService] dispose failed: {ex.Message}");
        }
    }

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool AddClipboardFormatListener(IntPtr hWnd);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool RemoveClipboardFormatListener(IntPtr hWnd);

    [DllImport("comctl32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, UIntPtr uIdSubclass, IntPtr dwRefData);

    [DllImport("comctl32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool RemoveWindowSubclass(IntPtr hWnd, SubclassProc pfnSubclass, UIntPtr uIdSubclass);

    [DllImport("comctl32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern IntPtr DefSubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam);

    private delegate IntPtr SubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, IntPtr dwRefData);
}
