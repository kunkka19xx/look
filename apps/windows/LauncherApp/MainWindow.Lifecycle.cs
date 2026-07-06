using System;
using System.Diagnostics;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using WinRT.Interop;

namespace LauncherApp;

// Window lifecycle: global hotkeys (Alt+Space / Alt+Shift+Q), hide-on-blur auto-dismiss,
// WS_EX_TOOLWINDOW (hide from taskbar + Alt-Tab), SuppressAutoHide scope for modal pickers,
// and the Close teardown path.
public sealed partial class MainWindow
{
    private SubclassProc? _hotkeySubclassProc;
    private IntPtr _hotkeyHwnd = IntPtr.Zero;
    private const int HotkeyIdToggle = 0xB001;
    private const int HotkeyIdQuit = 0xB002;
    private int _suppressAutoHideDepth;

    private void OnWindowClosed(object sender, WindowEventArgs args)
    {
        this.Activated -= OnWindowActivated;
        UnregisterGlobalHotkeys();
        _bannerCts?.Cancel();
        _bannerCts = null;
        if (_clipboardHistory is not null)
        {
            _clipboardHistory.Changed -= OnClipboardHistoryChanged;
            _clipboardHistory.Dispose();
            _clipboardHistory = null;
        }
    }

    private void OnWindowActivated(object sender, WindowActivatedEventArgs args)
    {
        if (args.WindowActivationState != WindowActivationState.Deactivated)
        {
            return;
        }
        if (_suppressAutoHideDepth > 0)
        {
            return;
        }
        if (this.AppWindow is not AppWindow appWindow || !appWindow.IsVisible)
        {
            return;
        }
        appWindow.Hide();
    }

    public IDisposable SuppressAutoHide()
    {
        _suppressAutoHideDepth++;
        return new SuppressAutoHideToken(this);
    }

    private sealed class SuppressAutoHideToken : IDisposable
    {
        private MainWindow? _owner;

        public SuppressAutoHideToken(MainWindow owner)
        {
            _owner = owner;
        }

        public void Dispose()
        {
            MainWindow? owner = _owner;
            if (owner is null)
            {
                return;
            }
            _owner = null;
            owner._suppressAutoHideDepth = Math.Max(0, owner._suppressAutoHideDepth - 1);
        }
    }

    private void HideFromTaskbarAndAltTab()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        long exStyle = GetWindowLongPtrCompat(hwnd, GWL_EXSTYLE).ToInt64();
        long updated = (exStyle | WS_EX_TOOLWINDOW) & ~WS_EX_APPWINDOW;
        if (updated == exStyle)
        {
            return;
        }
        SetWindowLongPtrCompat(hwnd, GWL_EXSTYLE, new IntPtr(updated));
    }

    private static IntPtr GetWindowLongPtrCompat(IntPtr hWnd, int nIndex)
    {
        if (IntPtr.Size == 8)
        {
            return GetWindowLongPtr64(hWnd, nIndex);
        }
        return new IntPtr(GetWindowLong32(hWnd, nIndex));
    }

    private static IntPtr SetWindowLongPtrCompat(IntPtr hWnd, int nIndex, IntPtr dwNewLong)
    {
        if (IntPtr.Size == 8)
        {
            return SetWindowLongPtr64(hWnd, nIndex, dwNewLong);
        }
        return new IntPtr(SetWindowLong32(hWnd, nIndex, dwNewLong.ToInt32()));
    }

    private void InitializeGlobalHotkeys()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }
        _hotkeyHwnd = hwnd;

        _hotkeySubclassProc = HotkeySubclassProc;
        if (!SetWindowSubclass(hwnd, _hotkeySubclassProc, (UIntPtr)1, IntPtr.Zero))
        {
            Debug.WriteLine("[MainWindow] SetWindowSubclass failed");
            return;
        }

        if (!RegisterHotKey(hwnd, HotkeyIdToggle, MOD_ALT | MOD_NOREPEAT, VK_SPACE))
        {
            Debug.WriteLine("[MainWindow] RegisterHotKey Alt+Space failed (already in use?)");
        }

        if (!RegisterHotKey(hwnd, HotkeyIdQuit, MOD_ALT | MOD_SHIFT | MOD_NOREPEAT, VK_Q))
        {
            Debug.WriteLine("[MainWindow] RegisterHotKey Alt+Shift+Q failed");
        }
    }

    private void UnregisterGlobalHotkeys()
    {
        if (_hotkeyHwnd == IntPtr.Zero)
        {
            return;
        }

        UnregisterHotKey(_hotkeyHwnd, HotkeyIdToggle);
        UnregisterHotKey(_hotkeyHwnd, HotkeyIdQuit);
        if (_hotkeySubclassProc is not null)
        {
            RemoveWindowSubclass(_hotkeyHwnd, _hotkeySubclassProc, (UIntPtr)1);
        }
        _hotkeyHwnd = IntPtr.Zero;
    }

    private IntPtr HotkeySubclassProc(IntPtr hWnd, uint uMsg, IntPtr wParam, IntPtr lParam, UIntPtr uIdSubclass, IntPtr dwRefData)
    {
        if (uMsg == WM_HOTKEY)
        {
            int id = wParam.ToInt32();
            if (id == HotkeyIdToggle)
            {
                ToggleLauncherVisibility();
                return IntPtr.Zero;
            }
            if (id == HotkeyIdQuit)
            {
                ForceQuit();
                return IntPtr.Zero;
            }
        }

        return DefSubclassProc(hWnd, uMsg, wParam, lParam);
    }

    private void ToggleLauncherVisibility()
    {
        AppWindow appWindow = this.AppWindow;
        if (appWindow is null)
        {
            return;
        }

        if (appWindow.IsVisible)
        {
            appWindow.Hide();
            return;
        }

        ShowLauncher();
    }

    /// <summary>
    /// Show the launcher window from any state and give the search input focus.
    /// Idempotent - safe to call when already visible. Used by the global hotkey
    /// (via ToggleLauncherVisibility) and by the single-instance activation listener
    /// in App.SingleInstance.cs when a sibling launch hands off to the primary.
    /// </summary>
    public void ShowLauncher()
    {
        AppWindow appWindow = this.AppWindow;
        if (appWindow is null)
        {
            return;
        }

        if (!appWindow.IsVisible)
        {
            appWindow.Show();
        }

        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd != IntPtr.Zero)
        {
            SetForegroundWindow(hwnd);
        }
        QueryInput.Focus(FocusState.Programmatic);
        QueryInput.SelectAll();
    }

    private static void ForceQuit()
    {
        try
        {
            Microsoft.UI.Xaml.Application.Current.Exit();
        }
        catch
        {
            Environment.Exit(0);
        }
    }
}
