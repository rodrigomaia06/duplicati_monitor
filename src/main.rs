use axum::{
    Router,
    routing::post,
    extract::Json,
    response::IntoResponse,
    http::StatusCode,
};
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use reqwest::Client;
use colored::*;

//For development
//use dotenv::dotenv;

// Define the structure for the JSON payload
#[derive(Deserialize, Debug)]
struct DuplicatiPayload {
    #[serde(rename = "Data")]
    data: BackupData,
    #[serde(rename = "Extra")]
    extra: ExtraInfo,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct BackupData {
    #[serde(rename = "DeletedFiles")]
    deleted_files: u64,
    #[serde(rename = "AddedFiles")]
    added_files: u64,
    #[serde(rename = "ExaminedFiles")]
    examined_files: u64,
    #[serde(rename = "SizeOfAddedFiles")]
    size_of_added_files: u64,
    #[serde(rename = "MainOperation")]
    main_operation: String,
    #[serde(rename = "ParsedResult")]
    parsed_result: String,
    #[serde(rename = "Duration")]
    duration: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct ExtraInfo {
    #[serde(rename = "OperationName")]
    operation_name: String,
    #[serde(rename = "machine-name")]
    machine_name: String,
    #[serde(rename = "backup-name")]
    backup_name: String,
}

#[derive(serde::Serialize, Debug)]
struct GotifyPayload {
    title: String,
    message: String,
    priority: u32,
}

fn is_debug_mode() -> bool {
    env::var("DEBUG_MODE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
}

async fn construct_gotify_payload(Json(duplicati_payload): Json<DuplicatiPayload>) -> Result<GotifyPayload, Box<dyn Error>> {
    
    print!("{}", "[INFO] Received DuplicatiPayload: ".green().bold());
    println!("{:?}", duplicati_payload);
    

    let parsed_result = &duplicati_payload.data.parsed_result;
    let title;

    if parsed_result == "Success" {
        title = env::var("GOTIFY_SUCCESS_MESSAGE").unwrap_or_else(|_| format!("💾🟢 Duplicati {}: {}", duplicati_payload.extra.operation_name, duplicati_payload.extra.backup_name));
    } else if parsed_result == "Warning" {
        title = env::var("GOTIFY_WARNING_MESSAGE").unwrap_or_else(|_| format!("💾🟡 Duplicati {}: {}", duplicati_payload.extra.operation_name, duplicati_payload.extra.backup_name));
    } else if parsed_result == "Error" {
        title = env::var("GOTIFY_ERROR_MESSAGE").unwrap_or_else(|_| format!("💾🔴 Duplicati {}: {}", duplicati_payload.extra.operation_name, duplicati_payload.extra.backup_name));
    } else {
        println!("{}", "[ERROR] Unknown parsed_result.".red().bold());
        return Err(format!("Unknown parsed_result: {}", parsed_result).into());
    };

    if is_debug_mode() {
        print!("{}", "[DEBUG] Title constructed: ".blue().bold());
        println!("{}", title);
    }

    let list_message_items = env::var("GOTIFY_MESSAGE_ITEMS").unwrap_or_else(|_| "backup_name,machine_name,operation_name,deleted_files,added_files,examined_files,size_of_added_files,main_operation,parsed_result,duration".to_string());
    if is_debug_mode() {
        print!("{}", "[DEBUG] GOTIFY_MESSAGE_ITEMS: ".blue().bold());
        println!("{}", list_message_items);
    }

    let message_items: Vec<&str> = list_message_items.split(',').collect();

    let mut message = String::new();
    for item in message_items.into_iter() {
        match item {
            "machine_name" => {
                message.push_str(&format!("Machine Name: {}\n", duplicati_payload.extra.machine_name));
            }
            "deleted_files" => {
                message.push_str(&format!("Deleted Files: {}\n", duplicati_payload.data.deleted_files));
            }
            "added_files" => {
                message.push_str(&format!("Added Files: {}\n", duplicati_payload.data.added_files));
            }
            "examined_files" => {
                message.push_str(&format!("Examined Files: {}\n", duplicati_payload.data.examined_files));
            }
            "size_of_added_files" => {
                message.push_str(&format!("Size of Added Files: {}\n", duplicati_payload.data.size_of_added_files));
            }
            "main_operation" => {
                message.push_str(&format!("Main Operation: {}\n", duplicati_payload.data.main_operation));
            }
            "parsed_result" => {
                message.push_str(&format!("Parsed Result: {}\n", duplicati_payload.data.parsed_result));
            }
            "duration" => {
                message.push_str(&format!("Duration: {}\n", duplicati_payload.data.duration));
            }
            "operation_name" => {
                message.push_str(&format!("Operation Name: {}\n", duplicati_payload.extra.operation_name));
            }
            "backup_name" => {
                message.push_str(&format!("Backup Name: {}\n", duplicati_payload.extra.backup_name));
            }
            _ => {
                message.push_str(&format!("Unknown Item: {}\n", item));
            }
        }
    }

    if is_debug_mode() {
        print!("{}", "[DEBUG] Constructed message:".blue().bold());
        println!("{}", message);
    }

    let priority_env = env::var("GOTIFY_PRIORITY").unwrap_or_else(|_| "10".to_string());

    let gotify_payload = GotifyPayload {
        title,
        message,
        priority: priority_env.parse::<u32>().unwrap_or(10),
    };

    if is_debug_mode() {
        print!("{}", "[DEBUG] Final GotifyPayload: ".blue().bold());
        println!("{:?}", gotify_payload);
    }

    Ok(gotify_payload)
}

async fn gotify_send(gotify_payload: GotifyPayload) -> Result<bool, String> {
    if is_debug_mode() {
        print!("{}", "[DEBUG] Sending GotifyPayload: ".blue().bold());
        println!("{:?}", gotify_payload);
    }

    let server_url = env::var("GOTIFY_SERVER_URL").expect("GOTIFY_SERVER_URL is not set");
    let app_token = env::var("GOTIFY_APP_TOKEN").expect("GOTIFY_APP_TOKEN is not set");

    if is_debug_mode() {
        print!("{}", "[DEBUG] GOTIFY_SERVER_URL: ".blue().bold());
        println!("{}", server_url);
        print!("{}", "[DEBUG] GOTIFY_APP_TOKEN: ".blue().bold());
        println!("{}", app_token);
    }

    let url = format!("{}/message?token={}", server_url, app_token);

    if is_debug_mode() {
        print!("{}", "[DEBUG] Final URL: ".blue().bold());
        println!("{}", url);
    }

    let client = Client::new();

    let response = client
        .post(&url)
        .json(&gotify_payload)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if is_debug_mode() {
                print!("{}", "[DEBUG] Response Status: ".blue().bold());
                println!("{}", resp.status());
            }
            if resp.status().is_success() {
                println!("{}", "[INFO] Gotify notification sent successfully.".green().bold());
                Ok(true)
            } else {
                println!("{}", "[WARN] Gotify notification failed to send.".yellow().bold());
                Err(format!("Failed to send Gotify payload. HTTP status: {}", resp.status()))
            }
        }
        Err(e) => {
            println!("{}", "[ERROR] Network error while sending Gotify payload.".red().bold());
            Err(format!("Failed to send Gotify payload. Network error: {}", e))
        }
    }
}

async fn report_handler(Json(duplicati_payload): Json<DuplicatiPayload>) -> impl IntoResponse {
    if is_debug_mode() {
        print!("{}", "[DEBUG] Handling /report with payload: ".blue().bold());
        println!("{:?}", duplicati_payload);
    }

    let gotify_payload = match construct_gotify_payload(Json(duplicati_payload)).await {
        Ok(payload) => payload,
        Err(_e) => {
            println!("{}", "[ERROR] Failed to construct Gotify payload.".red().bold());
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to construct Gotify payload",
            );
        }
    };

