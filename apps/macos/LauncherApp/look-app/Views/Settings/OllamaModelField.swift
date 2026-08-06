import SwiftUI

/// Ollama model chooser (control only, no label) that lists the models actually
/// installed on the local daemon (`GET /api/tags`), so the user picks instead of
/// typing a name that may not exist. Falls back to a free-text field when the
/// daemon is unreachable.
struct OllamaModelField: View {
    let host: String
    @Binding var model: String
    let themeStore: ThemeStore

    @State private var installed: [String] = []

    var body: some View {
        Group {
            if installed.isEmpty {
                TextField("llama3.1", text: $model)
                    .textFieldStyle(.roundedBorder)
            } else {
                Menu {
                    ForEach(installed, id: \.self) { name in
                        Button {
                            model = name
                        } label: {
                            if name == model {
                                Label(name, systemImage: "checkmark")
                            } else {
                                Text(name)
                            }
                        }
                    }
                } label: {
                    Text(model.isEmpty ? "Select a model" : model)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .onAppear(perform: refresh)
        .onChange(of: host) { _, _ in refresh() }
    }

    private func refresh() {
        Task {
            installed = await OllamaProvider.listModels(host: host)
        }
    }
}
