import Foundation

enum HintText {
    enum Launcher {
        static let normal = "Enter open  •  Cmd+F reveal  •  Cmd+H help  •  Cmd+/ command mode"
        static let command = "Tab select command  •  Cmd+1/2/3/4 switch  •  Enter run  •  Esc back  •  Cmd+Shift+, settings"
        static let kill = "Up/Down results  •  Type :3000 for port  •  Cmd+1/2/3/4 switch  •  Y/N confirm"
        static let sys = "Sys info view  •  Cmd+1/2/3/4 switch  •  Esc back"
    }

    enum Settings {
        static let advancedApply = "Save Config applies changes immediately. Cmd+Shift+; is only needed after editing .look.config manually."
        static let shortcutsTips = "Tips: t\"word for web EN/VI/JA translation | tw\"word for dictionary lookup panel | /kill to force quit apps"
    }
}