    match gotify_send(gotify_payload).await {
        Ok(true) => {
            (StatusCode::OK, "Notification sent successfully")
        }
        Ok(false) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Notification not sent")
        }
        Err(_e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send Gotify notification")
        }
    }
}

#[tokio::main]
async fn main() {
    //For development
    //dotenv().ok();

    // Define the list of environment variables you want to filter
    let filtered_env_vars = vec![
        "DEBUG_MODE",
        "GOTIFY_SERVER_URL",
        "GOTIFY_APP_TOKEN",
        "GOTIFY_PRIORITY",
        "GOTIFY_MESSAGE_ITEMS",
        "GOTIFY_TITLE",
        "GOTIFY_SUCCESS_MESSAGE",
        "GOTIFY_WARNING_MESSAGE",
        "GOTIFY_ERROR_MESSAGE",
    ];

    // Print filtered environment variables
    println!("{}", "[INFO] Listing selected environment variables at startup:".green().bold());
    for (key, value) in std::env::vars() {
        if filtered_env_vars.contains(&key.as_str()) {
            // Redact sensitive values, but show the last four characters
            let display_value = if key.to_lowercase().contains("token") {
                let masked_length = if value.len() > 4 { value.len() - 4 } else { 0 };
                format!(
                    "{}{}",
                    "*".repeat(masked_length),
                    &value[value.len().saturating_sub(4)..]
                )
                .yellow()
                .to_string()
            } else {
                value.blue().to_string()
            };
    
            println!("{}: {}", key.cyan().bold(), display_value);
        }
    }

    if is_debug_mode() {
        println!("{}", "[DEBUG] Environment variables loaded.".blue().bold());
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], 5050));
    let tcp = TcpListener::bind(&addr).await.unwrap();

    println!("{}", format!("[INFO] Server starting at http://{}", addr).green().bold());


    let router: Router = Router::new().route("/report", post(report_handler));

    axum::serve(tcp, router).await.unwrap();
}
