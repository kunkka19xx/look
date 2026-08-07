import AppKit
import SwiftUI

extension View {
    /// Shows the pointing-hand cursor on hover when `enabled`, so a clickable
    /// piece of text reads as clickable.
    @ViewBuilder
    func pointingHandCursor(enabled: Bool = true) -> some View {
        if enabled {
            onHover { inside in
                if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() }
            }
        } else {
            self
        }
    }
}
