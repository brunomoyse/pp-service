//! Company verification via the EU VIES service.
//!
//! VIES (VAT Information Exchange System) is the free, official EU endpoint
//! that confirms a VAT number is real and returns the registered trader's
//! name + address. We use it as the anti-abuse gate for self-serve club
//! onboarding (BE/FR/LU/NL).
//!
//! Three outcomes are kept distinct on purpose:
//!   - `available && valid`  → real company (name/address returned)
//!   - `available && !valid` → not found / not VAT-registered (hard-block)
//!   - `!available`          → VIES unreachable/slow; caller falls back to
//!     format-only so an outage never blocks signups.
//!
//! Caveat: VIES only knows VAT-*registered* entities. Some non-profits hold a
//! BCE enterprise number but aren't VAT-registered and will come back invalid.

use std::time::Duration;

use serde::Deserialize;

/// Result of a VIES lookup. Never errors on transport problems; those map to
/// `available: false` so the caller can degrade gracefully.
#[derive(Debug, Clone, Default)]
pub struct CompanyLookupResult {
    /// Whether the VIES service actually answered (false on timeout/error).
    pub available: bool,
    /// Whether the VAT number is a valid, registered company.
    pub valid: bool,
    /// Registered (legal) company name, when VIES returns one.
    pub name: Option<String>,
    /// Whether the registered name looks like a non-profit (ASBL/VZW/etc).
    /// VIES doesn't expose legal form, so this is a name heuristic.
    pub non_profit: bool,
    /// Street line of the registered address (the address arrives newline-split
    /// as `"street\npostal city"`, so we split it into the form's fields).
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViesResponse {
    // The REST API has used both `valid` and `isValid` across versions.
    #[serde(default, alias = "isValid")]
    valid: bool,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    address: Option<String>,
}

const VIES_ENDPOINT: &str = "https://ec.europa.eu/taxation_customs/vies/rest-api/check-vat-number";

/// Normalize a user-entered VAT number into the bare number VIES expects:
/// uppercased, non-alphanumerics stripped, and the leading country code
/// removed if the user typed it (e.g. "BE 0123.456.789" → "0123456789").
fn normalize_number(country: &str, vat_number: &str) -> String {
    let cleaned: String = vat_number
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    cleaned
        .strip_prefix(&country.to_uppercase())
        .unwrap_or(&cleaned)
        .to_string()
}

/// Look up a company by country code (BE/FR/LU/NL) + VAT number.
///
/// Always returns a `CompanyLookupResult`; transport failures surface as
/// `available: false` rather than an `Err`, so callers treat "couldn't verify"
/// separately from "not found".
pub async fn lookup(country: &str, vat_number: &str) -> CompanyLookupResult {
    let country = country.trim().to_uppercase();
    let number = normalize_number(&country, vat_number);

    if country.is_empty() || number.is_empty() {
        return CompanyLookupResult::default();
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(_) => return CompanyLookupResult::default(),
    };

    let body = serde_json::json!({ "countryCode": country, "vatNumber": number });

    let response = match client.post(VIES_ENDPOINT).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("VIES request failed: {e}");
            return CompanyLookupResult::default();
        }
    };

    if !response.status().is_success() {
        tracing::warn!("VIES returned HTTP {}", response.status());
        return CompanyLookupResult::default();
    }

    let parsed = match response.json::<ViesResponse>().await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("VIES response parse failed: {e}");
            return CompanyLookupResult::default();
        }
    };

    let (street, postal_code, city) = parse_address(parsed.address.as_deref(), &country);
    let name = clean_field(parsed.name);
    let non_profit = name.as_deref().map(is_non_profit).unwrap_or(false);

    CompanyLookupResult {
        available: true,
        valid: parsed.valid,
        name,
        non_profit,
        street,
        postal_code,
        city,
    }
}

/// Heuristic for whether a registered name is a non-profit. VIES doesn't return
/// the legal form, so we scan the name for the usual markers across BE/FR/LU/NL
/// (BE/LU: ASBL·VZW·AISBL·IVZW; FR: association; NL: vereniging·stichting).
/// Dots and case are ignored, so "A.S.B.L." matches "ASBL".
fn is_non_profit(name: &str) -> bool {
    const MARKERS: [&str; 9] = [
        "ASBL",
        "VZW",
        "AISBL",
        "IVZW",
        "ASSOCIATION",
        "VERENIGING",
        "STICHTING",
        "SANS BUT LUCRATIF",
        "ZONDER WINSTOOGMERK",
    ];
    let normalized: String = name
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect();
    MARKERS.iter().any(|m| normalized.contains(m))
}

/// VIES uses "---" / empty strings for missing trader data; turn those into None
/// and collapse internal whitespace to single spaces.
fn clean_field(value: Option<String>) -> Option<String> {
    clean_str(&value?)
}

fn clean_str(v: &str) -> Option<String> {
    let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
    if v.is_empty() || v == "---" {
        None
    } else {
        Some(v)
    }
}

/// Split a VIES address into (street, postal_code, city). VIES returns the
/// address newline-delimited, last line being `"<postal> <city>"`, e.g.
/// `"Rue des Tilleuls 13\n4920 Aywaille"` → ("Rue des Tilleuls 13", "4920",
/// "Aywaille"). Falls back gracefully when the shape is unexpected.
fn parse_address(
    raw: Option<&str>,
    country: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let raw = match raw {
        Some(r) => r,
        None => return (None, None, None),
    };
    let lines: Vec<String> = raw.split(['\n', '\r']).filter_map(clean_str).collect();

    match lines.len() {
        0 => (None, None, None),
        // No postal/city line to split off, so treat the whole thing as street.
        1 => (Some(lines[0].clone()), None, None),
        _ => {
            let street = lines[..lines.len() - 1].join(", ");
            let (postal, city) = parse_postal_city(&lines[lines.len() - 1], country);
            (Some(street), postal, city)
        }
    }
}

/// Split a `"<postal> <city>"` line. The postal code is the leading token(s)
/// containing a digit; NL also pulls in a trailing 2-letter block ("1234 AB").
fn parse_postal_city(line: &str, country: &str) -> (Option<String>, Option<String>) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return (None, None);
    }

    let mut idx = 0;
    let mut postal = String::new();
    if tokens[0].chars().any(|c| c.is_ascii_digit()) {
        postal = tokens[0].to_string();
        idx = 1;
        // NL postal codes are "1234 AB", so consume the 2-letter suffix.
        if country == "NL"
            && tokens.len() > 1
            && tokens[1].len() == 2
            && tokens[1].chars().all(|c| c.is_ascii_alphabetic())
        {
            postal = format!("{} {}", postal, tokens[1]);
            idx = 2;
        }
    }

    let city = tokens[idx..].join(" ");
    (
        clean_str(&postal),
        if city.is_empty() { None } else { Some(city) },
    )
}
