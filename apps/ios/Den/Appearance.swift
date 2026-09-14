import CoreText
import DenAPI
import Observation
import SwiftUI
import UIKit

/// Account appearance is generated from den-core; this store owns only local resolution and cache.
@MainActor @Observable
final class ThemeStore {
    private(set) var origin: URL
    private(set) var appearance: Components.Schemas.Appearance
    let builtins: [Components.Schemas.Theme]
    @ObservationIgnored private let defaults: UserDefaults

    init(origin: URL, defaults: UserDefaults = .standard) {
        self.origin = origin
        self.defaults = defaults
        builtins = Self.loadBuiltins()
        appearance = Self.cached(origin: origin, defaults: defaults) ?? Self.defaultAppearance
        DenFonts.register()
    }

    static var defaultAppearance: Components.Schemas.Appearance {
        .init(customThemes: [], mode: .system, theme: "den")
    }

    var allFamilies: [Components.Schemas.Theme] { builtins + appearance.customThemes }

    var selectedFamily: Components.Schemas.Theme {
        allFamilies.first { $0.id == appearance.theme } ?? builtins[0]
    }

    /// nil lets the system continue reacting to appearance changes while the app is running.
    var preferredColorScheme: ColorScheme? {
        switch appearance.mode {
        case .light: .light
        case .dark: .dark
        case .system: nil
        }
    }

    func resolve(_ system: ColorScheme) -> DenTheme {
        DenTheme(family: selectedFamily, scheme: preferredColorScheme ?? system)
    }

    /// Pass the request's origin when receiving an asynchronous response after a server switch.
    func receive(_ value: Components.Schemas.Appearance, from source: URL? = nil) {
        if let source, Self.cacheKey(source) != Self.cacheKey(origin) { return }
        appearance = value
        if let data = try? JSONEncoder().encode(value) {
            defaults.set(data, forKey: Self.cacheKey(origin))
        }
    }

    func reset(origin: URL) {
        self.origin = origin
        appearance = Self.cached(origin: origin, defaults: defaults) ?? Self.defaultAppearance
    }

    private static func cached(origin: URL, defaults: UserDefaults) -> Components.Schemas.Appearance? {
        guard let data = defaults.data(forKey: cacheKey(origin)) else { return nil }
        return try? JSONDecoder().decode(Components.Schemas.Appearance.self, from: data)
    }

    private static func cacheKey(_ origin: URL) -> String {
        "den.appearance:\(origin.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/")))"
    }

    static func loadBuiltins() -> [Components.Schemas.Theme] {
        guard let url = Bundle.main.url(forResource: "themes", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let families = try? JSONDecoder().decode([Components.Schemas.Theme].self, from: data),
              !families.isEmpty, families[0].id == "den" else {
            preconditionFailure("Missing shared theme resources. Run scripts/sync-resources.py.")
        }
        return families
    }
}

/// SwiftUI presentation of the selected shared palette, with no second theme schema.
@MainActor
struct DenTheme {
    let family: Components.Schemas.Theme
    let scheme: ColorScheme

    var colors: Components.Schemas.ThemeColors { scheme == .dark ? family.dark : family.light }
    var bg: Color { color(colors.bg) }
    var bg2: Color { color(colors.bg2) }
    var bg3: Color { color(colors.bg3) }
    var line: Color { color(colors.line) }
    var ink: Color { color(colors.ink) }
    var ink2: Color { color(colors.ink2) }
    var ink3: Color { color(colors.ink3) }
    var accent: Color { color(colors.accent) }
    var success: Color { color(colors.success) }
    var danger: Color { color(colors.danger) }
    var accentGlow: Color { accent.opacity(0.18) }
    var accentDim: Color {
        let accent = Self.rgb(colors.accent), background = Self.rgb(colors.bg)
        return Color(.sRGB, red: accent.0 * 0.55 + background.0 * 0.45,
                     green: accent.1 * 0.55 + background.1 * 0.45,
                     blue: accent.2 * 0.55 + background.2 * 0.45, opacity: 1)
    }

    var smallRadius: CGFloat {
        switch family.radius {
        case .sharp: 2
        case .soft: 6
        case .round: 10
        }
    }

    var radius: CGFloat {
        switch family.radius {
        case .sharp: 6
        case .soft: 12
        case .round: 18
        }
    }

