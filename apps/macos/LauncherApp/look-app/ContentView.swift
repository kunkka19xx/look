//
//  ContentView.swift
//  look-app
//
//  Created by kunkka07xx on 2026/04/04.
//

import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var themeStore: ThemeStore

    var body: some View {
        let minSize = WindowAutoScale.baseSize(for: themeStore.settings.layout)
        LauncherView()
            .frame(minWidth: minSize.width, minHeight: minSize.height)
            .motionPreference(animationsEnabled: themeStore.settings.animationsEnabled)
    }
}

#Preview {
    ContentView()
        .environmentObject(AppUIState())
        .environmentObject(ThemeStore())
}
