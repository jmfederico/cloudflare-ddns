# Cloudflare DDNS Updater

A Rust-based dynamic DNS updater for Cloudflare that automatically updates your DNS records with your current public IP address.

## Features

- 🌐 Automatically detects your current public IP address
- 🔄 Updates Cloudflare DNS records via API
- 🛡️ Secure API token authentication
- ⚡ Fast and lightweight Rust implementation
- 🔧 Configurable via environment variables
- 📋 Support for different record types (A, AAAA, etc.)
- ✅ Only updates when IP has changed
- 💾 **Smart caching** - Avoids unnecessary API calls when IP hasn't changed
- ⏰ **Configurable cache expiry** - Force checks after specified time period
- 🚀 **Faster execution** - Cache hits skip Cloudflare API calls entirely
- 🐳 **Docker support** - Easy deployment with Docker and Docker Compose

## Prerequisites

- Docker installed on your system
- Docker Compose (optional, but recommended)
- Cloudflare account with API access
- Domain managed by Cloudflare

## Setup

### 1. Get Cloudflare Credentials

#### API Token

1. Go to [Cloudflare API Tokens](https://dash.cloudflare.com/profile/api-tokens)
2. Click "Create Token"
3. Use "Custom token" template
4. Set permissions:
   - Zone:Zone:Read
   - Zone:DNS:Edit
5. Set zone resources to include your domain
6. Copy the generated token

#### Zone ID

1. Go to your domain's overview page in Cloudflare dashboard
2. Find "Zone ID" in the right sidebar
3. Copy the Zone ID

### 2. Configure Environment Variables

Copy the example environment file:

```bash
cp .env.example .env
```

Edit `.env` with your values:

```bash
# Required
CLOUDFLARE_API_TOKEN=your_api_token_here
CLOUDFLARE_ZONE_ID=your_zone_id_here
DNS_RECORD_NAME=your.domain.com

# Optional
DNS_RECORD_TYPE=A
DNS_RECORD_TTL=1
CACHE_EXPIRY_HOURS=24
```

## Usage

### Quick Start with Docker Compose (Recommended)

1. **Configure your environment file** (see Setup section above)

2. **Run the application:**
   ```bash
   docker compose up --build
   ```

This will build the Docker image and run the application with your configured settings.

### Manual Docker Commands

#### Build the Docker Image

```bash
docker build -t cloudflare-ddns .
```

#### Run Once

```bash
docker run --rm \
  -e CLOUDFLARE_API_TOKEN="your_api_token" \
  -e CLOUDFLARE_ZONE_ID="your_zone_id" \
  -e DNS_RECORD_NAME="your.domain.com" \
  -e DNS_RECORD_TYPE="A" \
  -e DNS_RECORD_TTL="1" \
  cloudflare-ddns
```

#### Run with Environment File

```bash
docker run --rm --env-file .env cloudflare-ddns
```

#### Run as a Daemon (Continuous Updates)

To run the application continuously with periodic updates:

```bash
docker run -d --name cloudflare-ddns-daemon \
  --env-file .env \
  --restart unless-stopped \
  cloudflare-ddns \
  sh -c "while true; do /app/cloudflare-ddns; sleep 300; done"
```

This runs the updater every 5 minutes (300 seconds).

### Scheduling with Docker

For production use, you can run this as a scheduled job:

#### Using Docker with Host Cron

1. Create a script `/usr/local/bin/update-ddns.sh`:

   ```bash
   #!/bin/bash
   docker run --rm \
     --env-file /path/to/your/.env \
     cloudflare-ddns
   ```

2. Make it executable:

   ```bash
   chmod +x /usr/local/bin/update-ddns.sh
   ```

3. Add to crontab (runs every 5 minutes):
   ```bash
   crontab -e
   # Add this line:
   */5 * * * * /usr/local/bin/update-ddns.sh
   ```

#### Using Docker Compose with Periodic Updates

Uncomment the command section in `docker compose.yml` to run it as a continuous service with periodic updates.

## Environment Variables

| Variable               | Required | Default | Description                                            |
| ---------------------- | -------- | ------- | ------------------------------------------------------ |
| `CLOUDFLARE_API_TOKEN` | Yes      | -       | Cloudflare API token with Zone:DNS:Edit permissions    |
| `CLOUDFLARE_ZONE_ID`   | Yes      | -       | Zone ID of your domain in Cloudflare                   |
| `DNS_RECORD_NAME`      | Yes      | -       | DNS record name to update (e.g., `home.example.com`)   |
| `DNS_RECORD_TYPE`      | No       | `A`     | DNS record type (`A`, `AAAA`, etc.)                    |
| `DNS_RECORD_TTL`       | No       | `1`     | TTL in seconds for the DNS record (1 = automatic)      |
| `CACHE_EXPIRY_HOURS`   | No       | `24`    | Hours before cache expires and forces Cloudflare check |

## Example Output

### First Run (No Cache)

```
🌐 Getting current public IP address...
📍 Current IP: 203.0.113.42
📄 No cache file found, will create one after first run
🔗 Connecting to Cloudflare API...
📋 Fetching DNS records for 'home.example.com'...
🔍 Found DNS record: home.example.com -> 198.51.100.123
🔄 Updating DNS record from '198.51.100.123' to '203.0.113.42'...
✅ Successfully updated DNS record!
   Record: home.example.com
   Type: A
   New IP: 203.0.113.42
   TTL: 1
💾 Cache saved to ./cache.json
```

### Cache Hit (IP Unchanged)

```
🌐 Getting current public IP address...
📍 Current IP: 203.0.113.42
📄 Loaded cache from ./cache.json
✅ Cache hit! IP unchanged (203.0.113.42), skipping Cloudflare API call
   Last checked: 2025-01-06 12:15:30 UTC
```

### Cache Miss (IP Changed)

```
🌐 Getting current public IP address...
📍 Current IP: 203.0.113.99
📄 Loaded cache from ./cache.json
🔄 Cache hit but IP changed: 203.0.113.42 -> 203.0.113.99
🔍 Connecting to Cloudflare API...
📋 Fetching DNS records for 'home.example.com'...
🔍 Found DNS record: home.example.com -> 203.0.113.42
🔄 Updating DNS record from '203.0.113.42' to '203.0.113.99'...
✅ Successfully updated DNS record!
   Record: home.example.com
   Type: A
   New IP: 203.0.113.99
   TTL: 1
💾 Cache saved to ./cache.json
```

## Troubleshooting

### Check Container Logs

```bash
# For docker compose
docker compose logs cloudflare-ddns

# For manual docker run
docker logs cloudflare-ddns-daemon
```

### Debug Mode

To run the container interactively for debugging:

```bash
docker run -it --rm \
  --env-file .env \
  --entrypoint /bin/bash \
  cloudflare-ddns
```

For more detailed output, you can set the `RUST_LOG` environment variable:

```bash
docker run --rm \
  --env-file .env \
  -e RUST_LOG=debug \
  cloudflare-ddns
```

### Common Issues

1. **"CLOUDFLARE_API_TOKEN environment variable is required"**

   - Make sure your `.env` file is properly configured
   - Verify the environment variables are being passed to the container
   - Check that you're loading the `.env` file correctly

2. **"Cloudflare API error"**

   - Verify your API token has the correct permissions
   - Check that the Zone ID is correct
   - Ensure the DNS record exists in Cloudflare

3. **"No DNS record found"**

   - Verify the DNS record name exists in your Cloudflare zone
   - Check that the record type matches (default is 'A')
   - Ensure `DNS_RECORD_NAME` matches exactly with your Cloudflare DNS record

4. **API Permission Errors**
   - Ensure your API token has the correct permissions:
     - Zone:Zone:Read
     - Zone:DNS:Edit

## Security Notes

- Keep your API token secure and never commit it to version control
- Never commit your `.env` file with real credentials to version control
- Use Docker secrets or environment variable injection in production
- The API token should have minimal required permissions
- Consider rotating API tokens regularly
- The container runs as a non-root user for security
- Consider using read-only filesystem mounts in production

## Docker Image Details

The Dockerfile uses a multi-stage build to keep the final image small:

- Build stage: Uses full Rust toolchain (~1.5GB)
- Runtime stage: Uses minimal Debian slim (~80MB final image)

## License

This project is licensed under the MIT License - see the LICENSE file for details.
