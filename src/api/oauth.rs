//! OAuth authentication flow
//!
//! Handles Twitch OAuth login by opening browser and receiving callback.
#![allow(clippy::manual_pattern_char_comparison)]

use anyhow::{Result, anyhow};
use log::{error, info};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::twitch::{OAUTH_REDIRECT_PORT, TwitchClient};

/// Success page HTML shown after successful login
const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Secousse - Login Successful</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0e0e10;
            color: #efeff1;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
            padding: 40px;
        }
        .logo {
            width: 80px;
            height: 80px;
            background: #9146ff;
            border-radius: 16px;
            margin: 0 auto 24px;
        }
        h1 { color: #9146ff; margin-bottom: 16px; }
        p { color: #adadb8; margin-bottom: 24px; }
        .hint { font-size: 14px; color: #71717a; }
    </style>
    <script>
        // Send the token fragment to the server
        if (window.location.hash) {
            const params = new URLSearchParams(window.location.hash.substring(1));
            const token = params.get('access_token');
            if (token) {
                fetch('/callback?access_token=' + token)
                    .then(() => {
                        document.getElementById('status').textContent = 'Login successful! You can close this window.';
                    })
                    .catch(() => {
                        document.getElementById('status').textContent = 'Login successful! You can close this window.';
                    });
            }
        }
    </script>
</head>
<body>
    <div class="container">
        <div class="logo"></div>
        <h1>Secousse</h1>
        <p id="status">Processing login...</p>
        <p class="hint">You can close this window and return to the app.</p>
    </div>
</body>
</html>"#;

/// Error page HTML
const ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
    <title>Secousse - Login Failed</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #0e0e10;
            color: #efeff1;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
            margin: 0;
        }
        .container { text-align: center; padding: 40px; }
        h1 { color: #ff4444; margin-bottom: 16px; }
        p { color: #adadb8; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Login Failed</h1>
        <p>Something went wrong. Please try again.</p>
    </div>
</body>
</html>"#;

/// Start OAuth flow - opens browser and waits for callback
/// Returns the access token on success
pub fn start_oauth_flow() -> Result<String> {
    // Generate OAuth URL
    let oauth_url = TwitchClient::get_oauth_url();
    info!("Opening OAuth URL: {}", oauth_url);

    // Create channel for receiving token
    let (tx, rx) = mpsc::channel::<String>();

    // Start local server in background thread
    let server_handle = thread::spawn(move || run_oauth_server(tx));

    // Open browser
    if let Err(e) = open::that(&oauth_url) {
        error!("Failed to open browser: {}", e);
        return Err(anyhow!("Failed to open browser: {}", e));
    }

    // Wait for token with timeout (2 minutes)
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(token) => {
            info!("Received OAuth token");
            Ok(token)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            error!("OAuth timeout - no response received");
            Err(anyhow!("Login timed out. Please try again."))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // Check if server thread had an error
            if let Ok(Err(e)) = server_handle.join() {
                error!("OAuth server error: {}", e);
                return Err(e);
            }
            Err(anyhow!("Login cancelled"))
        }
    }
}

/// Run the local OAuth callback server
fn run_oauth_server(tx: mpsc::Sender<String>) -> Result<()> {
    let addr = format!("127.0.0.1:{}", OAUTH_REDIRECT_PORT);

    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind OAuth server to {}: {}", addr, e);
            return Err(anyhow!(
                "Failed to start login server. Port {} may be in use.",
                OAUTH_REDIRECT_PORT
            ));
        }
    };

    // Set a timeout so we don't block forever
    listener.set_nonblocking(false)?;

    info!("OAuth server listening on {}", addr);

    // Handle incoming connections
    let mut token_received = false;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let mut buffer = [0; 4096];
                if let Ok(n) = stream.read(&mut buffer) {
                    let request = String::from_utf8_lossy(&buffer[..n]);

                    // Parse the request
                    if let Some(token) = parse_oauth_callback(&request) {
                        info!("Extracted access token from callback");

                        // Send success response
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                            SUCCESS_HTML.len(),
                            SUCCESS_HTML
                        );
                        let _ = stream.write_all(response.as_bytes());

                        // Send token through channel
                        let _ = tx.send(token);
                        token_received = true;
                    } else if request.contains("GET / ") || request.contains("GET /callback") {
                        // Initial page load or callback without token yet
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                            SUCCESS_HTML.len(),
                            SUCCESS_HTML
                        );
                        let _ = stream.write_all(response.as_bytes());
                    } else {
                        // Unknown request
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                            ERROR_HTML.len(),
                            ERROR_HTML
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
                }

                if token_received {
                    // Give browser time to receive response
                    thread::sleep(Duration::from_millis(500));
                    break;
                }
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

/// Parse OAuth callback to extract access token
fn parse_oauth_callback(request: &str) -> Option<String> {
    // Token comes as query param: GET /callback?access_token=xxx
    if let Some(start) = request.find("access_token=") {
        let start = start + "access_token=".len();
        let end = request[start..]
            .find(|c: char| c == '&' || c == ' ' || c == '\r' || c == '\n')
            .map(|i| start + i)
            .unwrap_or(request.len());

        let token = &request[start..end];
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_oauth_callback() {
        let request = "GET /callback?access_token=abc123xyz&token_type=bearer HTTP/1.1\r\n";
        assert_eq!(parse_oauth_callback(request), Some("abc123xyz".to_string()));

        let request2 = "GET /callback?access_token=testtoken HTTP/1.1\r\n";
        assert_eq!(
            parse_oauth_callback(request2),
            Some("testtoken".to_string())
        );

        let request3 = "GET / HTTP/1.1\r\n";
        assert_eq!(parse_oauth_callback(request3), None);
    }
}
