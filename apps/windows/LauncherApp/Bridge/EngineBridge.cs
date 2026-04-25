using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Diagnostics;

namespace LauncherApp.Bridge;

public sealed class EngineBridge
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    public IReadOnlyList<LauncherResult> Search(string query, int limit = 40)
    {
        IntPtr queryPtr = IntPtr.Zero;
        IntPtr resultPtr = IntPtr.Zero;

        try
        {
            Debug.WriteLine($"[EngineBridge] Searching: '{query}' limit={limit}");
            queryPtr = Marshal.StringToCoTaskMemUTF8(query);
            resultPtr = FfiBindings.look_search_json_compact(queryPtr, (uint)limit);
            if (resultPtr == IntPtr.Zero)
            {
                Debug.WriteLine("[EngineBridge] FFI returned null");
                return [];
            }

            string raw = Marshal.PtrToStringUTF8(resultPtr) ?? string.Empty;
            Debug.WriteLine($"[EngineBridge] Raw result: {raw.Substring(0, Math.Min(200, raw.Length))}");
            if (string.IsNullOrWhiteSpace(raw))
            {
                return [];
            }

            CompactSearchPayload? payload = JsonSerializer.Deserialize<CompactSearchPayload>(raw, JsonOptions);
            if (payload?.Error != null || payload?.Results == null)
            {
                return [];
            }

            List<LauncherResult> mapped = new(payload.Results.Count);
            foreach (SearchItem item in payload.Results)
            {
                mapped.Add(new LauncherResult
                {
                    Id = item.Id,
                    Kind = item.Kind,
                    Title = item.Title,
                    Subtitle = item.Subtitle,
                    Path = item.Path,
                    Score = item.Score,
                });
            }

            return mapped;
        }
        catch (DllNotFoundException)
        {
            return [];
        }
        catch (EntryPointNotFoundException)
        {
            return [];
        }
        catch (JsonException)
        {
            return [];
        }
        finally
        {
            if (resultPtr != IntPtr.Zero)
            {
                FfiBindings.look_free_cstring(resultPtr);
            }

            if (queryPtr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(queryPtr);
            }
        }
    }

    public TranslatePayload? Translate(string text, string targetLang)
    {
        IntPtr textPtr = IntPtr.Zero;
        IntPtr langPtr = IntPtr.Zero;
        IntPtr resultPtr = IntPtr.Zero;

        try
        {
            textPtr = Marshal.StringToCoTaskMemUTF8(text);
            langPtr = Marshal.StringToCoTaskMemUTF8(targetLang);
            resultPtr = FfiBindings.look_translate_json(textPtr, langPtr);
            if (resultPtr == IntPtr.Zero)
            {
                return null;
            }

            string raw = Marshal.PtrToStringUTF8(resultPtr) ?? string.Empty;
            if (string.IsNullOrWhiteSpace(raw))
            {
                return null;
            }

            return JsonSerializer.Deserialize<TranslatePayload>(raw, JsonOptions);
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
        catch (JsonException)
        {
            return null;
        }
        finally
        {
            if (resultPtr != IntPtr.Zero)
            {
                FfiBindings.look_free_cstring(resultPtr);
            }

            if (langPtr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(langPtr);
            }

            if (textPtr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(textPtr);
            }
        }
    }
}
