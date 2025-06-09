use anyhow::{Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct IpResponse {
    ip: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    errors: Vec<CloudflareError>,
    messages: Vec<String>,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: u32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DnsRecord {
    id: String,
    name: String,
    content: String,
    #[serde(rename = "type")]
    record_type: String,
    ttl: u32,
}

#[derive(Debug, Serialize)]
struct UpdateDnsRecord {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct DnsCache {
    record_name: String,
    record_type: String,
    ip_address: String,
    last_checked: DateTime<Utc>,
    last_updated: DateTime<Utc>,
}

impl DnsCache {
    fn new(record_name: String, record_type: String, ip_address: String) -> Self {
        let now = Utc::now();
        Self {
            record_name,
            record_type,
            ip_address,
            last_checked: now,
            last_updated: now,
        }
    }

    fn is_expired(&self, expiry_hours: i64) -> bool {
        let expiry_duration = Duration::hours(expiry_hours);
        Utc::now() - self.last_checked > expiry_duration
    }

    fn matches_config(&self, record_name: &str, record_type: &str) -> bool {
        self.record_name == record_name && self.record_type == record_type
    }

    fn update_ip(&mut self, new_ip: String) {
        self.ip_address = new_ip;
        self.last_updated = Utc::now();
        self.last_checked = Utc::now();
    }

    fn update_checked(&mut self) {
        self.last_checked = Utc::now();
    }
}

struct CloudflareClient {
    client: Client,
    api_token: String,
    zone_id: String,
}

impl CloudflareClient {
    fn new(api_token: String, zone_id: String) -> Self {
        let client = Client::new();
        Self {
            client,
            api_token,
            zone_id,
        }
    }

    async fn get_dns_records(&self, record_name: &str) -> Result<Vec<DnsRecord>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records?name={}",
            self.zone_id, record_name
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .await?;

        let cf_response: CloudflareResponse<Vec<DnsRecord>> = response.json().await?;

        if !cf_response.success {
            let error_details = cf_response
                .errors
                .iter()
                .map(|e| format!("Code {}: {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!("Cloudflare API error: {}", error_details));
        }

        // Log any messages from Cloudflare
        if !cf_response.messages.is_empty() {
            println!("📝 Cloudflare messages: {:?}", cf_response.messages);
        }

        cf_response
            .result
            .ok_or_else(|| anyhow!("No result in response"))
    }

    async fn update_dns_record(&self, record_id: &str, update_data: UpdateDnsRecord) -> Result<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record_id
        );

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&update_data)
            .send()
            .await?;

        let cf_response: CloudflareResponse<DnsRecord> = response.json().await?;

        if !cf_response.success {
            let error_details = cf_response
                .errors
                .iter()
                .map(|e| format!("Code {}: {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!("Failed to update DNS record: {}", error_details));
        }

        // Log any messages from Cloudflare
        if !cf_response.messages.is_empty() {
            println!("📝 Cloudflare messages: {:?}", cf_response.messages);
        }

        Ok(())
    }
}

async fn get_public_ip() -> Result<String> {
    let client = Client::new();

    // Try multiple IP services for reliability
    let ip_services = [
        "https://api.ipify.org?format=json",
        "https://httpbin.org/ip",
        "https://api.myip.com",
    ];

    for service in &ip_services {
        match client.get(*service).send().await {
            Ok(response) => {
                if let Ok(ip_response) = response.json::<IpResponse>().await {
                    return Ok(ip_response.ip);
                }
            }
            Err(_) => continue,
        }
    }

    // Fallback to a simple text-based service
    let response = client.get("https://ipinfo.io/ip").send().await?;
    let ip = response.text().await?.trim().to_string();

    Ok(ip)
}

fn load_cache() -> Option<DnsCache> {
    let cache_path = "./cache.json";

    if !Path::new(cache_path).exists() {
        println!("📄 No cache file found, will create one after first run");
        return None;
    }

    match fs::read_to_string(cache_path) {
        Ok(content) => match serde_json::from_str::<DnsCache>(&content) {
            Ok(cache) => {
                println!("📄 Loaded cache from {}", cache_path);
                Some(cache)
            }
            Err(e) => {
                println!("⚠️  Cache file corrupted ({}), will recreate", e);
                None
            }
        },
        Err(e) => {
            println!("⚠️  Failed to read cache file ({}), will recreate", e);
            None
        }
    }
}

fn save_cache(cache: &DnsCache) -> Result<()> {
    let cache_path = "./cache.json";
    let content = serde_json::to_string_pretty(cache)?;

    fs::write(cache_path, content)?;
    println!("💾 Cache saved to {}", cache_path);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Read environment variables
    let api_token = env::var("CLOUDFLARE_API_TOKEN")
        .map_err(|_| anyhow!("CLOUDFLARE_API_TOKEN environment variable is required"))?;

    let zone_id = env::var("CLOUDFLARE_ZONE_ID")
        .map_err(|_| anyhow!("CLOUDFLARE_ZONE_ID environment variable is required"))?;

    let record_name = env::var("DNS_RECORD_NAME")
        .map_err(|_| anyhow!("DNS_RECORD_NAME environment variable is required"))?;

    let record_type = env::var("DNS_RECORD_TYPE").unwrap_or_else(|_| "A".to_string());
    let ttl: u32 = env::var("DNS_RECORD_TTL")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .unwrap_or(1);

    let cache_expiry_hours: i64 = env::var("CACHE_EXPIRY_HOURS")
        .unwrap_or_else(|_| "24".to_string())
        .parse()
        .unwrap_or(24);

    println!("🌐 Getting current public IP address...");
    let current_ip = get_public_ip().await?;
    println!("📍 Current IP: {}", current_ip);

    // Load cache and check if we can skip Cloudflare API call
    let mut cache = load_cache();

    if let Some(ref cached_data) = cache {
        if cached_data.matches_config(&record_name, &record_type) {
            if !cached_data.is_expired(cache_expiry_hours) {
                if cached_data.ip_address == current_ip {
                    println!(
                        "✅ Cache hit! IP unchanged ({}), skipping Cloudflare API call",
                        current_ip
                    );
                    println!(
                        "   Last checked: {}",
                        cached_data.last_checked.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    return Ok(());
                } else {
                    println!(
                        "🔄 Cache hit but IP changed: {} -> {}",
                        cached_data.ip_address, current_ip
                    );
                }
            } else {
                println!(
                    "⏰ Cache expired ({}h), checking Cloudflare",
                    cache_expiry_hours
                );
            }
        } else {
            println!("⚠️  Cache config mismatch, checking Cloudflare");
        }
    }

    // Need to check Cloudflare API
    println!("🔍 Connecting to Cloudflare API...");
    let cf_client = CloudflareClient::new(api_token, zone_id);

    println!("📋 Fetching DNS records for '{}'...", record_name);
    let records = cf_client.get_dns_records(&record_name).await?;

    if records.is_empty() {
        return Err(anyhow!("No DNS record found with name '{}'", record_name));
    }

    // Find the record with the matching type (default to A record)
    let target_record = records
        .iter()
        .find(|r| r.record_type == record_type)
        .ok_or_else(|| {
            anyhow!(
                "No {} record found with name '{}'",
                record_type,
                record_name
            )
        })?;

    println!(
        "🔍 Found DNS record: {} -> {} (TTL: {})",
        target_record.name, target_record.content, target_record.ttl
    );

    // Update or create cache with current Cloudflare record
    match cache.as_mut() {
        Some(cached_data) if cached_data.matches_config(&record_name, &record_type) => {
            cached_data.update_checked();
        }
        _ => {
            cache = Some(DnsCache::new(
                record_name.clone(),
                record_type.clone(),
                target_record.content.clone(),
            ));
        }
    }

    // Check if update is needed
    if target_record.content == current_ip {
        println!("✅ DNS record is already up to date!");

        // Update cache with current IP if it was different
        if let Some(ref mut cached_data) = cache {
            if cached_data.ip_address != current_ip {
                cached_data.update_ip(current_ip);
            }
        }

        // Save cache
        if let Some(ref cached_data) = cache {
            if let Err(e) = save_cache(cached_data) {
                println!("⚠️  Failed to save cache: {}", e);
            }
        }

        return Ok(());
    }

    println!(
        "🔄 Updating DNS record from '{}' to '{}'...",
        target_record.content, current_ip
    );

    let update_data = UpdateDnsRecord {
        record_type: record_type.clone(),
        name: record_name.clone(),
        content: current_ip.clone(),
        ttl,
    };

    cf_client
        .update_dns_record(&target_record.id, update_data)
        .await?;

    println!("✅ Successfully updated DNS record!");
    println!("   Record: {}", record_name);
    println!("   Type: {}", record_type);
    println!("   New IP: {}", current_ip);
    println!("   TTL: {}", ttl);

    // Update cache with new IP
    if let Some(ref mut cached_data) = cache {
        cached_data.update_ip(current_ip);
    } else {
        cache = Some(DnsCache::new(record_name, record_type, current_ip));
    }

    // Save cache
    if let Some(ref cached_data) = cache {
        if let Err(e) = save_cache(cached_data) {
            println!("⚠️  Failed to save cache: {}", e);
        }
    }

    Ok(())
}
