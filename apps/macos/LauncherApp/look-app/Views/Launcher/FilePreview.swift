import SwiftUI

/// A file's contents, however it previews best: text and source render in
/// place, everything else goes through Quick Look. Extracted from the result
/// preview pane so the `@`-mention list can show the same thing while picking.
struct FilePreview: View {
    let path: String
    var maxHeight: CGFloat = .infinity

    var body: some View {
        if QuickLookPreviewService.isTextFile(path: path) {
            TextFilePreview(path: path, maxHeight: maxHeight)
        } else {
            QuickLookPreviewImage(path: path, maxHeight: maxHeight)
        }
    }
}
