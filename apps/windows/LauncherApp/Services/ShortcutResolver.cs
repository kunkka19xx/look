using System;
using System.Runtime.InteropServices;
using System.Text;

namespace LauncherApp.Services;

// Resolves .lnk shortcut targets via early-bound IShellLinkW / IPersistFile COM.
// The previous WScript.Shell + `dynamic` implementation crashed the process on
// some Windows 10 installs: the DLR's IDispatch type-info scan hit an uncatchable
// failure inside Microsoft.CSharp.RuntimeBinder.ComInterop.GetTypeAttrForTypeInfo
// during ITypeInfo.ReleaseTypeAttr. Early-bound ComImport interfaces sidestep the
// runtime binder entirely.
//
// ShellLink COM activation requires an STA apartment. WinUI UI-thread callers are
// fine; if a future caller runs on the thread pool, activation will throw and the
// catch will return false (no resolution, no crash).
internal static class ShortcutResolver
{
    public static bool TryResolveShortcutTarget(string shortcutPath, out string targetPath)
    {
        targetPath = string.Empty;

        IShellLinkW? shellLink = null;
        IPersistFile? persistFile = null;
        try
        {
            shellLink = (IShellLinkW)new ShellLink();
            persistFile = (IPersistFile)shellLink;
            persistFile.Load(shortcutPath, 0);

            var buffer = new StringBuilder(32768);
            shellLink.GetPath(buffer, buffer.Capacity, IntPtr.Zero, 0);
            targetPath = buffer.ToString().Trim();
            return targetPath.Length > 0;
        }
        catch
        {
            return false;
        }
        finally
        {
            if (persistFile != null)
                Marshal.ReleaseComObject(persistFile);
            if (shellLink != null)
                Marshal.ReleaseComObject(shellLink);
        }
    }

    [ComImport]
    [Guid("00021401-0000-0000-C000-000000000046")]
    private class ShellLink
    {
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("000214F9-0000-0000-C000-000000000046")]
    private interface IShellLinkW
    {
        void GetPath([Out] StringBuilder pszFile, int cchMaxPath, IntPtr pfd, uint fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription([Out] StringBuilder pszName, int cchMaxName);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory([Out] StringBuilder pszDir, int cchMaxPath);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments([Out] StringBuilder pszArgs, int cchMaxPath);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out short pwHotkey);
        void SetHotkey(short wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation([Out] StringBuilder pszIconPath, int cchIconPath, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("0000010B-0000-0000-C000-000000000046")]
    private interface IPersistFile
    {
        void GetClassID(out Guid pClassID);
        void IsDirty();
        void Load([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, uint dwMode);
        void Save([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, bool fRemember);
        void SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
        void GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
    }
}
