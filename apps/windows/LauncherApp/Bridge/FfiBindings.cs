using System;
using System.Runtime.InteropServices;

namespace LauncherApp.Bridge;

internal static class FfiBindings
{
    private const string LibraryName = "look_ffi";

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr look_search_json_compact(IntPtr query, uint limit);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void look_free_cstring(IntPtr ptr);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static extern bool look_request_index_refresh();

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static extern bool look_reload_config();
}
