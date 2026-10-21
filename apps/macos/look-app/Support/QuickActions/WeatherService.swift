import Foundation
import OSLog

/// A resolved weather reading ready for display in the launchpad tile.
struct WeatherSnapshot: Equatable {
    /// Formatted temperature including the degree sign, e.g. "18°".
    let temperature: String
    /// SF Symbol name for the current condition.
    let symbolName: String
    /// Short human-readable condition, e.g. "Partly Cloudy".
    let condition: String
    /// Today's formatted high and low temperatures, e.g. "24°" / "13°".
    let high: String
    let low: String
    /// Today's chance of precipitation as a formatted percent (e.g. "60%"), or
    /// nil when the source didn't report it.
    let rainChance: String?
    /// City name from the IP lookup, or nil when it wasn't reported. Not shown in
    /// the tile (location is kept private); retained for potential future use.
    let city: String?
}

/// Fetches current weather for the user's approximate location and caches it.
///
/// Location comes from a keyless IP lookup (no permission prompt, city-level),
/// weather from Open-Meteo (keyless, free). Both responses are cached in
/// `UserDefaults`, so reopening the launcher shows the last reading instantly and
/// the network is hit at most once per refresh interval. This is the native
/// per-shell state read the shared catalog declares for the Weather tile; other
/// platforms resolve it their own way.
final class WeatherService {
    static let shared = WeatherService()

    private enum Endpoint {
        /// Keyless HTTPS IP geolocation (ipwho.is); returns numeric
        /// `latitude`/`longitude` and a `success` flag. Free tier is generous
        /// enough for a per-user tile given the day-long location cache.
        static let ipGeolocation = "https://ipwho.is/"
        /// Open-Meteo current-conditions, parameterized by coordinates and unit.
        static let openMeteo = "https://api.open-meteo.com/v1/forecast"
    }

    /// Skip the network when the cached reading is younger than this.
    private static let weatherMaxAge: TimeInterval = 30 * 60  // 30 minutes
    /// Location changes rarely; reuse the cached coordinates for this long.
    private static let locationMaxAge: TimeInterval = 24 * 60 * 60  // 1 day
    /// Cap each request so a hung network never stalls the refresh.
    private static let requestTimeout: TimeInterval = 8

    private static let cachedWeatherKey = "look.weather.cachedReading"
    private static let cachedLocationKey = "look.weather.cachedLocation"

    private let session: URLSession
    private let defaults: UserDefaults
    private let log = Logger(subsystem: "noah-code.Look", category: "WeatherService")

    init(session: URLSession = .shared, defaults: UserDefaults = .standard) {
        self.session = session
        self.defaults = defaults
    }

    /// Returns the current weather, using the cache when it is still fresh and
    /// otherwise fetching from the network. Returns the last cached reading (or
    /// nil) when the network is unavailable, so the tile degrades gracefully.
    func currentWeather() async -> WeatherSnapshot? {
        if let cached = usableCachedWeather(), age(of: cached.fetchedAt) < Self.weatherMaxAge {
            return snapshot(from: cached)
        }

        do {
            let coordinates = try await coordinates()
            let reading = try await fetchWeather(at: coordinates)
            store(reading, forKey: Self.cachedWeatherKey)
            return snapshot(from: reading)
        } catch {
            log.error("Weather refresh failed: \(error.localizedDescription, privacy: .public)")
            return usableCachedWeather().map(snapshot(from:))
        }
    }

    // MARK: - Location

    private func coordinates() async throws -> Coordinates {
        if let cached = cached(Coordinates.self, forKey: Self.cachedLocationKey),
           age(of: cached.fetchedAt) < Self.locationMaxAge {
            return cached
        }
        let fresh = try await fetchCoordinates()
        store(fresh, forKey: Self.cachedLocationKey)
        return fresh
    }

    private func fetchCoordinates() async throws -> Coordinates {
        guard let url = URL(string: Endpoint.ipGeolocation) else { throw WeatherError.badURL }
        let response: IPGeoResponse = try await get(url)
        guard response.success, let latitude = response.latitude, let longitude = response.longitude else {
            throw WeatherError.badResponse
        }
        return Coordinates(
            latitude: latitude,
            longitude: longitude,
            city: response.city,
            fetchedAt: Date().timeIntervalSince1970
        )
    }

    // MARK: - Weather

    private func fetchWeather(at coordinates: Coordinates) async throws -> CachedWeather {
        let fahrenheit = usesFahrenheit
        var components = URLComponents(string: Endpoint.openMeteo)
        components?.queryItems = [
            URLQueryItem(name: "latitude", value: String(coordinates.latitude)),
            URLQueryItem(name: "longitude", value: String(coordinates.longitude)),
            URLQueryItem(name: "current", value: "temperature_2m,weather_code"),
            URLQueryItem(name: "daily", value: "temperature_2m_max,temperature_2m_min,precipitation_probability_max"),
            URLQueryItem(name: "temperature_unit", value: fahrenheit ? "fahrenheit" : "celsius"),
            URLQueryItem(name: "timezone", value: "auto"),
            URLQueryItem(name: "forecast_days", value: "1"),
        ]
        guard let url = components?.url else { throw WeatherError.badURL }
        let response: OpenMeteoResponse = try await get(url)
        guard let high = response.daily.high.first, let low = response.daily.low.first else {
            throw WeatherError.badResponse
        }
        return CachedWeather(
            temperature: response.current.temperature,
            weatherCode: response.current.weatherCode,
            high: high,
            low: low,
            rainChance: response.daily.rainChance.first.flatMap { $0 },
            isFahrenheit: fahrenheit,
            city: coordinates.city,
            fetchedAt: Date().timeIntervalSince1970
        )
    }

