import Foundation

/// Pattern-gated instant answers (weather, currency, crypto).
/// Each provider first matches the query shape *synchronously* and only then
/// hits the network, so adding providers never fans out wasted requests.
/// All sources are free and keyless.
enum InstantAnswerSources {
    /// Weather needs two sequential calls and Open-Meteo's geocoder can have a
    /// multi-second cold start, so it gets a longer per-request timeout than the
    /// default. Matched sources are rare, so a slow miss waiting this out is OK.
    nonisolated private static let weatherTimeout: TimeInterval = 6

    /// Fetch closures for every provider whose pattern matches `query`.
    nonisolated static func matches(for query: String) -> [@Sendable () async -> WebAnswer?] {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        var fetchers: [@Sendable () async -> WebAnswer?] = []

        if let c = currencyParse(q) { fetchers.append { await currency(c) } }
        if let place = weatherParse(q) { fetchers.append { await weather(place: place) } }
        if let id = cryptoParse(q) { fetchers.append { await crypto(id) } }
        return fetchers
    }

    nonisolated static func hasMatch(for query: String) -> Bool {
        !matches(for: query).isEmpty
    }

    // MARK: - Providers

    struct CurrencyQuery: Sendable { let amount: Double; let from: String; let to: String }

    nonisolated static func currency(_ q: CurrencyQuery) async -> WebAnswer? {
        guard let object = await WebJSON.object(WebJSON.url("https://api.frankfurter.dev/v1/latest", [
                  URLQueryItem(name: "amount", value: trimNumber(q.amount)),
                  URLQueryItem(name: "base", value: q.from),
                  URLQueryItem(name: "symbols", value: q.to),
              ])),
              let rates = object["rates"] as? [String: Any],
              let value = (rates[q.to] as? NSNumber)?.doubleValue
        else { return nil }
        let text = "\(formatNumber(q.amount)) \(q.from) = \(formatNumber(value)) \(q.to)"
        return WebAnswer(text: text, source: "Currency", url: nil, imageURL: nil)
    }

    nonisolated static func weather(place: String) async -> WebAnswer? {
        guard let geoObj = await WebJSON.object(WebJSON.url("https://geocoding-api.open-meteo.com/v1/search", [
                  URLQueryItem(name: "name", value: place),
                  URLQueryItem(name: "count", value: "1"),
              ]), timeout: weatherTimeout),
              let first = (geoObj["results"] as? [[String: Any]])?.first,
              let lat = first["latitude"] as? Double,
              let lon = first["longitude"] as? Double
        else { return nil }

        let name = (first["name"] as? String) ?? place
        let country = (first["country"] as? String) ?? ""

        guard let fcObj = await WebJSON.object(WebJSON.url("https://api.open-meteo.com/v1/forecast", [
                  URLQueryItem(name: "latitude", value: String(lat)),
                  URLQueryItem(name: "longitude", value: String(lon)),
                  URLQueryItem(
                      name: "current",
                      value: "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m"
                  ),
              ]), timeout: weatherTimeout),
              let current = fcObj["current"] as? [String: Any],
              let temp = current["temperature_2m"] as? Double
        else { return nil }

        let code = (current["weather_code"] as? Int) ?? -1
        let place = country.isEmpty ? name : "\(name), \(country)"
        var text = "\(place): \(Int(temp.rounded()))°C"
        if let feels = current["apparent_temperature"] as? Double, abs(feels - temp) >= 1 {
            text += " (feels \(Int(feels.rounded()))°C)"
        }
        text += ", \(wmoDescription(code))."
        if let humidity = current["relative_humidity_2m"] as? Int {
            text += " Humidity \(humidity)%."
        }
        if let wind = current["wind_speed_10m"] as? Double {
            text += " Wind \(Int(wind.rounded())) km/h."
        }
        return WebAnswer(text: text, source: "Weather", url: nil, imageURL: nil)
    }

