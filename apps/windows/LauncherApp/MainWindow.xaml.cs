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
using LauncherApp.Commands;
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
        private KillCommand.RunningApp? _pendingKillTarget;
        private SettingsTabsView? _settingsTabsView;
        private LauncherMode _mode = LauncherMode.Search;
        private int _searchVersion;
        private string _backdropMode = "Acrylic";
        private string _appliedBackdropMode = string.Empty;
        private Windows.UI.Color _acrylicTint = Windows.UI.Color.FromArgb(45, 21, 28, 38);
        private readonly TransparentTintBackdrop _transparentBackdrop;
        private Windows.UI.Color _frameBorderColor = Windows.UI.Color.FromArgb(0x66, 0x27, 0x34, 0x46);
        private double _frameBorderThickness = 0.15;
        private bool _frameBorderHidden;
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
            InitializeFrameBorderState();

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
            CommandPanelsPanel.CommandTextChanged += CommandPanelsPanel_OnCommandTextChanged;
            CommandPanelsPanel.ActiveCommandChanged += CommandPanelsPanel_OnActiveCommandChanged;
            CommandPanelsPanel.KillSelectionChanged += CommandPanelsPanel_OnKillSelectionChanged;
            CommandPanelsPanel.KillCandidateInvoked += CommandPanelsPanel_OnKillCandidateInvoked;
            CommandPanelsPanel.KillConfirmAccepted += CommandPanelsPanel_OnKillConfirmAccepted;
            CommandPanelsPanel.KillConfirmCancelled += CommandPanelsPanel_OnKillConfirmCancelled;
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
                new LauncherResult { Id = "command:shell", Kind = "app", Title = "shell", Subtitle = "Run shell command", Path = "command://shell", Score = 1000 },
                new LauncherResult { Id = "command:calc", Kind = "app", Title = "calc", Subtitle = "Evaluate expression", Path = "command://calc", Score = 990 },
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

            HwndExtensions.ToggleWindowStyle(
                WindowNative.GetWindowHandle(this),
                false,
                WindowStyle.TiledWindow);

            ExtendsContentIntoTitleBar = true;
            SetTitleBar(SearchBarHost);

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

        private void ApplyImmersiveDarkModeFrame()
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            int enabled = 1;
            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.UseImmersiveDarkMode,
                ref enabled,
                Marshal.SizeOf<int>());
        }

        public void UpdateFrameBorder(Windows.UI.Color color, double thickness)
        {
            _frameBorderColor = color;
            _frameBorderThickness = thickness;

            if (LauncherSurface is not null)
            {
                LauncherSurface.BorderThickness = new Thickness(thickness);
            }

            ApplyDwmBorderColor();
        }

        public void UpdateTopEdgeMask(Windows.UI.Color panelColor, Windows.UI.Color borderColor)
        {
            Windows.UI.Color maskColor = Windows.UI.Color.FromArgb(255, panelColor.R, panelColor.G, panelColor.B);
            ApplyDwmCaptionColor(panelColor);
            ApplyDwmBorderColor();
        }

        public void UpdateFrameCaptionColor(Windows.UI.Color panelColor)
        {
            ApplyDwmCaptionColor(panelColor);
        }

        public void UpdateBorderThickness(Thickness thickness)
        {
            UpdateFrameBorder(_frameBorderColor, thickness.Left);
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

            _frameBorderHidden = removeBorder;
            ApplyDwmBorderColor();
        }

        private void ApplyDwmBorderColor()
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            int borderColor = unchecked((int)0xFFFFFFFE);

            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.BorderColor,
                ref borderColor,
                Marshal.SizeOf<int>());
        }

        private void ApplyDwmCaptionColor(Windows.UI.Color panelColor)
        {
            IntPtr hwnd = WindowNative.GetWindowHandle(this);
            if (hwnd == IntPtr.Zero)
            {
                return;
            }

            int captionColor = BuildColorRef(panelColor.R, panelColor.G, panelColor.B);
            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.CaptionColor,
                ref captionColor,
                Marshal.SizeOf<int>());

            int textColor = BuildColorRef(0xE8, 0xED, 0xF7);
            _ = DwmSetWindowAttribute(
                hwnd,
                (int)DwmWindowAttribute.TextColor,
                ref textColor,
                Marshal.SizeOf<int>());
        }

        private static int BuildColorRef(byte red, byte green, byte blue)
        {
            return red | (green << 8) | (blue << 16);
        }

        private void InitializeFrameBorderState()
        {
            if (Application.Current?.Resources is not ResourceDictionary resources)
            {
                ApplyDwmBorderColor();
                return;
            }

            Windows.UI.Color panelColor = Windows.UI.Color.FromArgb(45, 21, 28, 38);

            if (resources.ContainsKey("LauncherColorBorder") && resources["LauncherColorBorder"] is Windows.UI.Color color)
            {
                _frameBorderColor = color;
            }

            if (resources.ContainsKey("LauncherColorPanel") && resources["LauncherColorPanel"] is Windows.UI.Color panel)
            {
                panelColor = panel;
            }

            if (resources.ContainsKey("LauncherBorderThickness") && resources["LauncherBorderThickness"] is Thickness thickness)
            {
                _frameBorderThickness = thickness.Left;
            }

            UpdateFrameBorder(_frameBorderColor, _frameBorderThickness);
            UpdateTopEdgeMask(panelColor, _frameBorderColor);
            UpdateFrameCaptionColor(panelColor);
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
            UseImmersiveDarkMode = 20,
            WindowCornerPreference = 33,
            BorderColor = 34,
            CaptionColor = 35,
            TextColor = 36,
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

        private (LauncherMode mode, string normalizedQuery) ResolveMode(string rawQuery)
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

            if (_mode == LauncherMode.Command)
            {
                return (LauncherMode.Command, query);
            }

            return (LauncherMode.Search, query);
        }

        private void SetMode(LauncherMode mode)
        {
            _mode = mode;

            if (mode != LauncherMode.Command)
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
            }

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
                    SearchBarHost.Visibility = Visibility.Collapsed;
                    HintText.Text = "Enter run  •  Ctrl+1..4 switch  •  Y/N confirm kill";
                    ResultsList.Visibility = Visibility.Collapsed;
                    ResultPreviewPanel.Visibility = Visibility.Collapsed;
                    CommandPanelsPanel.Visibility = Visibility.Visible;
                    CommandPanelsPanel.ApplyFilter(string.Empty);
                    CommandPanelsPanel.SelectPanel("command:calc");
                    CommandPanelsPanel.CommandInputText = string.Empty;
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

            if (_mode == LauncherMode.Command)
            {
                query = CommandPanelsPanel.CommandInputText.Trim();
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
                try
                {
                    CommandPanelsPanel.ApplyFilter(string.Empty);
                    CommandPanelsPanel.SelectPanel(ResolveCommandId(CommandPanelsPanel.ActiveCommandId));
                }
                catch
                {
                    CommandPanelsPanel.SelectPanel("command:calc");
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

            try
            {
                RefreshResults(query);
                UpdateCommandPreview();
            }
            catch (Exception ex)
            {
                _pendingKillTarget = null;
                if (_mode == LauncherMode.Command)
                {
                    CommandPanelsPanel.SetExecutionFeedback($"Input update failed: {ex.Message}", isError: true);
                }
            }
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
                VirtualKey.Number4 => "command:sys",
                _ => null,
            };

            if (commandId is null)
            {
                return false;
            }

            CommandPanelsPanel.SelectPanel(commandId);
            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
            return true;
        }

        private bool TryHandleCommandModeKey(VirtualKey key)
        {
            if (_mode != LauncherMode.Command)
            {
                return false;
            }

            bool inKillCommand = CommandPanelsPanel.ActiveCommandId == "command:kill";

            if (key == VirtualKey.Tab)
            {
                int direction = IsShiftPressed() ? -1 : 1;
                CommandPanelsPanel.MoveSelection(direction);
                UpdateCommandPreview();
                CommandPanelsPanel.FocusCommandInput();
                return true;
            }

            if (inKillCommand && _pendingKillTarget is not null)
            {
                if (key == VirtualKey.Y)
                {
                    ConfirmPendingKill();
                    return true;
                }

                if (key == VirtualKey.N)
                {
                    CancelPendingKill();
                    return true;
                }
            }

            if (key == VirtualKey.Down)
            {
                if (inKillCommand)
                {
                    CommandPanelsPanel.MoveKillSelection(1);
                }
                else
                {
                    CommandPanelsPanel.MoveSelection(1);
                }

                UpdateCommandPreview();
                CommandPanelsPanel.FocusCommandInput();
                return true;
            }

            if (key == VirtualKey.Up)
            {
                if (inKillCommand)
                {
                    CommandPanelsPanel.MoveKillSelection(-1);
                }
                else
                {
                    CommandPanelsPanel.MoveSelection(-1);
                }

                UpdateCommandPreview();
                CommandPanelsPanel.FocusCommandInput();
                return true;
            }

            if (key == VirtualKey.Enter)
            {
                if (inKillCommand && _pendingKillTarget is not null)
                {
                    ConfirmPendingKill();
                    return true;
                }

                _ = RunCommandAsync(CommandPanelsPanel.ActiveCommandId);
                return true;
            }

            if (key == VirtualKey.Escape)
            {
                if (inKillCommand && _pendingKillTarget is not null)
                {
                    CancelPendingKill();
                    return true;
                }

                CommandPanelsPanel.CommandInputText = string.Empty;
                SetMode(LauncherMode.Search);
                RefreshResults(string.Empty);
                QueryInput.Focus(FocusState.Programmatic);
                return true;
            }

            return false;
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

            if (TryHandleCommandModeKey(e.Key))
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
            CommandPanelsPanel.CommandInputText = string.Empty;
            SetMode(LauncherMode.Command);
            RefreshResults(string.Empty);
            CommandPanelsPanel.SelectPanel("command:calc");
            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
        }

        private void CommandPanelsPanel_OnCommandTextChanged(object? sender, EventArgs e)
        {
            if (_mode != LauncherMode.Command)
            {
                return;
            }

            RefreshResults(CommandPanelsPanel.CommandInputText);
            UpdateCommandPreview();
        }

        private void CommandPanelsPanel_OnActiveCommandChanged(object? sender, EventArgs e)
        {
            if (_mode != LauncherMode.Command)
            {
                return;
            }

            UpdateCommandPreview();
            CommandPanelsPanel.FocusCommandInput();
        }

        private void CommandPanelsPanel_OnKillSelectionChanged(object? sender, EventArgs e)
        {
            if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
            {
                return;
            }

            var selected = CommandPanelsPanel.GetSelectedKillCandidate();
            if (selected is null)
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
                return;
            }

            _pendingKillTarget = selected.App;
            CommandPanelsPanel.ShowKillConfirmation(selected);
            HintText.Text = "Press Y to confirm kill, N to cancel";
        }

        private void CommandPanelsPanel_OnKillCandidateInvoked(object? sender, EventArgs e)
        {
            if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
            {
                return;
            }

            var selected = CommandPanelsPanel.GetSelectedKillCandidate();
            if (selected is null)
            {
                return;
            }

            _pendingKillTarget = selected.App;
            CommandPanelsPanel.ShowKillConfirmation(selected);
            HintText.Text = "Press Y to confirm kill, N to cancel";
        }

        private void CommandPanelsPanel_OnKillConfirmAccepted(object? sender, EventArgs e)
        {
            if (_mode == LauncherMode.Command && CommandPanelsPanel.ActiveCommandId == "command:kill")
            {
                ConfirmPendingKill();
            }
        }

        private void CommandPanelsPanel_OnKillConfirmCancelled(object? sender, EventArgs e)
        {
            if (_mode == LauncherMode.Command && CommandPanelsPanel.ActiveCommandId == "command:kill")
            {
                CancelPendingKill();
            }
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
                if (CommandPanelsPanel.ActiveCommandId == "command:kill" && _pendingKillTarget is not null)
                {
                    if (e.Key == VirtualKey.Y)
                    {
                        var killResult = KillCommand.ConfirmKill(_pendingKillTarget);
                        _pendingKillTarget = null;
                        CommandPanelsPanel.SetExecutionFeedback(killResult.message, isError: !killResult.ok);
                        HintText.Text = killResult.ok ? "kill executed" : "kill failed";
                        e.Handled = true;
                        return;
                    }

                    if (e.Key == VirtualKey.N)
                    {
                        _pendingKillTarget = null;
                        CommandPanelsPanel.SetExecutionFeedback("Kill canceled");
                        HintText.Text = "kill canceled";
                        e.Handled = true;
                        return;
                    }
                }

                if (e.Key == VirtualKey.Down)
                {
                    CommandPanelsPanel.MoveSelection(1);
                    UpdateCommandPreview();
                    e.Handled = true;
                    return;
                }

                if (e.Key == VirtualKey.Up)
                {
                    CommandPanelsPanel.MoveSelection(-1);
                    UpdateCommandPreview();
                    e.Handled = true;
                    return;
                }

                if (e.Key == VirtualKey.Enter)
                {
                    _ = RunCommandAsync(CommandPanelsPanel.ActiveCommandId);
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
                return;
            }

            if (e.Key == VirtualKey.Enter && ResultsList.SelectedItem is LauncherRowItem enterSelected)
            {
                HandlePrimaryAction(enterSelected, forceNewWindow: IsShiftPressed());
                e.Handled = true;
            }
        }

        private void ResultsList_OnItemClick(object sender, ItemClickEventArgs e)
        {
            if (e.ClickedItem is LauncherRowItem clickedRow)
            {
                HandlePrimaryAction(clickedRow, forceNewWindow: IsShiftPressed());
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
                HandlePrimaryAction(selected, forceNewWindow: IsShiftPressed());
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

        private void HandlePrimaryAction(LauncherRowItem selected, bool forceNewWindow = false)
        {
            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.SelectPanel(selected.Result.Id);
                _ = RunCommandAsync(selected.Result.Id);
                return;
            }

            if (_mode == LauncherMode.Clipboard)
            {
                CopyText(selected.Result.Path);
                HintText.Text = "Copied clipboard item  •  Enter copy  •  Up/Down select  •  Esc clear";
                return;
            }

            bool ok = _actionDispatcher.OpenResult(selected.Result, forceNewWindow);
            HintText.Text = ok
                ? "Opened selected item  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy"
                : "Open action failed  •  Enter open  •  Ctrl+R reveal  •  Ctrl+C copy";
        }

        private void ConfirmPendingKill()
        {
            if (_pendingKillTarget is null)
            {
                CommandPanelsPanel.SetExecutionFeedback("Select a running app first", isError: true);
                return;
            }

            var target = _pendingKillTarget;
            var killResult = KillCommand.ConfirmKill(target);
            _pendingKillTarget = null;
            string args = ResolveCommandArgs("command:kill");
            string refreshQuery = args;
            if (killResult.ok && !string.IsNullOrWhiteSpace(args))
            {
                CommandPanelsPanel.CommandInputText = string.Empty;
                refreshQuery = string.Empty;
            }

            RefreshKillCandidates(refreshQuery, preserveFeedback: true);
            CommandPanelsPanel.HideKillConfirmation();
            CommandPanelsPanel.SetExecutionFeedback(killResult.message, isError: !killResult.ok);
            HintText.Text = killResult.ok ? "kill executed" : "kill failed";
            CommandPanelsPanel.FocusCommandInput();

            if (killResult.ok)
            {
                _ = RefreshKillCandidatesAfterDelayAsync(refreshQuery, 260);
            }
        }

        private void CancelPendingKill()
        {
            _pendingKillTarget = null;
            CommandPanelsPanel.HideKillConfirmation();
            CommandPanelsPanel.SetExecutionFeedback("Kill canceled");
            HintText.Text = "kill canceled";
        }

        private async Task RefreshKillCandidatesAfterDelayAsync(string query, int delayMs)
        {
            await Task.Delay(delayMs);
            if (_mode != LauncherMode.Command || CommandPanelsPanel.ActiveCommandId != "command:kill")
            {
                return;
            }

            DispatcherQueue.TryEnqueue(() => RefreshKillCandidates(query, preserveFeedback: true));
        }

        private void RefreshKillCandidates(string query, bool preserveFeedback = false)
        {
            var selectedCandidate = CommandPanelsPanel.GetSelectedKillCandidate();
            int? selectedPid = selectedCandidate?.App.Pid;
            bool isPortQuery = query.TrimStart().StartsWith(":", StringComparison.Ordinal)
                || query.TrimStart().StartsWith("port ", StringComparison.OrdinalIgnoreCase);

            var apps = KillCommand.ListRunningApps(query).Take(20).ToList();
            var items = apps.Select(app => new KillCandidateItem(app)).ToList();

            int? selectedNumber = selectedPid is int pid
                ? items.FirstOrDefault(x => x.App.Pid == pid)?.Number
                : null;

            CommandPanelsPanel.SetKillCandidates(items, selectedNumber);

            foreach (var item in items.Take(12))
            {
                _ = item.LoadIconAsync();
            }

            if (items.Count == 0)
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
                CommandPanelsPanel.SetKillEmptyMessage(
                    isPortQuery
                        ? "No process listening on this port"
                        : "No matching apps. Type app name or use :3000");
                if (!preserveFeedback)
                {
                    CommandPanelsPanel.SetExecutionFeedback(
                        isPortQuery ? "No process listening on this port" : "No matching apps. Type app name or use :3000",
                        isError: true);
                    HintText.Text = "No running match";
                }
                return;
            }

            CommandPanelsPanel.SetKillEmptyMessage(null);

            if (_pendingKillTarget is not null)
            {
                var pending = items.FirstOrDefault(x => x.App.Pid == _pendingKillTarget.Pid);
                if (pending is null)
                {
                    _pendingKillTarget = null;
                    CommandPanelsPanel.HideKillConfirmation();
                }
                else
                {
                    CommandPanelsPanel.ShowKillConfirmation(pending);
                }
            }

            if (!preserveFeedback)
            {
                if (string.IsNullOrWhiteSpace(query))
                {
                    CommandPanelsPanel.SetExecutionFeedback("Running apps. Use Up/Down or mouse to select, then confirm.");
                }
                else
                {
                    CommandPanelsPanel.SetExecutionFeedback("Filtered running apps. Select one and confirm kill.");
                }
            }
        }

        private async Task RunCommandAsync(string id)
        {
            string resolvedCommandId = ResolveCommandId(id);
            string args = ResolveCommandArgs(resolvedCommandId);
            CommandPanelsPanel.SelectPanel(resolvedCommandId);

            if (resolvedCommandId != "command:kill")
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.HideKillConfirmation();
            }

            switch (resolvedCommandId)
            {
                case "command:calc":
                    if (CalcCommand.TryEvaluate(args, out string calcMessage))
                    {
                        CommandPanelsPanel.SetExecutionFeedback(calcMessage);
                        HintText.Text = "calc executed";
                    }
                    else
                    {
                        CommandPanelsPanel.SetExecutionFeedback(calcMessage, isError: true);
                        HintText.Text = "calc error";
                    }
                    break;
                case "command:shell":
                    CommandPanelsPanel.SetExecutionFeedback("Running...");
                    var shellResult = await ShellCommand.RunAsync(args);
                    CommandPanelsPanel.SetExecutionFeedback(shellResult.message, isError: !shellResult.ok);
                    HintText.Text = shellResult.ok ? "shell executed" : "shell failed";
                    break;
                case "command:kill":
                    RefreshKillCandidates(args);
                    var selected = CommandPanelsPanel.GetSelectedKillCandidate();
                    if (selected is null)
                    {
                        HintText.Text = "kill command";
                        break;
                    }

                    _pendingKillTarget = selected.App;
                    CommandPanelsPanel.ShowKillConfirmation(selected);
                    HintText.Text = "Press Y to confirm kill, N to cancel";
                    break;
                case "command:sys":
                    string sysSummary = SystemInfoCommand.BuildSummary();
                    CommandPanelsPanel.SetExecutionFeedback(sysSummary);
                    HintText.Text = "sys info updated";
                    break;
                default:
                    CommandPanelsPanel.SetExecutionFeedback("Unknown command. Try shell, calc, kill, or sys", isError: true);
                    HintText.Text = "Unknown command";
                    break;
            }
        }

        private void UpdateCommandPreview()
        {
            if (_mode != LauncherMode.Command)
            {
                return;
            }

            try
            {
                string resolvedCommandId = ResolveCommandId(CommandPanelsPanel.ActiveCommandId);
                string args = ResolveCommandArgs(resolvedCommandId);
                CommandPanelsPanel.SelectPanel(resolvedCommandId);

                if (resolvedCommandId != "command:kill")
                {
                    _pendingKillTarget = null;
                    CommandPanelsPanel.HideKillConfirmation();
                }

                switch (resolvedCommandId)
                {
                    case "command:calc":
                        if (string.IsNullOrWhiteSpace(args))
                        {
                            CommandPanelsPanel.SetExecutionFeedback(string.Empty);
                            return;
                        }

                        if (CalcCommand.TryEvaluate(args, out string calcMessage))
                        {
                            CommandPanelsPanel.SetExecutionFeedback(calcMessage);
                        }
                        else
                        {
                            CommandPanelsPanel.SetExecutionFeedback(calcMessage, isError: true);
                        }
                        return;
                    case "command:shell":
                        CommandPanelsPanel.SetExecutionFeedback(
                            string.IsNullOrWhiteSpace(args)
                                ? "Enter to run shell command"
                                : $"Ready to run: {args}");
                        return;
                    case "command:kill":
                        RefreshKillCandidates(args);
                        if (_pendingKillTarget is not null)
                        {
                            HintText.Text = "Press Y to confirm kill, N to cancel";
                        }
                        else
                        {
                            HintText.Text = "Select app, then confirm kill";
                        }
                        return;
                    case "command:sys":
                        CommandPanelsPanel.SetExecutionFeedback(SystemInfoCommand.BuildSummary());
                        return;
                    default:
                        CommandPanelsPanel.SetExecutionFeedback("Unknown command", isError: true);
                        return;
                }
            }
            catch (Exception ex)
            {
                _pendingKillTarget = null;
                CommandPanelsPanel.SetExecutionFeedback($"Preview failed: {ex.Message}", isError: true);
            }
        }

        private string ResolveCommandId(string fallbackId)
        {
            string body = GetCommandInputBody();
            if (body.StartsWith('/'))
                body = body[1..].Trim();

            if (string.IsNullOrWhiteSpace(body))
                return fallbackId;

            string firstToken = body.Split(' ', StringSplitOptions.RemoveEmptyEntries).FirstOrDefault() ?? string.Empty;
            return firstToken.ToLowerInvariant() switch
            {
                "calc" => "command:calc",
                "shell" => "command:shell",
                "kill" => "command:kill",
                "sys" => "command:sys",
                _ => fallbackId,
            };
        }

        private string ResolveCommandArgs(string commandId)
        {
            string body = GetCommandInputBody();
            if (body.StartsWith('/'))
                body = body[1..].Trim();

            if (string.IsNullOrWhiteSpace(body))
                return string.Empty;

            string expected = commandId switch
            {
                "command:calc" => "calc",
                "command:shell" => "shell",
                "command:kill" => "kill",
                "command:sys" => "sys",
                _ => string.Empty,
            };

            if (string.IsNullOrWhiteSpace(expected))
                return string.Empty;

            if (body.Equals(expected, StringComparison.OrdinalIgnoreCase))
                return string.Empty;

            if (body.StartsWith(expected + " ", StringComparison.OrdinalIgnoreCase))
                return body[(expected.Length + 1)..].Trim();

            if (!body.Contains(' '))
            {
                return commandId == "command:sys" && body.Equals("sys", StringComparison.OrdinalIgnoreCase)
                    ? string.Empty
                    : body;
            }

            return body;
        }

        private string GetCommandInputBody()
        {
            if (_mode == LauncherMode.Command)
            {
                return CommandPanelsPanel.CommandInputText.Trim();
            }

            return QueryInput.Text?.Trim() ?? string.Empty;
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
