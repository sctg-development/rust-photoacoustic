// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Redis Data Viewer
//!
//! A simple CLI tool to view and monitor data stored in Redis by the photoacoustic system.
//! Supports both key-value inspection and pub/sub monitoring.

use anyhow::Result;
use clap::{Arg, Command};
use futures::StreamExt;
use redis::{AsyncCommands, Client};
use serde_json::Value;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let matches = Command::new("redis-viewer")
        .version("1.0")
        .about("View and monitor Redis data from photoacoustic system")
        .arg(
            Arg::new("connection")
                .long("connection-string")
                .short('c')
                .value_name("URL")
                .help("Redis connection string")
                .default_value("redis://localhost:6379")
                .required(false),
        )
        .arg(
            Arg::new("mode")
                .long("mode")
                .short('m')
                .value_name("MODE")
                .help("Operation mode: keys or subscribe")
                .value_parser(["keys", "subscribe"])
                .default_value("keys")
                .required(false),
        )
        .arg(
            Arg::new("pattern")
                .long("pattern")
                .short('p')
                .value_name("PATTERN")
                .help("Key pattern to search (for keys mode) or channel to subscribe (for subscribe mode)")
                .default_value("photoacoustic*")
                .required(false),
        )
        .arg(
            Arg::new("channel")
                .long("channel")
                .value_name("CHANNEL")
                .help("Redis channel to subscribe to (for subscribe mode)")
                .required(false),
        )
        .arg(
            Arg::new("limit")
                .long("limit")
                .short('l')
                .value_name("NUMBER")
                .help("Limit number of results (for keys mode)")
                .default_value("100")
                .value_parser(clap::value_parser!(usize))
                .required(false),
        )
        .arg(
            Arg::new("watch")
                .long("watch")
                .short('w')
                .help("Watch for real-time updates (for keys mode)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .short('j')
                .help("Pretty print JSON values")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let connection_string = matches.get_one::<String>("connection").unwrap();
    let mode = matches.get_one::<String>("mode").unwrap();
    let pattern = matches.get_one::<String>("pattern").unwrap();
    let limit = *matches.get_one::<usize>("limit").unwrap();
    let watch = matches.get_flag("watch");
    let pretty_json = matches.get_flag("json");

    println!("🔗 Connecting to Redis at: {}", connection_string);

    let client = Client::open(connection_string.clone())?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    // Test connection
    let _: String = redis::cmd("PING").query_async(&mut conn).await?;
    println!("✅ Connected to Redis successfully\n");

    match mode.as_str() {
        "keys" => {
            if watch {
                watch_keys(&mut conn, pattern, limit, pretty_json).await?;
            } else {
                list_keys(&mut conn, pattern, limit, pretty_json).await?;
            }
        }
        "subscribe" => {
            let channel = matches.get_one::<String>("channel").unwrap_or(pattern);
            subscribe_channel(&client, channel).await?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

async fn list_keys(
    conn: &mut redis::aio::MultiplexedConnection,
    pattern: &str,
    limit: usize,
    pretty_json: bool,
) -> Result<()> {
    println!("🔍 Searching for keys matching pattern: {}", pattern);

    let keys: Vec<String> = conn.keys(pattern).await?;
    let total_keys = keys.len();
    let keys_to_show = keys.into_iter().take(limit).collect::<Vec<_>>();

    println!(
        "📊 Found {} keys (showing first {})\n",
        total_keys,
        keys_to_show.len()
    );

    for (i, key) in keys_to_show.iter().enumerate() {
        println!("🔑 Key {}: {}", i + 1, key);

        // Get key type
        let key_type: String = redis::cmd("TYPE").arg(key).query_async(conn).await?;
        println!("   Type: {}", key_type);

        // Get TTL
        let ttl: i64 = conn.ttl(key).await?;
        if ttl > 0 {
            println!("   TTL: {} seconds", ttl);
        } else if ttl == -1 {
            println!("   TTL: No expiration");
        } else {
            println!("   TTL: Key does not exist or expired");
        }

        match key_type.as_str() {
            "string" => {
                let value: String = conn.get(key).await?;
                if pretty_json && (value.starts_with('{') || value.starts_with('[')) {
                    match serde_json::from_str::<Value>(&value) {
                        Ok(json_value) => {
                            println!("   Value (JSON):");
                            println!("{}", serde_json::to_string_pretty(&json_value)?);
                        }
                        Err(_) => {
                            println!("   Value: {}", value);
                        }
                    }
                } else {
                    println!("   Value: {}", value);
                }
            }
            "list" => {
                let list_len: usize = conn.llen(key).await?;
                println!("   List length: {}", list_len);
                let items: Vec<String> = conn.lrange(key, 0, 4).await?; // Show first 5 items
                for (j, item) in items.iter().enumerate() {
                    println!("   [{}]: {}", j, item);
                }
                if list_len > 5 {
                    println!("   ... and {} more items", list_len - 5);
                }
            }
            "set" => {
                let set_size: usize = conn.scard(key).await?;
                println!("   Set size: {}", set_size);
                let members: Vec<String> = conn.smembers(key).await?;
                for (j, member) in members.iter().take(5).enumerate() {
                    println!("   {{{}}}: {}", j, member);
                }
                if set_size > 5 {
                    println!("   ... and {} more members", set_size - 5);
                }
            }
            "hash" => {
                let hash_len: usize = conn.hlen(key).await?;
                println!("   Hash length: {}", hash_len);
                let hash: HashMap<String, String> = conn.hgetall(key).await?;
                for (j, (field, value)) in hash.iter().take(5).enumerate() {
                    println!("   {}. {}: {}", j + 1, field, value);
                }
                if hash_len > 5 {
                    println!("   ... and {} more fields", hash_len - 5);
                }
            }
            _ => {
                println!("   Unsupported key type: {}", key_type);
            }
        }

        println!(); // Empty line for readability
    }

    if total_keys > limit {
        println!(
            "... and {} more keys (use --limit to see more)",
            total_keys - limit
        );
    }

    Ok(())
}

async fn watch_keys(
    conn: &mut redis::aio::MultiplexedConnection,
    pattern: &str,
    limit: usize,
    pretty_json: bool,
) -> Result<()> {
    println!(
        "👀 Watching keys matching pattern: {} (press Ctrl+C to stop)",
        pattern
    );
    println!("📊 Refreshing every 2 seconds...\n");

    loop {
        // Clear screen
        print!("\x1B[2J\x1B[1;1H");

        println!(
            "🔄 Redis Key Watcher - {} at {}",
            pattern,
            chrono::Local::now().format("%H:%M:%S")
        );
        println!("{}", "=".repeat(60));

        if let Err(e) = list_keys(conn, pattern, limit, pretty_json).await {
            eprintln!("❌ Error reading keys: {}", e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

async fn subscribe_channel(client: &Client, channel: &str) -> Result<()> {
    println!("📡 Subscribing to channel: {}", channel);
    println!("💡 Waiting for messages (press Ctrl+C to stop)...\n");

    let mut pubsub = client.get_async_pubsub().await?;
    pubsub.subscribe(channel).await?;
    let mut stream = pubsub.on_message();

    let mut message_count = 0;
    while let Some(msg) = stream.next().await {
        message_count += 1;
        let channel: String = msg.get_channel_name().to_string();
        let payload: String = msg.get_payload()?;
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");

        println!("📨 Message #{} at {}", message_count, timestamp);
        println!("   Channel: {}", channel);

        // Try to parse as JSON for pretty printing
        match serde_json::from_str::<Value>(&payload) {
            Ok(json_value) => {
                println!("   Payload (JSON):");
                let pretty = serde_json::to_string_pretty(&json_value)?;
                for line in pretty.lines() {
                    println!("   {}", line);
                }
            }
            Err(_) => {
                println!("   Payload: {}", payload);
            }
        }
        println!();
    }

    Ok(())
}