    nonisolated static func crypto(_ id: String) async -> WebAnswer? {
        guard let object = await WebJSON.object(WebJSON.url("https://api.coingecko.com/api/v3/simple/price", [
                  URLQueryItem(name: "ids", value: id),
                  URLQueryItem(name: "vs_currencies", value: "usd"),
                  URLQueryItem(name: "include_24hr_change", value: "true"),
              ])),
              let coin = object[id] as? [String: Any],
              let usd = (coin["usd"] as? NSNumber)?.doubleValue
        else { return nil }

        var text = "\(id.capitalized): $\(formatNumber(usd))"
        if let change = (coin["usd_24h_change"] as? NSNumber)?.doubleValue {
            text += String(format: " (%@%.2f%% 24h)", change >= 0 ? "+" : "", change)
        }
        let page = URL(string: "https://www.coingecko.com/en/coins/\(id)")
        return WebAnswer(text: text, source: "Crypto", url: page, imageURL: nil)
    }

    // MARK: - Parsers

    nonisolated static func currencyParse(_ q: String) -> CurrencyQuery? {
        guard let g = q.captures(#"^([0-9]+(?:[.,][0-9]+)?)?\s*([a-zA-Z]{3})\s+(?:to|in|->|=)\s+([a-zA-Z]{3})$"#)
        else { return nil }
        let amount = Double(g[1].replacingOccurrences(of: ",", with: ".")) ?? 1
        return CurrencyQuery(amount: amount == 0 ? 1 : amount, from: g[2].uppercased(), to: g[3].uppercased())
    }

    nonisolated static func weatherParse(_ q: String) -> String? {
        guard let g = q.captures(#"^weather(?:\s+(?:in|at|for))?\s+(.+)$"#) else { return nil }
        let place = g[1].trimmingCharacters(in: .whitespaces)
        return place.isEmpty ? nil : place
    }

    nonisolated static func cryptoParse(_ q: String) -> String? {
        var name: String?
        if let g = q.captures(#"^(.+?)\s+price$"#) { name = g[1] }
        else if let g = q.captures(#"^price\s+of\s+(.+)$"#) { name = g[1] }
        guard var n = name?.lowercased().trimmingCharacters(in: .whitespaces), !n.isEmpty else { return nil }
        let aliases = [
            "btc": "bitcoin", "eth": "ethereum", "sol": "solana", "doge": "dogecoin",
            "ada": "cardano", "xrp": "ripple", "bnb": "binancecoin", "ltc": "litecoin",
        ]
        if let mapped = aliases[n] { n = mapped }
        return n.replacingOccurrences(of: " ", with: "-")
    }

    // MARK: - Helpers

    nonisolated private static func wmoDescription(_ code: Int) -> String {
        switch code {
        case 0: return "clear sky"
        case 1: return "mainly clear"
        case 2: return "partly cloudy"
        case 3: return "overcast"
        case 45, 48: return "fog"
        case 51, 53, 55: return "drizzle"
        case 56, 57: return "freezing drizzle"
        case 61, 63, 65: return "rain"
        case 66, 67: return "freezing rain"
        case 71, 73, 75: return "snow"
        case 77: return "snow grains"
        case 80, 81, 82: return "rain showers"
        case 85, 86: return "snow showers"
        case 95: return "thunderstorm"
        case 96, 99: return "thunderstorm with hail"
        default: return "—"
        }
    }

    nonisolated private static func trimNumber(_ value: Double) -> String {
        value == value.rounded() ? String(Int(value)) : String(value)
    }

    nonisolated private static func formatNumber(_ value: Double) -> String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        f.maximumFractionDigits = value < 1 ? 6 : 2
        f.minimumFractionDigits = 0
        return f.string(from: NSNumber(value: value)) ?? String(value)
    }
}

private extension String {
    /// Capture groups (index 0 = whole match) of the first regex match, or nil.
    nonisolated func captures(_ pattern: String) -> [String]? {
        guard let re = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else { return nil }
        let range = NSRange(startIndex..., in: self)
        guard let match = re.firstMatch(in: self, range: range) else { return nil }
        return (0..<match.numberOfRanges).map { idx in
            Range(match.range(at: idx), in: self).map { String(self[$0]) } ?? ""
        }
    }
}
