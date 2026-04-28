using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.Foundation;
using Windows.Foundation.Collections;
using LauncherApp.Services;

// To learn more about WinUI, the WinUI project structure,
// and more about our project templates, see: http://aka.ms/winui-project-info.

namespace LauncherApp
{
    /// <summary>
    /// Provides application-specific behavior to supplement the default Application class.
    /// </summary>
    public partial class App : Application
    {
        private Window? _window;
        public static MainWindow? MainAppWindow { get; private set; }

        /// <summary>
        /// Initializes the singleton application object.  This is the first line of authored code
        /// executed, and as such is the logical equivalent of main() or WinMain().
        /// </summary>
        public App()
        {
            // Mirrors macOS AppDelegate.checkAndActivateDuplicateInstance: if another
            // copy of this exe is already running, signal it forward and exit instead
            // of accumulating a second window / hotkey-registration / clipboard-listener.
            // See App.SingleInstance.cs for the path-hashed mutex + activate-event setup.
            if (!TryClaimSingleton())
            {
                Environment.Exit(0);
                return;
            }

            InitializeComponent();
            UnhandledException += OnUnhandledException;
            AppDomain.CurrentDomain.UnhandledException += OnDomainUnhandledException;
            System.Threading.Tasks.TaskScheduler.UnobservedTaskException += OnUnobservedTaskException;
        }

        private static void OnDomainUnhandledException(object sender, System.UnhandledExceptionEventArgs e)
        {
            WriteCrashLog("AppDomain", e.ExceptionObject as Exception);
        }

        private static void OnUnobservedTaskException(object? sender, System.Threading.Tasks.UnobservedTaskExceptionEventArgs e)
        {
            WriteCrashLog("TaskScheduler", e.Exception);
            e.SetObserved();
        }

        private static void WriteCrashLog(string origin, Exception? ex)
        {
            try
            {
                string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
                string logDir = string.IsNullOrWhiteSpace(localAppData) ? System.IO.Path.GetTempPath() : System.IO.Path.Combine(localAppData, "look");
                Directory.CreateDirectory(logDir);
                string logPath = System.IO.Path.Combine(logDir, "look-crash.log");
                string message = $"[{DateTime.Now:O}] [{origin}] {ex?.GetType().Name}: {ex?.Message}{Environment.NewLine}{ex}{Environment.NewLine}";
                File.AppendAllText(logPath, message);
            }
            catch
            {
            }
        }

        /// <summary>
        /// Invoked when the application is launched.
        /// </summary>
        /// <param name="args">Details about the launch request and process.</param>
        protected override void OnLaunched(Microsoft.UI.Xaml.LaunchActivatedEventArgs args)
        {
            StartupRegistration.Sync(LookConfig.GetBool("launch_at_login", fallback: false));

            // Apply the persisted appearance (tint / text / border / font / blur-opacity)
            // to Application.Resources before the main window loads. Without this, the app
            // always boots from App.xaml defaults and user-saved theme values only survive
            // within a single session.
            ThemeBootstrap.ApplyFromConfig();

            MainAppWindow = new MainWindow();
            _window = MainAppWindow;
            _window.Activate();

            // Spawn the activation listener after the window is constructed so it has a
            // valid DispatcherQueue to marshal onto. Sibling launches Set the named event,
            // and this thread reacts by calling MainWindow.ShowLauncher on the UI thread.
            StartActivationListener(MainAppWindow);
        }

        private static void OnUnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
        {
            WriteCrashLog("UI", e.Exception);
            e.Handled = true;
        }
    }
}
