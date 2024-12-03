# HTTP Proxy with Probability-Based Failure Simulation

A Rust-based HTTP proxy service that can simulate failures with configurable probability. This service is useful for testing application resilience and error handling by introducing controlled failures in your HTTP requests.

## Features

- Forward HTTP/HTTPS requests to configurable target URLs
- Simulate failures with configurable probability
- Support for both environment-based and header-based target URL configuration
- Detailed error reporting and logging
- Docker support for easy deployment
- JSON request/response handling

## Prerequisites

- Rust 1.75 or later (for local development)
- Docker (for containerized deployment)
- Cargo (Rust's package manager)

## Installation

### Local Development

1. Clone the repository:
   ```bash
   git clone git@github.com:yihaozhadan/http-proxy.git
   cd http-proxy
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Run the server:
   ```bash
   export TARGET_URL=https://httpbin.org/post
   export SUCCESS_PROBABILITY=0.8
   cargo run --release
   ```

### Docker Deployment

1. Build the Docker image:
   ```bash
   docker build -t http-proxy .
   ```

2. Run the container:
   ```bash
   docker run -p 3000:3000 \
     -e TARGET_URL=https://httpbin.org/post \
     -e SUCCESS_PROBABILITY=0.8 \
     http-proxy
   ```

## Configuration

The service can be configured using environment variables:

- `TARGET_URL`: The default target URL for proxying requests (required)
- `SUCCESS_PROBABILITY`: Default probability of successful request forwarding (default: 0.8)
  - Must be a float between 0.0 and 1.0
  - 0.0 means all requests fail
  - 1.0 means all requests succeed

## API Endpoints

### POST /delay

Forwards POST requests to the configured target URL with configurable delays. You can specify either a constant delay, a random delay range, or both.

**Headers:**
- `Content-Type: application/json` (required)
- `X-Proxy-Url`: Optional. Override the default target URL for testing
- `X-Constant-Delay-Ms`: Optional. Add a constant delay in milliseconds to every request
- `X-Max-Random-Delay-Ms`: Optional. Add a random delay between 0 and the specified milliseconds

**Example with constant delay:**
```bash
curl -X POST http://localhost:3000/delay \
  -H "Content-Type: application/json" \
  -H "X-Constant-Delay-Ms: 1000" \
  -d '{"test": "data"}'
```

**Example with random delay:**
```bash
curl -X POST http://localhost:3000/delay \
  -H "Content-Type: application/json" \
  -H "X-Max-Random-Delay-Ms: 2000" \
  -d '{"test": "data"}'
```

**Example with both delays and custom target:**
```bash
curl -X POST http://localhost:3000/delay \
  -H "Content-Type: application/json" \
  -H "X-Proxy-Url: https://api.example.com/endpoint" \
  -H "X-Constant-Delay-Ms: 500" \
  -H "X-Max-Random-Delay-Ms: 1000" \
  -d '{"test": "data"}'
```

**Success Response:**
```json
{
  "status": "success",
  "applied_delays": {
    "constant_delay_ms": 500,
    "random_delay_ms": "0-1000"
  },
  "target_url": "https://api.example.com/endpoint",
  "response": { "original": "response" }
}
```

### POST /failure

Forwards POST requests to the configured target URL with configurable failure simulation.

**Headers:**
- `Content-Type: application/json` (required)
- `X-Proxy-Url`: Optional. Override the default target URL for testing
- `X-Failure-Rate`: Optional. Override the default failure rate (value between 0.0 and 1.0)
  - If not provided, uses `1.0 - SUCCESS_PROBABILITY` from environment config
  - 0.0 means no failures
  - 1.0 means all requests fail

**Example with default configuration:**
```bash
curl -X POST http://localhost:3000/failure \
  -H "Content-Type: application/json" \
  -d '{"test": "data"}'
```

**Example with custom failure rate:**
```bash
curl -X POST http://localhost:3000/failure \
  -H "Content-Type: application/json" \
  -H "X-Failure-Rate: 0.3" \
  -d '{"test": "data"}'
```

**Example with custom target and failure rate:**
```bash
curl -X POST http://localhost:3000/failure \
  -H "Content-Type: application/json" \
  -H "X-Proxy-Url: https://api.example.com/endpoint" \
  -H "X-Failure-Rate: 0.5" \
  -d '{"test": "data"}'
```

## Error Responses

When a request fails (either due to probability or actual errors), the service returns a detailed error response:

```json
{
  "error": "Simulated failure",
  "target_url": "https://api.example.com/endpoint",
  "failure_rate": 0.3,
  "request_body": { "original": "request" }
}
```

## Response Format

### Success Response
```json
{
    "status": "success",
    "message": "..."
}
```

### Error Response
```json
{
    "error": "Error type",
    "details": "Detailed error message",
    "target_url": "Attempted target URL",
    "request_body": { ... }
}
```

## Error Types

1. **Simulated Failure**
   - Occurs based on SUCCESS_PROBABILITY
   - Returns 500 Internal Server Error

2. **Request Building Errors**
   - Invalid URL
   - Invalid request body
   - Returns 400 Bad Request

3. **Proxy Errors**
   - Connection failures
   - Timeout errors
   - Returns 500 Internal Server Error

## Development

### Running Tests
```bash
cargo test
```

### Local Development with Environment File
Create a `.env` file in the project root:
```env
TARGET_URL=https://httpbin.org/post
SUCCESS_PROBABILITY=0.8
```

Then run:
```bash
cargo run
```

## License

MIT License

Copyright (c) 2024 Hui Zhou

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.