    /// Fahrenheit for US-style locales, Celsius everywhere else.
    private var usesFahrenheit: Bool {
        Locale.current.measurementSystem == .us
    }

    // MARK: - Networking

    private func get<T: Decodable>(_ url: URL) async throws -> T {
        var request = URLRequest(url: url)
        request.timeoutInterval = Self.requestTimeout
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            throw WeatherError.badResponse
        }
        return try JSONDecoder().decode(T.self, from: data)
    }

    // MARK: - Presentation

    private func snapshot(from reading: CachedWeather) -> WeatherSnapshot {
        let condition = Self.condition(for: reading.weatherCode)
        return WeatherSnapshot(
            temperature: Self.formatTemperature(reading.temperature),
            symbolName: condition.symbol,
            condition: condition.label,
            high: Self.formatTemperature(reading.high),
            low: Self.formatTemperature(reading.low),
            rainChance: reading.rainChance.map { "\($0)%" },
            city: reading.city
        )
    }

    private static func formatTemperature(_ value: Double) -> String {
        "\(Int(value.rounded()))°"
    }

    /// Maps a WMO weather-interpretation code to an SF Symbol and short label.
    /// Codes are grouped per the Open-Meteo documentation.
    private static func condition(for code: Int) -> (symbol: String, label: String) {
        switch code {
        case 0: return ("sun.max.fill", "Clear")
        case 1, 2: return ("cloud.sun.fill", "Partly Cloudy")
        case 3: return ("cloud.fill", "Cloudy")
        case 45, 48: return ("cloud.fog.fill", "Fog")
        case 51, 53, 55, 56, 57: return ("cloud.drizzle.fill", "Drizzle")
        case 61, 63, 65, 66, 67: return ("cloud.rain.fill", "Rain")
        case 71, 73, 75, 77: return ("cloud.snow.fill", "Snow")
        case 80, 81, 82: return ("cloud.heavyrain.fill", "Showers")
        case 85, 86: return ("cloud.snow.fill", "Snow Showers")
        case 95, 96, 99: return ("cloud.bolt.rain.fill", "Thunderstorm")
        default: return ("cloud.fill", "Cloudy")
        }
    }

    // MARK: - Cache

    private func cachedWeather() -> CachedWeather? {
        cached(CachedWeather.self, forKey: Self.cachedWeatherKey)
    }

    /// The cached reading, but only when it was captured in the unit the current
    /// locale wants. After a region change the old value is in the wrong unit, so
    /// it is discarded to force a refetch rather than shown as a bare degree.
    private func usableCachedWeather() -> CachedWeather? {
        guard let cached = cachedWeather(), cached.isFahrenheit == usesFahrenheit else {
            return nil
        }
        return cached
    }

    private func cached<T: Decodable>(_ type: T.Type, forKey key: String) -> T? {
        guard let data = defaults.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    private func store<T: Encodable>(_ value: T, forKey key: String) {
        guard let data = try? JSONEncoder().encode(value) else { return }
        defaults.set(data, forKey: key)
    }

    private func age(of epoch: TimeInterval) -> TimeInterval {
        Date().timeIntervalSince1970 - epoch
    }
}

// MARK: - Wire models

private enum WeatherError: Error {
    case badURL
    case badResponse
}

private struct Coordinates: Codable {
    let latitude: Double
    let longitude: Double
    let city: String?
    let fetchedAt: TimeInterval
}

private struct CachedWeather: Codable {
    let temperature: Double
    let weatherCode: Int
    let high: Double
    let low: Double
    let rainChance: Int?
    let isFahrenheit: Bool
    let city: String?
    let fetchedAt: TimeInterval
}

private struct IPGeoResponse: Decodable {
    /// ipwho.is reports `success: false` with a message on failure (e.g. an
    /// unroutable IP), and omits coordinates, so both are optional and gated.
    let success: Bool
    let latitude: Double?
    let longitude: Double?
    let city: String?
}

private struct OpenMeteoResponse: Decodable {
    struct Current: Decodable {
        let temperature: Double
        let weatherCode: Int

        private enum CodingKeys: String, CodingKey {
            case temperature = "temperature_2m"
            case weatherCode = "weather_code"
        }
    }

    struct Daily: Decodable {
        /// Single-element arrays (the request asks for one forecast day). Rain
        /// probability can be null for a slot, hence the optional element.
        let high: [Double]
        let low: [Double]
        let rainChance: [Int?]

        private enum CodingKeys: String, CodingKey {
            case high = "temperature_2m_max"
            case low = "temperature_2m_min"
            case rainChance = "precipitation_probability_max"
        }
    }

    let current: Current
    let daily: Daily
}
