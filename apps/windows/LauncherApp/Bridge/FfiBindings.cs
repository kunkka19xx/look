using System;
using System.Runtime.InteropServices;

namespace LauncherApp.Bridge;

public static class FfiBindings
{
    private const string LibraryName = "look_ffi";

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr look_search_json_compact(IntPtr query, uint limit);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void look_free_cstring(IntPtr ptr);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool look_request_index_refresh();

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool look_reload_config();

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool look_record_usage(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string candidateId,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string action);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    [return: MarshalAs(UnmanagedType.I1)]
    public static extern bool look_seed_uwp_apps_json(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string json);

    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr look_translate_json(IntPtr text, IntPtr targetLang);
}
