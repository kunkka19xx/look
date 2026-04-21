using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Core;
using LauncherApp.Features.Search;
using LauncherApp.Services;
using LauncherApp.Views.Settings;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using WinRT.Interop;
using WinUIEx;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace LauncherApp
{
    public sealed partial class MainWindow : Window
    {
        private readonly LauncherSearchLogic _searchLogic;
        private readonly ActionDispatcher _actionDispatcher;
        private readonly ObservableCollection<LauncherRowItem> _results;
        private readonly List<LauncherResult> _commandSeed;
        private readonly List<LauncherResult> _clipboardSeed;
        private SettingsTabsView? _settingsTabsView;
        private LauncherMode _mode = LauncherMode.Search;
        private int _searchVersion;
        private string _backdropMode = "Acrylic";
        private string _appliedBackdropMode = string.Empty;
        private Windows.UI.Color _acrylicTint = Windows.UI.Color.FromArgb(45, 21, 28, 38);
        private readonly TransparentTintBackdrop _transparentBackdrop;
        private static readonly ConcurrentDictionary<string, bool> ExeVersionInfoCache = new(StringComparer.OrdinalIgnoreCase);
        private static readonly string[] NoisyExecutableNameTokens =
        [
            "appinstallerprotocolshim",
            "appinstallerpythonredirector",
            "deploymentagent",
            "dynamicdependency.datastore",
            "pushnotificationslongrunningtask",
            "pushnotificationbackgroundtask",
            "backgroundtask",
            "ftserver",
            "elevate-shim",
            "protocolshim",
            "redirector",
            "crashpad",
        ];

        public string CurrentBackdropMode => _backdropMode;

        public MainWindow()
        {
            InitializeComponent();
            _transparentBackdrop = new TransparentTintBackdrop(_acrylicTint);
            ConfigureLauncherWindow();

            if (Content is UIElement root)
            {
                root.AddHandler(UIElement.KeyDownEvent, new KeyEventHandler(GlobalKeyDown), true);
            }

            bool useRealSearch = true;
            ISearchProvider searchProvider = useRealSearch
                ? new FfiSearchProvider(new EngineBridge())
                : new MockSearchProvider();

            _searchLogic = new LauncherSearchLogic(searchProvider);
            _actionDispatcher = new ActionDispatcher(new ShellExecuteService(), new ExplorerRevealService());
            _results = new ObservableCollection<LauncherRowItem>();
            _commandSeed = BuildCommandSeed();
            _clipboardSeed = BuildClipboardSeed();

            ResultsList.ItemsSource = _results;
            SetMode(LauncherMode.Search);
            RefreshResults(QueryInput.Text?.Trim() ?? string.Empty);
        }

        private SettingsTabsView EnsureSettingsView()
        {
            if (_settingsTabsView != null)
            {
                return _settingsTabsView;
            }

            _settingsTabsView = new SettingsTabsView();
            _settingsTabsView.CloseRequested += SettingsTabsPanel_OnCloseRequested;
            SettingsHost.Children.Clear();
            SettingsHost.Children.Add(_settingsTabsView);
            return _settingsTabsView;
        }

        private static List<LauncherResult> BuildCommandSeed()
        {
            return
            [
                new LauncherResult { Id = "command:calc", Kind = "app", Title = "calc", Subtitle = "Evaluate expression", Path = "command://calc", Score = 1000 },
                new LauncherResult { Id = "command:shell", Kind = "app", Title = "shell", Subtitle = "Run shell command", Path = "command://shell", Score = 990 },
                new LauncherResult { Id = "command:kill", Kind = "app", Title = "kill", Subtitle = "Terminate process", Path = "command://kill", Score = 980 },
                new LauncherResult { Id = "command:sys", Kind = "app", Title = "sys", Subtitle = "System info panel", Path = "command://sys", Score = 970 },
            ];
        }

        private static List<LauncherResult> BuildClipboardSeed()
        {
            return
            [
                new LauncherResult { Id = "clip:1", Kind = "clipboard", Title = "cargo test -p look-engine", Subtitle = "Copied 3m ago", Path = "cargo test -p look-engine", Score = 1000 },
                new LauncherResult { Id = "clip:2", Kind = "clipboard", Title = "https://docs.rs/serde", Subtitle = "Copied 11m ago", Path = "https://docs.rs/serde", Score = 990 },
                new LauncherResult { Id = "clip:3", Kind = "clipboard", Title = "C:\\Users\\haong\\Documents\\git\\look", Subtitle = "Copied 19m ago", Path = "C:\\Users\\haong\\Documents\\git\\look", Score = 980 },
            ];
        }

        private void ConfigureLauncherWindow()
        {
            this.SetWindowSize(960, 620);
            this.CenterOnScreen();
            this.SetWindowPresenter(AppWindowPresenterKind.Overlapped);
            this.SetIsResizable(false);
            this.SetIsMaximizable(false);
            this.SetIsMinimizable(false);

            if (this.AppWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.SetBorderAndTitleBar(true, false);
            }

            ExtendsContentIntoTitleBar = false;

            if (Content is FrameworkElement root)
            {
                root.RequestedTheme = ElementTheme.Dark;
            }

            SetBackdropMode("Acrylic");

            ApplyRuntimeIcon();
        }

        public void SetBackdropMode(string mode)
        {
            _backdropMode = mode;

            if (mode.Equals(_appliedBackdropMode, StringComparison.OrdinalIgnoreCase))
            {
                return;
            }

            if (mode.Equals("Acrylic", StringComparison.OrdinalIgnoreCase))
            {
                SystemBackdrop = _transparentBackdrop;
                ApplyFrameStyle(removeRoundedCorners: true, removeBorder: true);
                _appliedBackdropMode = mode;
                return;
            }

            if (mode.Equals("Solid", StringComparison.OrdinalIgnoreCase))
            {
                SystemBackdrop = null;
                DisableAcrylicFallback();
                ApplyFrameStyle(removeRoundedCorners: false, removeBorder: false);
                _appliedBackdropMode = mode;
                return;
            }

            DisableAcrylicFallback();
            SystemBackdrop = new MicaBackdrop { Kind = MicaKind.BaseAlt };
            ApplyFrameStyle(removeRoundedCorners: false, removeBorder: false);
            _appliedBackdropMode = mode;
        }

        private void ApplyRuntimeIcon()
        {
            string iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "look.ico");
            if (File.Exists(iconPath))
            {
                this.SetIcon(iconPath);
            }
        }

        public void UpdateAcrylicOpacity(double opacityPercent)
        {
            byte alpha = (byte)System.Math.Clamp((int)System.Math.Round(opacityPercent / 100d * 255d), 10, 120);
            _acrylicTint = Windows.UI.Color.FromArgb(alpha, 21, 28, 38);
            _transparentBackdrop.TintColor = _acrylicTint;
            if (!_backdropMode.Equals("Acrylic", StringComparison.OrdinalIgnoreCase))
            {
                return;
            }
        }

        private void EnableAcrylicFallback(Windows.UI.Color tint)
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            var accent = new AccentPolicy
            {
                AccentState = AccentState.AccentEnableAcrylicBlurBehind,
                AccentFlags = 0,
                GradientColor = BuildAccentColor(tint.A, tint.R, tint.G, tint.B),
                AnimationId = 0
            };

            SetAccentPolicy(hwnd, accent);
        }

        private void DisableAcrylicFallback()
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            var accent = new AccentPolicy
            {
                AccentState = AccentState.AccentDisabled,
                AccentFlags = 0,
                GradientColor = 0,
                AnimationId = 0
            };

            SetAccentPolicy(hwnd, accent);
        }

        private static int BuildAccentColor(byte alpha, byte red, byte green, byte blue)
        {
            return (alpha << 24) | (blue << 16) | (green << 8) | red;
        }

        private static void SetAccentPolicy(IntPtr hwnd, AccentPolicy accent)
        {
            int size = Marshal.SizeOf<AccentPolicy>();
            IntPtr accentPtr = Marshal.AllocHGlobal(size);

            try
            {
                Marshal.StructureToPtr(accent, accentPtr, false);
                var data = new WindowCompositionAttributeData
                {
                    Attribute = WindowCompositionAttribute.WcaAccentPolicy,
                    Data = accentPtr,
                    SizeOfData = size
                };

                SetWindowCompositionAttribute(hwnd, ref data);
            }
            finally
            {
                Marshal.FreeHGlobal(accentPtr);
            }
        }

        [DllImport("user32.dll")]
        private static extern int SetWindowCompositionAttribute(IntPtr hwnd, ref WindowCompositionAttributeData data);

        [DllImport("dwmapi.dll")]
        private static extern int DwmSetWindowAttribute(IntPtr hwnd, int dwAttribute, ref int pvAttribute, int cbAttribute);

        private void ApplyFrameStyle(bool removeRoundedCorners, bool removeBorder)
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            int cornerPreference = removeRoundedCorners
                ? (int)DwmWindowCornerPreference.DoNotRound
                : (int)DwmWindowCornerPreference.Default;

            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.WindowCornerPreference,
                ref cornerPreference,
                Marshal.SizeOf<int>());

            int borderColor = removeBorder ? unchecked((int)0xFFFFFFFE) : unchecked((int)0xFFFFFFFF);
            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.BorderColor,
                ref borderColor,
                Marshal.SizeOf<int>());
        }

        private enum AccentState
        {
            AccentDisabled = 0,
            AccentEnableAcrylicBlurBehind = 4,
        }

        private enum WindowCompositionAttribute
        {
            WcaAccentPolicy = 19,
        }

        private enum DwmWindowAttribute
        {
            WindowCornerPreference = 33,
            BorderColor = 34,
        }

        private enum DwmWindowCornerPreference
        { 
            Default = 0,
            DoNotRound = 1,
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct AccentPolicy
        {
            public AccentState AccentState;
            public int AccentFlags;
            public int GradientColor;
            public int AnimationId;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct WindowCompositionAttributeData
        {
            public WindowCompositionAttribute Attribute;
            public IntPtr Data;
            public int SizeOfData;
        }

        private static (LauncherMode mode, string normalizedQuery) ResolveMode(string rawQuery)
        {
            string query = rawQuery.Trim();
            if (query.StartsWith("/"))
            {
                return (LauncherMode.Command, query.TrimStart('/').Trim());
            }

            if (query.StartsWith("c\"", StringComparison.OrdinalIgnoreCase))
            {
                return (LauncherMode.Clipboard, query.Substring(2).Trim());
            }

            if (query.StartsWith("?"))
            {
                return (LauncherMode.Help, query.TrimStart('?').Trim());
            }

            if (query.StartsWith(","))
            {
                return (LauncherMode.Settings, query.TrimStart(',').Trim());
            }

            return (LauncherMode.Search, query);
        }

        private void SetMode(LauncherMode mode)
        {
            _mode = mode;

            SearchBarHost.Visibility = Visibility.Visible;
            ResultsHost.Visibility = Visibility.Visible;
            HintBarHost.Visibility = Visibility.Visible;
            SettingsHost.Visibility = Visibility.Collapsed;

            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;
            CommandPanelsPanel.Visibility = Visibility.Collapsed;
            HelpScreenPanel.Visibility = Visibility.Collapsed;
            ResultsList.Visibility = Visibility.Visible;

            switch (mode)
            {
                case LauncherMode.Search:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Search apps";
                    HintText.Text = "Enter open  •  Ctrl+R reveal  •  Ctrl+C copy  •  Ctrl+Enter web";
                    ResultsHost.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Command:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Use /calc, /shell, /kill, /sys";
                    HintText.Text = "Enter run  •  Up/Down switch  •  Esc clear";
                    ResultsList.Visibility = Visibility.Collapsed;
                    ResultPreviewPanel.Visibility = Visibility.Collapsed;
                    CommandPanelsPanel.Visibility = Visibility.Visible;
                    CommandPanelsPanel.ApplyFilter(string.Empty);
                    CommandPanelsPanel.SelectPanel("command:calc");
                    break;
                case LauncherMode.Clipboard:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Use c\" to search clipboard";
                    HintText.Text = "Enter copy  •  Up/Down select  •  Esc clear";
                    ResultsList.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Settings:
                    ApplyConfiguredSurface();
                    EnsureSettingsView();
                    SearchBarHost.Visibility = Visibility.Collapsed;
                    ResultsHost.Visibility = Visibility.Collapsed;
                    HintBarHost.Visibility = Visibility.Collapsed;
                    SettingsHost.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Help:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Use ? or Ctrl+H to view help";
                    HintText.Text = "Ctrl+H close  •  Esc hide  •  Ctrl+/ command mode";
                    ResultsList.Visibility = Visibility.Collapsed;
                    HelpScreenPanel.Visibility = Visibility.Visible;
                    break;
            }
        }

        private void ApplyConfiguredSurface()
        {
            SetBackdropMode(_backdropMode);
            LauncherSurface.Background = (Brush)Application.Current.Resources["LauncherPanelBrush"];
        }

        private void RefreshResults(string rawQuery)
        {
            var (resolvedMode, query) = ResolveMode(rawQuery);
            if (resolvedMode != _mode)
            {
                SetMode(resolvedMode);
            }

            IReadOnlyList<LauncherResult> source = _mode switch
            {
                LauncherMode.Search => FilterSearchNoise(DeduplicatePairedAppEntries(_searchLogic.Search(query, 120)), 40),
                LauncherMode.Command => FilterRows(_commandSeed, query),
                LauncherMode.Clipboard => FilterRows(_clipboardSeed, query),
                LauncherMode.Settings => [],
                LauncherMode.Help => FilterRows(BuildHelpRows(), query),
                _ => [],
            };

            _results.Clear();
            foreach (LauncherResult item in source)
            {
                _results.Add(new LauncherRowItem(item));
            }

            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.ApplyFilter(query);
                if (_results.Count > 0)
                {
                    CommandPanelsPanel.SelectPanel(_results[0].Result.Id);
                }
            }

            if (_results.Count > 0)
            {
                if (_mode != LauncherMode.Command)
                {
                    ResultsList.SelectedIndex = 0;
                }
                return;
            }

            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;

        }

        private static IReadOnlyList<LauncherResult> FilterRows(IEnumerable<LauncherResult> source, string query)
        {
            if (string.IsNullOrWhiteSpace(query))
            {
                return source.OrderByDescending(item => item.Score).ToList();
            }

            return source.Where(item =>
                    item.Title.Contains(query, StringComparison.OrdinalIgnoreCase)
                    || item.Path.Contains(query, StringComparison.OrdinalIgnoreCase)
                    || (item.Subtitle?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false))
                .OrderByDescending(item => item.Score)
                .ToList();
        }

        private static IReadOnlyList<LauncherResult> FilterSearchNoise(IReadOnlyList<LauncherResult> source, int limit)
        {
            return source
                .Where(item => !ShouldHideSearchResult(item))
                .Take(limit)
                .ToList();
        }

        private static IReadOnlyList<LauncherResult> DeduplicatePairedAppEntries(IReadOnlyList<LauncherResult> source)
        {
            var output = new List<LauncherResult>(source.Count);
            var representativeIndexByKey = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

            foreach (var item in source)
            {
                var category = GetAppPathCategory(item);
                if (category == AppPathCategory.Other)
                {
                    output.Add(item);
                    continue;
                }

                string key = NormalizeAppIdentity(item.Title);
                if (string.IsNullOrEmpty(key))
                {
                    output.Add(item);
                    continue;
                }

                if (!representativeIndexByKey.TryGetValue(key, out int existingIndex))
                {
                    representativeIndexByKey[key] = output.Count;
                    output.Add(item);
                    continue;
                }

                var existing = output[existingIndex];
                var existingCategory = GetAppPathCategory(existing);

                if (!ArePairedDuplicateCategories(existingCategory, category))
                {
                    output.Add(item);
                    continue;
                }

                if (IsPreferredAppEntry(item, existing))
                    output[existingIndex] = item;
            }

            return output;
        }

        private static bool ArePairedDuplicateCategories(AppPathCategory a, AppPathCategory b)
        {
            return (a == AppPathCategory.StartMenuShortcut && b == AppPathCategory.InstallExecutable)
                || (a == AppPathCategory.InstallExecutable && b == AppPathCategory.StartMenuShortcut);
        }

        private static bool IsPreferredAppEntry(LauncherResult candidate, LauncherResult existing)
        {
            int candidateRank = GetAppEntryRank(candidate);
            int existingRank = GetAppEntryRank(existing);
            if (candidateRank != existingRank)
                return candidateRank > existingRank;

            return candidate.Score > existing.Score;
        }

        private static int GetAppEntryRank(LauncherResult item)
        {
            if (GetAppPathCategory(item) == AppPathCategory.InstallExecutable)
                return HasVersionMetadata(item.Path) ? 3 : 2;

            if (GetAppPathCategory(item) == AppPathCategory.StartMenuShortcut)
                return 1;

            return 0;
        }

        private static bool HasVersionMetadata(string path)
        {
            if (string.IsNullOrWhiteSpace(path))
                return false;

            string normalized = path.Replace('/', '\\');
            return ExeVersionInfoCache.GetOrAdd(normalized, p =>
            {
                try
                {
                    if (!File.Exists(p))
                        return false;

                    var info = FileVersionInfo.GetVersionInfo(p);
                    return !string.IsNullOrWhiteSpace(info.FileVersion)
                        || !string.IsNullOrWhiteSpace(info.ProductVersion);
                }
                catch
                {
                    return false;
                }
            });
        }

        private static AppPathCategory GetAppPathCategory(LauncherResult item)
        {
            if (!item.Kind.Equals("app", StringComparison.OrdinalIgnoreCase)
                || string.IsNullOrWhiteSpace(item.Path))
            {
                return AppPathCategory.Other;
            }

            string path = item.Path.Replace('/', '\\');
            if (path.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase)
                && path.Contains("\\Start Menu\\Programs\\", StringComparison.OrdinalIgnoreCase))
            {
                return AppPathCategory.StartMenuShortcut;
            }

            if (path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
                && (path.Contains("\\Program Files\\", StringComparison.OrdinalIgnoreCase)
                    || path.Contains("\\Program Files (x86)\\", StringComparison.OrdinalIgnoreCase)
                    || path.Contains("\\AppData\\Local\\Programs\\", StringComparison.OrdinalIgnoreCase)))
            {
                return AppPathCategory.InstallExecutable;
            }

            return AppPathCategory.Other;
        }

        private static string NormalizeAppIdentity(string title)
        {
            if (string.IsNullOrWhiteSpace(title))
                return string.Empty;

            return new string(title
                .ToLowerInvariant()
                .Where(char.IsLetterOrDigit)
                .ToArray());
        }

        private static bool ShouldHideSearchResult(LauncherResult item)
        {
            if (!item.Kind.Equals("app", StringComparison.OrdinalIgnoreCase)
                && !item.Kind.Equals("file", StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            if (string.IsNullOrWhiteSpace(item.Path))
                return false;

            string path = item.Path.Replace('/', '\\');
            if (!path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
                return false;

            string fileName = Path.GetFileNameWithoutExtension(path);
            if (string.IsNullOrWhiteSpace(fileName))
                return false;

            string normalizedName = fileName.ToLowerInvariant();
            return NoisyExecutableNameTokens.Any(token => normalizedName.Contains(token, StringComparison.Ordinal));
        }

        private enum AppPathCategory
        {
            Other,
            StartMenuShortcut,
            InstallExecutable,
        }

        private static IReadOnlyList<LauncherResult> BuildHelpRows()
        {
            return
            [
                new LauncherResult { Id = "help:search", Kind = "app", Title = "Search mode", Subtitle = "Blend apps, files, folders", Path = "help://search", Score = 1000 },
                new LauncherResult { Id = "help:command", Kind = "app", Title = "Command mode", Subtitle = "calc, shell, kill, sys", Path = "help://command", Score = 990 },
                new LauncherResult { Id = "help:clipboard", Kind = "clipboard", Title = "Clipboard mode", Subtitle = "session-local history", Path = "help://clipboard", Score = 980 },
            ];
        }

        private async void QueryInput_OnTextChanged(object sender, Microsoft.UI.Xaml.Controls.TextChangedEventArgs e)
        {
            int currentVersion = ++_searchVersion;
            string query = QueryInput.Text?.Trim() ?? string.Empty;

            await Task.Delay(8);
            if (currentVersion != _searchVersion)
            {
                return;
            }

            RefreshResults(query);
        }

        private void QueryInput_OnLoaded(object sender, RoutedEventArgs e)
        {
            if (_results.Count > 0 && ResultsList.SelectedIndex < 0)
            {
                ResultsList.SelectedIndex = 0;
            }

            QueryInput.Focus(FocusState.Programmatic);
        }

        private bool IsSettingsToggleShortcut(VirtualKey key)
        {
            return key == (VirtualKey)188 && IsCtrlPressed() && IsShiftPressed();
        }

        private bool IsCommandModeShortcut(KeyRoutedEventArgs e)
        {
            if (!IsCtrlPressed())
            {
                return false;
            }

            return e.Key == VirtualKey.Divide
                || e.Key == (VirtualKey)191
                || e.KeyStatus.ScanCode == 53;
        }

        private bool TryHandleCommandQuickSelect(VirtualKey key)
        {
            if (_mode != LauncherMode.Command || !IsCtrlPressed())
            {
                return false;
            }

            string? commandId = key switch
            {
                VirtualKey.Number1 => "command:shell",
                VirtualKey.Number2 => "command:calc",
                VirtualKey.Number3 => "command:kill",
                _ => null,
            };

            if (commandId is null)
            {
                return false;
            }

            CommandPanelsPanel.SelectPanel(commandId);
            return true;
        }

        private void GlobalKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == VirtualKey.H && IsCtrlPressed())
            {
                EnterHelpScreen();
                e.Handled = true;
                return;
            }

            if (IsCommandModeShortcut(e))
            {
                EnterCommandScreen();
                e.Handled = true;
                return;
            }

            if (TryHandleCommandQuickSelect(e.Key))
            {
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Escape && _mode == LauncherMode.Settings)
            {
                ToggleSettingsMode();
                e.Handled = true;
                return;
            }

            if (TryHandleCommandQuickSelect(e.Key))
            {
                e.Handled = true;
                return;
            }

            if (!IsSettingsToggleShortcut(e.Key))
            {
                return;
            }

            ToggleSettingsMode();
            e.Handled = true;
        }

        private void ToggleSettingsMode()
        {
            if (_mode == LauncherMode.Settings)
            {
                QueryInput.Text = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
                QueryInput.Focus(FocusState.Programmatic);
                QueryInput.SelectionStart = QueryInput.Text.Length;
                return;
            }

            SetMode(LauncherMode.Settings);
        }

        private void SettingsTabsPanel_OnCloseRequested(object? sender, EventArgs e)
        {
            if (_mode == LauncherMode.Settings)
            {
                ToggleSettingsMode();
            }
        }

        private void EnterCommandScreen()
        {
            QueryInput.Text = "/";
            SetMode(LauncherMode.Command);
            RefreshResults(QueryInput.Text);
            CommandPanelsPanel.SelectPanel("command:calc");

            QueryInput.Focus(FocusState.Programmatic);
            QueryInput.SelectionStart = QueryInput.Text.Length;
        }

        private void EnterHelpScreen()
        {
            if (_mode == LauncherMode.Help)
            {
                QueryInput.Text = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
                QueryInput.Focus(FocusState.Programmatic);
                return;
            }

            QueryInput.Text = string.Empty;
            SetMode(LauncherMode.Help);
            QueryInput.Focus(FocusState.Programmatic);
        }

        private void QueryInput_OnPreviewKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key != VirtualKey.Tab || _results.Count == 0)
            {
                return;
            }

            int selected = ResultsList.SelectedIndex;
            if (selected < 0)
            {
                ResultsList.SelectedIndex = IsShiftPressed() ? _results.Count - 1 : 0;
            }
            else if (IsShiftPressed())
            {
                ResultsList.SelectedIndex = selected > 0 ? selected - 1 : _results.Count - 1;
            }
            else
            {
                ResultsList.SelectedIndex = selected < _results.Count - 1 ? selected + 1 : 0;
            }

            e.Handled = true;
        }

        private void QueryInput_OnKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (IsCommandModeShortcut(e))
            {
                EnterCommandScreen();
                e.Handled = true;
                return;
            }

            if (_mode == LauncherMode.Command)
            {
                if (e.Key == VirtualKey.Down)
                {
                    CommandPanelsPanel.MoveSelection(1);
                    e.Handled = true;
                    return;
                }

                if (e.Key == VirtualKey.Up)
                {
                    CommandPanelsPanel.MoveSelection(-1);
                    e.Handled = true;
                    return;
                }

                if (e.Key == VirtualKey.Enter)
                {
                    RunCommand(CommandPanelsPanel.ActiveCommandId);
                    e.Handled = true;
                    return;
                }
            }

            if (e.Key == VirtualKey.Escape)
            {
                QueryInput.Text = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Down && _results.Count > 0)
            {
                int selected = ResultsList.SelectedIndex;
                if (selected < 0)
                {
                    ResultsList.SelectedIndex = 0;
                }
                else
                {
                    ResultsList.SelectedIndex = selected < _results.Count - 1 ? selected + 1 : 0;
                }
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Up && _results.Count > 0)
            {
                int selected = ResultsList.SelectedIndex;
                if (selected <= 0)
                {
                    ResultsList.SelectedIndex = _results.Count - 1;
                }
                else
                {
                    ResultsList.SelectedIndex = selected - 1;
                }
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Enter && IsCtrlPressed())
            {
                bool ok = _actionDispatcher.WebHandoff(QueryInput.Text ?? string.Empty);
                HintText.Text = ok
                    ? "Opened browser search  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Web handoff failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
            }
        }

        private void ResultsList_OnKeyDown(object sender, KeyRoutedEventArgs e)
        {
            if (e.Key == VirtualKey.Tab)
            {
                if (IsShiftPressed())
                {
                    if (ResultsList.SelectedIndex > 0)
                        ResultsList.SelectedIndex--;
                    else if (_results.Count > 0)
                        ResultsList.SelectedIndex = _results.Count - 1;
                }
                else
                {
                    if (ResultsList.SelectedIndex < _results.Count - 1)
                        ResultsList.SelectedIndex++;
                    else if (_results.Count > 0)
                        ResultsList.SelectedIndex = 0;
                }
                ResultsList.UpdateLayout();
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Up && ResultsList.SelectedIndex <= 0)
            {
                ResultsList.SelectedIndex = _results.Count - 1;
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.Enter && ResultsList.SelectedItem is LauncherRowItem selected)
            {
                HandlePrimaryAction(selected);
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.C && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem copySelected)
            {
                bool ok = _actionDispatcher.CopyResultPath(copySelected.Result);
                HintText.Text = ok
                    ? "Copied path/content  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Copy action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
                return;
            }

            if (e.Key == VirtualKey.R && IsCtrlPressed() && ResultsList.SelectedItem is LauncherRowItem revealSelected)
            {
                bool ok = _actionDispatcher.RevealResult(revealSelected.Result);
                HintText.Text = ok
                    ? "Revealed in Explorer  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                    : "Reveal action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
                e.Handled = true;
            }
        }

        private void ResultsList_OnSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
        {
            if (ResultsList.SelectedItem is LauncherRowItem selected)
            {
                ResultPreviewPanel.SetRow(selected);
                if (_mode == LauncherMode.Search || _mode == LauncherMode.Clipboard || _mode == LauncherMode.Help)
                {
                    ResultPreviewPanel.Visibility = Visibility.Visible;
                    PreviewDivider.Visibility = Visibility.Visible;
                }
                if (_mode == LauncherMode.Command)
                {
                    CommandPanelsPanel.SelectPanel(selected.Result.Id);
                }
                return;
            }

            ResultPreviewPanel.SetRow(null);
            ResultPreviewPanel.Visibility = Visibility.Collapsed;
            PreviewDivider.Visibility = Visibility.Collapsed;
        }

        private void HandlePrimaryAction(LauncherRowItem selected)
        {
            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.SelectPanel(selected.Result.Id);
                RunCommand(selected.Result.Id);
                return;
            }

            if (_mode == LauncherMode.Clipboard)
            {
                CopyText(selected.Result.Path);
                HintText.Text = "Copied clipboard item  •  Enter copy  •  Up/Down select  •  Esc clear";
                return;
            }

            bool ok = _actionDispatcher.OpenResult(selected.Result);
            HintText.Text = ok
                ? "Opened selected item  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                : "Open action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
        }

        private void RunCommand(string id)
        {
            switch (id)
            {
                case "command:calc":
                    HintText.Text = "calc executed in mock mode";
                    break;
                case "command:shell":
                    HintText.Text = "shell executed in mock mode";
                    break;
                case "command:kill":
                    HintText.Text = "kill requires confirmation flow (planned)";
                    break;
                case "command:sys":
                    HintText.Text = "sys panel opened in mock mode";
                    break;
                default:
                    HintText.Text = "Unknown command";
                    break;
            }
        }

        private static void CopyText(string value)
        {
            DataPackage package = new();
            package.SetText(value);
            Clipboard.SetContent(package);
        }

        private static bool IsCtrlPressed()
        {
            Windows.UI.Core.CoreVirtualKeyStates state = Microsoft.UI.Input.InputKeyboardSource
                .GetKeyStateForCurrentThread(VirtualKey.Control);
            return (state & Windows.UI.Core.CoreVirtualKeyStates.Down) == Windows.UI.Core.CoreVirtualKeyStates.Down;
        }

        private static bool IsShiftPressed()
        {
            Windows.UI.Core.CoreVirtualKeyStates state = Microsoft.UI.Input.InputKeyboardSource
                .GetKeyStateForCurrentThread(VirtualKey.Shift);
            return (state & Windows.UI.Core.CoreVirtualKeyStates.Down) == Windows.UI.Core.CoreVirtualKeyStates.Down;
        }
    }
}