    /// Scale padding, never font sizes or the minimum 44-point touch target.
    var density: CGFloat { family.density == .compact ? 0.8 : 1 }

    func bodyFont(_ style: Font.TextStyle = .body) -> Font {
        DenFonts.font(family.fonts.body, style: style)
    }

    /// Reserve the display face for room titles and theme-name previews.
    func displayFont(_ style: Font.TextStyle = .title) -> Font {
        DenFonts.font(family.fonts.display, style: style)
    }

    func monoFont(_ style: Font.TextStyle = .caption) -> Font {
        DenFonts.font(family.fonts.mono, style: style, monospaced: true)
    }

    static func defaultTheme(scheme: ColorScheme = .dark) -> DenTheme {
        DenTheme(family: ThemeStore.loadBuiltins()[0], scheme: scheme)
    }

    static func color(_ hex: String) -> Color {
        let (red, green, blue) = rgb(hex)
        return Color(.sRGB, red: red, green: green, blue: blue, opacity: 1)
    }

    private func color(_ hex: String) -> Color { Self.color(hex) }

    private static func rgb(_ hex: String) -> (Double, Double, Double) {
        let value = UInt32(hex.dropFirst(), radix: 16) ?? 0
        return (Double((value >> 16) & 255) / 255,
                Double((value >> 8) & 255) / 255,
                Double(value & 255) / 255)
    }
}

@MainActor
enum DenFonts {
    private static var registered = false
    private static var postScriptNames: [String: String] = [:]

    /// Register only this app's bundled fonts, never systemwide fonts or downloaded account content.
    static func register() {
        guard !registered else { return }
        registered = true
        guard let resourceURL = Bundle.main.resourceURL else { return }
        if let files = FileManager.default.enumerator(at: resourceURL, includingPropertiesForKeys: nil) {
            for case let url as URL in files where ["ttf", "otf"].contains(url.pathExtension.lowercased()) {
                CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
            }
        }
        if let url = Bundle.main.url(forResource: "font-families", withExtension: "json"),
           let data = try? Data(contentsOf: url),
           let names = try? JSONDecoder().decode([String: String].self, from: data) {
            postScriptNames = names
        }
    }

    static func font(_ family: String, style: Font.TextStyle = .body, monospaced: Bool = false) -> Font {
        register()
        guard let face = regularFont(family, size: baseSize(style)) else {
            return .system(style, design: monospaced ? .monospaced : .default)
        }
        return .custom(face.fontName, size: baseSize(style), relativeTo: style)
    }

    /// UIKit text and attributed-string consumers use the same family and Dynamic Type curve.
    static func uiFont(_ family: String, style: UIFont.TextStyle = .body,
                       monospaced: Bool = false, compatibleWith traits: UITraitCollection? = nil) -> UIFont {
        register()
        let base = UIFont.preferredFont(forTextStyle: style,
                                        compatibleWith: UITraitCollection(preferredContentSizeCategory: .large))
        let face = regularFont(family, size: base.pointSize)
            ?? (monospaced ? UIFont.monospacedSystemFont(ofSize: base.pointSize, weight: .regular) : base)
        return UIFontMetrics(forTextStyle: style).scaledFont(for: face, compatibleWith: traits)
    }

    private static func regularFont(_ family: String, size: CGFloat) -> UIFont? {
        guard let name = postScriptNames[family], let bundled = UIFont(name: name, size: size) else { return nil }
        // A variable file's default instance can be ExtraLight or ExtraBold. Resolve
        // the family's regular instance instead of inheriting that file default.
        let descriptor = UIFontDescriptor(fontAttributes: [
            .family: bundled.familyName,
            .traits: [UIFontDescriptor.TraitKey.weight: UIFont.Weight.regular.rawValue],
        ])
        return UIFont(descriptor: descriptor, size: size)
    }

    private static func baseSize(_ style: Font.TextStyle) -> CGFloat {
        switch style {
        case .largeTitle: 34
        case .title: 28
        case .title2: 22
        case .title3: 20
        case .headline, .body: 17
        case .callout: 16
        case .subheadline: 15
        case .footnote: 13
        case .caption: 12
        case .caption2: 11
        @unknown default: 17
        }
    }
}
