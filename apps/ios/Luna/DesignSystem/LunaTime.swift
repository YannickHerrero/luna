import Foundation

enum LunaTime {
    static func conversationTimestamp(
        _ value: String,
        now: Date = .now,
        calendar: Calendar = .current,
        locale: Locale = .current
    ) -> String {
        guard let date = parse(value) else { return "" }
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        if calendar.isDate(date, inSameDayAs: now) {
            formatter.setLocalizedDateFormatFromTemplate("jm")
        } else {
            let days = calendar.dateComponents(
                [.day],
                from: calendar.startOfDay(for: date),
                to: calendar.startOfDay(for: now)
            ).day ?? Int.max
            if days > 0 && days < 7 {
                formatter.setLocalizedDateFormatFromTemplate("EEE")
            } else if calendar.component(.year, from: date) == calendar.component(.year, from: now) {
                formatter.setLocalizedDateFormatFromTemplate("MMM d")
            } else {
                formatter.setLocalizedDateFormatFromTemplate("MMM d y")
            }
        }
        return formatter.string(from: date)
            .replacingOccurrences(of: "\u{202f}", with: " ")
    }

    static func messageTimestamp(_ value: String, locale: Locale = .current) -> String {
        guard let date = parse(value) else { return "" }
        return date.formatted(
            Date.FormatStyle(date: .abbreviated, time: .shortened, locale: locale)
        )
    }

    static func parse(_ value: String) -> Date? {
        (try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(value))
            ?? (try? Date.ISO8601FormatStyle().parse(value))
    }
}
