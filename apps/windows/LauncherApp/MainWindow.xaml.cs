using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Commands;
using LauncherApp.Core;
using LauncherApp.Features.Search;
using LauncherApp.Services;
using LauncherApp.Views.Settings;
using Microsoft.UI.Composition;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Hosting;
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
        private readonly UwpAppService _uwpAppService;
        private readonly TranslationService _translationService;
        private readonly ObservableCollection<LauncherRowItem> _results;
        private readonly List<LauncherResult> _commandSeed;
        private CancellationTokenSource? _translateCts;
        private ClipboardHistoryService? _clipboardHistory;
        private KillCommand.RunningApp? _pendingKillTarget;
        private SettingsTabsView? _settingsTabsView;
        private LauncherMode _mode = LauncherMode.Search;
        private int _searchVersion;
        // Multi-pick state (Ctrl+P toggles, Ctrl+Shift+P clears). Mirrors macOS pickedKeys /
        // pickedResultsByKey in LauncherView.swift. Picks live for the launcher session and
        // are independent of the visible result list / current query.
        private readonly List<string> _pickedKeys = new();
        private readonly Dictionary<string, LauncherResult> _pickedResultsByKey = new();
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

        private static readonly string[] JunkPathSegments =
        [
            "\\$RECYCLE.BIN\\",
            "\\System Volume Information\\",
            "\\$WINDOWS.~BT\\",
            "\\$WINDOWS.~WS\\",
            "\\Config.Msi\\",
            "\\PerfLogs\\",
            "\\Recovery\\",
            "\\$GetCurrent\\",
            "\\$SysReset\\",
            "\\$INPLACE.~TR\\",
            // Direct UWP package exes - inaccessible to user, duplicates AppsFolder entries.
            "\\WindowsApps\\",
        ];

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
            EngineBridge engineBridge = new EngineBridge();
            ISearchProvider searchProvider = useRealSearch
                ? new FfiSearchProvider(engineBridge)
                : new MockSearchProvider();

            _searchLogic = new LauncherSearchLogic(searchProvider);
            _actionDispatcher = new ActionDispatcher(new ShellExecuteService(), new ExplorerRevealService());
            _uwpAppService = new UwpAppService();
            _translationService = new TranslationService(engineBridge);
            _uwpAppService.BeginInitialize();
            _results = new ObservableCollection<LauncherRowItem>();
            _commandSeed = BuildCommandSeed();

            ResultsList.ItemsSource = _results;
            CommandPanelsPanel.CommandTextChanged += CommandPanelsPanel_OnCommandTextChanged;
            CommandPanelsPanel.ActiveCommandChanged += CommandPanelsPanel_OnActiveCommandChanged;
            CommandPanelsPanel.KillSelectionChanged += CommandPanelsPanel_OnKillSelectionChanged;
            CommandPanelsPanel.KillCandidateInvoked += CommandPanelsPanel_OnKillCandidateInvoked;
            CommandPanelsPanel.KillConfirmAccepted += CommandPanelsPanel_OnKillConfirmAccepted;
            CommandPanelsPanel.KillConfirmCancelled += CommandPanelsPanel_OnKillConfirmCancelled;
            TranslatePanel.OpenInBrowserRequested += TranslatePanel_OnOpenInBrowserRequested;
            TranslatePanel.CopyTranslatedRequested += TranslatePanel_OnCopyTranslatedRequested;
            ResultPreviewPanel.ClipboardDeleteRequested += OnClipboardDeleteRequested;
            PickedItemsPanel.RemoveRequested += OnPickedPanelRemoveRequested;
            PickedItemsPanel.ClearAllRequested += OnPickedPanelClearAllRequested;
            SetMode(LauncherMode.Search);
            RefreshResults(QueryInput.Text?.Trim() ?? string.Empty);
            InitializeBlurLayer();
            LoadBackgroundImageFromConfig();
            ApplySettingsBlurFromConfig();
            ApplyBlurOpacityFromConfig();
            SetBlurStyle(LookConfig.Get("ui_blur_material") ?? "balanced");
            InitializeGlobalHotkeys();
            InitializeClipboardHistory();
            InitializeDevHintBadge();
            this.Activated += OnWindowActivated;
            this.Closed += OnWindowClosed;
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
                presenter.SetBorderAndTitleBar(false, false);
            }

            HwndExtensions.ToggleWindowStyle(
                WindowNative.GetWindowHandle(this),
                false,
                WindowStyle.TiledWindow);

            HideFromTaskbarAndAltTab();

            ExtendsContentIntoTitleBar = true;
            SetTitleBar(SearchBarHost);

            if (Content is FrameworkElement root)
            {
                root.RequestedTheme = ElementTheme.Dark;
            }

            ApplyAcrylicBackdrop();

            ApplyRuntimeIcon();
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
            PickedItemsPanel.Visibility = Visibility.Collapsed;
            CommandPanelsPanel.Visibility = Visibility.Collapsed;
            HelpScreenPanel.Visibility = Visibility.Collapsed;
            TranslatePanel.Visibility = Visibility.Collapsed;
            ResultsList.Visibility = Visibility.Visible;

            switch (mode)
            {
                case LauncherMode.Search:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Search apps";
                    HintText.Text = "Enter open  •  Ctrl+F reveal  •  Ctrl+C copy  •  Ctrl+P pick  •  Ctrl+Enter web";
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
                    QueryInput.PlaceholderText = "Keyboard help";
                    HintText.Text = "Ctrl+H close  •  Esc hide  •  Ctrl+/ command mode";
                    ResultsList.Visibility = Visibility.Collapsed;
                    HelpScreenPanel.Visibility = Visibility.Visible;
                    break;
                case LauncherMode.Translate:
                    ApplyConfiguredSurface();
                    QueryInput.PlaceholderText = "Use t\" to translate text";
                    HintText.Text = "Press Enter to translate  •  Browser button opens Google Translate  •  Esc clear";
                    ResultsList.Visibility = Visibility.Collapsed;
                    TranslatePanel.Visibility = Visibility.Visible;
                    break;
            }

            // Re-show the picked-items panel when re-entering a mode that uses the right column.
            // RefreshPickedSidePanel internally restores the standard preview when there are no
            // picks, so this is also the path that re-binds preview to the current selection.
            if (mode == LauncherMode.Search || mode == LauncherMode.Clipboard || mode == LauncherMode.Help)
            {
                RefreshPickedSidePanel();
            }

            // Lazily-mounted panels (Settings, Command) come into the visual tree only
            // after the user enters their mode, so the initial zoom walk in ZoomIn/Out/Reset
            // misses them. Re-walk after each mode switch so font scale stays consistent.
            ReapplyUiScaleAfterModeSwitch();
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

    }
}
