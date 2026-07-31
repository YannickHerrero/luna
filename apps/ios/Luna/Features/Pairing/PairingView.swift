import SwiftUI
import UIKit

struct PairingView: View {
    @Bindable var model: AppModel
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.lunaPalette) private var palette

    @State private var code = ""
    @State private var deviceName = UIDevice.current.name
    @State private var notice: String?
    @State private var localError: String?
    @State private var isRequestingCode = false
    @State private var isPairing = false
    @State private var isEditingServer = false
    @State private var serverInput = ""

    var body: some View {
        GeometryReader { geometry in
            ScrollView {
                VStack {
                    pairingCard
                }
                .frame(maxWidth: .infinity)
                .frame(minHeight: geometry.size.height)
                .padding(24)
            }
            .scrollDismissesKeyboard(.interactively)
            .background(LunaBackground())
        }
        .alert("Luna server", isPresented: $isEditingServer) {
            TextField("https://your-mac.example.ts.net:8447", text: $serverInput)
                .textInputAutocapitalization(.never)
                .keyboardType(.URL)
            Button("Cancel", role: .cancel) {}
            Button("Save") {
                Task { await saveServer() }
            }
        } message: {
            Text("Use the private HTTPS address exposed by Tailscale Serve.")
        }
    }

    private var pairingCard: some View {
        VStack(alignment: .leading, spacing: 0) {
            LunaMoonMark()
                .padding(.bottom, 25)

            Text("PRIVATE BY DESIGN")
                .lunaMonoFont(10, weight: .bold)
                .tracking(1.3)
                .foregroundStyle(palette.accent)
                .accessibilityHidden(true)

            Text("Pair with Luna")
                .font(LunaFont.display(40, weight: .bold))
                .tracking(-1.6)
                .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                .minimumScaleFactor(dynamicTypeSize.isAccessibilitySize ? 1 : 0.85)
                .foregroundStyle(palette.foreground)
                .padding(.vertical, 8)

            Text("Ask Luna for a one-time code, find the newest code in its Citadel logs, then enter it below. Your conversations stay on your Mac.")
                .font(LunaFont.body(13))
                .foregroundStyle(palette.muted)
                .lineSpacing(5)
                .padding(.bottom, 14)

            serverButton
                .padding(.bottom, 12)

            Button(action: requestCode) {
                HStack(spacing: 8) {
                    if isRequestingCode {
                        ProgressView()
                            .controlSize(.small)
                    }
                    Text(isRequestingCode ? "Requesting…" : "Ask for a pairing code")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(LunaSecondaryButtonStyle())
            .disabled(isRequestingCode || isPairing)
            .accessibilityIdentifier("pairing-request-code")

            if let notice {
                Text(notice)
                    .font(LunaFont.body(12))
                    .foregroundStyle(palette.foreground)
                    .lineSpacing(3)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(palette.green.opacity(0.14))
                    .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
                    .padding(.vertical, 12)
                    .accessibilityIdentifier("pairing-notice")
            } else {
                Spacer().frame(height: 14)
            }

            VStack(alignment: .leading, spacing: 6) {
                fieldLabel("PAIRING CODE")
                TextField("123456", text: $code)
                    .keyboardType(.numberPad)
                    .textContentType(.oneTimeCode)
                    .lunaField()
                    .onChange(of: code) { _, value in
                        code = String(value.filter(\.isNumber).prefix(6))
                    }
                    .accessibilityLabel("Pairing code")
                    .accessibilityIdentifier("pairing-code")
            }

            VStack(alignment: .leading, spacing: 6) {
                fieldLabel("DEVICE NAME")
                TextField("iPhone", text: $deviceName)
                    .textContentType(.name)
                    .lunaField()
                    .accessibilityLabel("Device name")
                    .accessibilityIdentifier("device-name")
            }
            .padding(.top, 14)

            Button(action: pair) {
                HStack(spacing: 8) {
                    if isPairing {
                        ProgressView()
                            .controlSize(.small)
                            .tint(palette.onAccent)
                    }
                    Text(isPairing ? "Pairing…" : "Pair device")
                }
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(LunaPrimaryButtonStyle())
            .disabled(!canPair || isPairing || isRequestingCode)
            .padding(.top, 14)
            .accessibilityIdentifier("pair-device")

            if let error = localError ?? model.errorMessage {
                Text(error)
                    .font(LunaFont.body(12))
                    .foregroundStyle(palette.red)
                    .padding(.top, 12)
                    .accessibilityIdentifier("pairing-error")
            }
        }
        .padding(horizontalSizeClass == .compact ? 24 : 40)
        .padding(.vertical, horizontalSizeClass == .compact ? 30 : 40)
        .frame(maxWidth: 430)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 28, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .shadow(color: palette.foreground.opacity(0.12), radius: 45, y: 30)
    }

    private var serverButton: some View {
        Button {
            serverInput = model.configuration.serverURL.absoluteString
            isEditingServer = true
        } label: {
            HStack(spacing: 8) {
                Circle()
                    .fill(palette.green)
                    .frame(width: 7, height: 7)
                Text(model.configuration.serverURL.host ?? model.configuration.serverURL.absoluteString)
                    .font(LunaFont.body(12, weight: .semibold))
                    .lineLimit(1)
                Spacer(minLength: 8)
                Text("Change")
                    .font(LunaFont.body(12, weight: .semibold))
                    .foregroundStyle(palette.accent)
            }
            .foregroundStyle(palette.muted)
            .padding(.horizontal, 12)
            .frame(minHeight: 36)
            .background(palette.background)
            .clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Luna server, \(model.configuration.serverURL.absoluteString). Change server")
    }

    private func fieldLabel(_ text: String) -> some View {
        Text(text)
            .font(LunaFont.body(11, weight: .bold))
            .foregroundStyle(palette.muted)
    }

    private var canPair: Bool {
        code.count == 6 && !deviceName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func requestCode() {
        isRequestingCode = true
        notice = nil
        localError = nil
        model.errorMessage = nil
        Task {
            defer { isRequestingCode = false }
            do {
                let response = try await model.requestPairingCode()
                code = ""
                notice = "A new code was written to Luna’s Citadel logs. It expires at \(formattedTime(response.expiresAt))."
            } catch {
                localError = message(from: error)
            }
        }
    }

    private func pair() {
        isPairing = true
        localError = nil
        model.errorMessage = nil
        Task {
            defer { isPairing = false }
            do {
                try await model.pair(
                    code: code,
                    deviceName: deviceName.trimmingCharacters(in: .whitespacesAndNewlines)
                )
            } catch {
                localError = message(from: error)
            }
        }
    }

    private func saveServer() async {
        localError = nil
        do {
            try await model.changeServer(to: serverInput)
        } catch {
            localError = message(from: error)
        }
    }

    private func formattedTime(_ value: String) -> String {
        let formatter = ISO8601DateFormatter()
        guard let date = formatter.date(from: value) else { return value }
        return date.formatted(date: .omitted, time: .shortened)
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}
