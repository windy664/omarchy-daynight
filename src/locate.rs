//! Guess the user's coordinates from their public IP address.
//!
//! China only: the Meteorological Administration is reachable without a VPN
//! and answers with coordinates and the local timezone. Both services reject a
//! bare user agent, so one is set, and both occasionally time out on a
//! domestic mobile connection, so the calls are retried.

use serde::Deserialize;

const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64)";
const IP_URL: &str = "https://myip.ipip.net";
const CMA_AUTOCOMPLETE_URL: &str = "https://weather.cma.cn/api/autocomplete";
const CMA_DETAIL_URL: &str = "https://weather.cma.cn/api/weather/view";

#[derive(Debug, Clone)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub city: String,
}

#[derive(Deserialize)]
struct CmaAutocomplete {
    #[serde(default)]
    data: Vec<String>,
}

#[derive(Deserialize)]
struct CmaResponse {
    data: Option<CmaData>,
}

#[derive(Deserialize)]
struct CmaData {
    location: CmaLocation,
}

#[derive(Deserialize)]
struct CmaLocation {
    latitude: f64,
    longitude: f64,
}

fn get(url: &str) -> Result<String, String> {
    let mut last = String::from("no attempt made");
    for attempt in 0..3 {
        let result = ureq::get(url)
            .header("User-Agent", USER_AGENT)
            .call()
            .and_then(|mut response| response.body_mut().read_to_string());

        match result {
            Ok(text) => return Ok(text),
            Err(error) => {
                last = error.to_string();
                if attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    }
    Err(last)
}

fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let text = get(url)?;
    serde_json::from_str(&text).map_err(|error| format!("{url}: {error}"))
}

/// The city the public IP address belongs to.
///
/// The service answers in the form "当前 IP：1.2.3.4  来自于：中国 广东 东莞  移动",
/// which is country, province, city and operator. The city is the field
/// before the operator, but not every region lists all four.
fn city_from_ip_info(text: &str) -> Option<String> {
    let tail = text.split("来自于").nth(1)?;
    let tail = tail.replace(['：', ':'], " ");
    let fields: Vec<&str> = tail.split_whitespace().collect();
    if fields.is_empty() {
        return None;
    }

    let candidates: Vec<&str> = if fields.len() >= 4 {
        vec![fields[2], fields[1], fields[0]]
    } else if fields.len() == 3 {
        vec![fields[1], fields[0]]
    } else {
        vec![fields[0]]
    };

    candidates
        .into_iter()
        .find(|field| !field.is_empty() && *field != "中国")
        .map(|field| field.to_string())
}

/// The weather station whose name matches the city exactly, inside China.
fn cma_station(city: &str) -> Option<String> {
    let url = format!("{CMA_AUTOCOMPLETE_URL}?q={}", urlencode(city));
    let response: CmaAutocomplete = get_json(&url).ok()?;

    response
        .data
        .iter()
        .filter_map(|item| {
            let parts: Vec<&str> = item.split('|').collect();
            if parts.len() >= 4 && parts[1] == city && parts[3] == "中国" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .next()
}

fn cma_coordinates(station: &str) -> Option<(f64, f64)> {
    let url = format!("{CMA_DETAIL_URL}?stationid={station}");
    let response: CmaResponse = get_json(&url).ok()?;
    let location = response.data?.location;
    Some((location.latitude, location.longitude))
}

/// Look up the coordinates of the current public IP address.
pub fn locate() -> Result<Location, String> {
    let ip_info = get(IP_URL).map_err(|error| format!("could not read the IP address: {error}"))?;
    let city = city_from_ip_info(&ip_info)
        .ok_or_else(|| "could not determine your city from the IP address".to_string())?;

    let station =
        cma_station(&city).ok_or_else(|| format!("could not find a weather station for {city}"))?;
    let (latitude, longitude) = cma_coordinates(&station)
        .ok_or_else(|| format!("could not read the coordinates for {city}"))?;

    Ok(Location {
        latitude,
        longitude,
        city,
    })
}

/// Percent encode a value for a query string.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_city_out_of_the_ip_service() {
        let text = "当前 IP：223.74.166.53  来自于：中国 广东 东莞  移动";
        assert_eq!(city_from_ip_info(text).as_deref(), Some("东莞"));
    }

    #[test]
    fn falls_back_when_the_regional_fields_are_missing() {
        let text = "当前 IP：1.2.3.4  来自于：中国 北京  联通";
        assert_eq!(city_from_ip_info(text).as_deref(), Some("北京"));
    }

    #[test]
    fn percent_encodes_the_city() {
        assert_eq!(urlencode("东莞"), "%E4%B8%9C%E8%8E%9E");
        assert_eq!(urlencode("a b"), "a%20b");
    }
}
