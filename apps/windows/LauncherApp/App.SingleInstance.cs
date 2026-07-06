using System;
using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using Microsoft.UI.Dispatching;

namespace LauncherApp;

// Mirrors macOS Support/SingleInstanceLock.swift + AppDelegate.checkAndActivateDuplicateInstance:
// at most one primary per exe path. Launching a second copy of the SAME exe path signals the
// primary to come forward and the second copy exits. Different exe paths (e.g. the build dir
// vs the %LOCALAPPDATA%\Programs\Look Dev side-by-side install) get separate keys and can run
// concurrently - same per-bundle-path semantics as macOS.
public partial class App
{
    private static Mutex? _singletonMutex;
    private static EventWaitHandle? _activateEvent;
    private static Thread? _activateListenerThread;

    private static (string mutexName, string eventName) GetSingleInstanceNames()
    {
        string exePath;
        try
        {
            exePath = Process.GetCurrentProcess().MainModule?.FileName ?? "look";
        }
        catch
        {
            exePath = "look";
        }

        byte[] digest = SHA256.HashData(Encoding.UTF8.GetBytes(exePath));
        string hash = Convert.ToHexString(digest, 0, 8); // 16 hex chars

        // Local\ namespace = per-user; multiple users on one machine each get their own
        // primary, which matches the per-user nature of the launcher.
        return ($"Local\\noah-code.Look.SingleInstance.{hash}",
                $"Local\\noah-code.Look.Activate.{hash}");
    }

    /// <summary>
    /// Returns true when this process owns the singleton (primary). Returns false when
    /// another instance already holds it - in that case the existing primary has been
    /// signaled and the caller should exit.
    /// </summary>
    private static bool TryClaimSingleton()
    {
        var (mutexName, eventName) = GetSingleInstanceNames();

        try
        {
            _singletonMutex = new Mutex(initiallyOwned: false, mutexName);
        }
        catch
        {
            // Fail open: if we can't even create the mutex (security ACL or odd
            // sandbox), launching a duplicate is better than refusing to start.
            return true;
        }

        bool acquired;
        try
        {
            acquired = _singletonMutex.WaitOne(millisecondsTimeout: 0, exitContext: false);
        }
        catch (AbandonedMutexException)
        {
            // The previous primary crashed without releasing the mutex; ownership
            // transfers to us. WaitOne already returned the mutex held.
            acquired = true;
        }

        if (!acquired)
        {
            try
            {
                EventWaitHandle.OpenExisting(eventName)?.Set();
            }
            catch
            {
                // If the activate event is missing the primary is in an odd state;
                // we still exit. User can press Alt+Space to wake it.
            }
            return false;
        }

        try
        {
            _activateEvent = new EventWaitHandle(false, EventResetMode.AutoReset, eventName);
        }
        catch
        {
            _activateEvent = null;
        }
        return true;
    }

    /// <summary>
    /// Spawn a background thread that wakes the launcher window when a sibling
    /// instance signals the activate event. Called once after the MainWindow is
    /// constructed so the dispatcher queue is available.
    /// </summary>
    private static void StartActivationListener(MainWindow window)
    {
        if (_activateEvent is null)
        {
            return;
        }

        DispatcherQueue dispatcherQueue = window.DispatcherQueue;

        _activateListenerThread = new Thread(() =>
        {
            while (true)
            {
                try
                {
                    if (_activateEvent is null || !_activateEvent.WaitOne())
                    {
                        return;
                    }
                    dispatcherQueue.TryEnqueue(window.ShowLauncher);
                }
                catch
                {
                    return;
                }
            }
        })
        {
            IsBackground = true,
            Name = "look-singleton-activation-listener",
        };
        _activateListenerThread.Start();
    }
}
