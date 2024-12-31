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

use dotenv::dotenv;

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
    priority: i32,
}

async fn construct_gotify_payload(Json(duplicati_payload): Json<DuplicatiPayload>) -> Result<GotifyPayload, Box<dyn Error>> {
    println!("[INFO] Received DuplicatiPayload: {:?}", duplicati_payload);

    let parsed_result = &duplicati_payload.data.parsed_result;
    let mut title;

    if parsed_result == "Success" {
        title = env::var("GOTIFY_TITLE").unwrap_or_else(|_| "🟩".to_string());
    } else if parsed_result == "Warning" {
        title = env::var("GOTIFY_TITLE").unwrap_or_else(|_| "🟨".to_string());
    } else if parsed_result == "Error" {
        title = env::var("GOTIFY_TITLE").unwrap_or_else(|_| "🟥".to_string());
    } else {
        return Err(format!("Unknown parsed_result: {}", parsed_result).into());
    };

    title.push_str(&format!(" {}: {}", duplicati_payload.extra.operation_name, duplicati_payload.extra.backup_name));


    // Retrieve the environment variable (comma-separated list)
    let list_message_items = env::var("GOTIFY_MESSAGE_ITEMS").unwrap_or_else(|_| "backup_name,machine_name,operation_name,deleted_files,added_files,examined_files,size_of_added_files,main_operation,parsed_result,duration".to_string());

    // Split the list into individual tags
    let message_items: Vec<&str> = list_message_items.split(',').collect();

    // Construct the message for Gotify
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
            _ => {
                message.push_str(&format!("Unknown Item: {}\n", item));
            }
        }
    }


    let priority_env = env::var("GOTIFY_PRIORITY").unwrap_or_else(|_| "10".to_string());

    let gotify_payload = GotifyPayload {
        title,
        message,
        priority: priority_env.parse::<i32>().unwrap_or(10),
    };

    println!("[INFO] Final GotifyPayload: {:?}", gotify_payload);

    Ok(gotify_payload)
}

async fn gotify_send(gotify_payload: GotifyPayload) -> Result<bool, String> {


    // Retrieve environment variables
    let server_url = env::var("GOTIFY_SERVER_URL").expect("GOTIFY_SERVER_URL is not set");
    let app_token = env::var("GOTIFY_APP_TOKEN").expect("GOTIFY_APP_TOKEN is not set");


    let url = format!("{}/message?token={}", server_url, app_token);

    let client = Client::new();

    // Send the JSON payload
    let response = client
        .post(&url)
        .json(&gotify_payload)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("[INFO] Gotify notification sent successfully: {}", gotify_payload.title);
                Ok(true)
            } else {
                Err(format!("Failed to send Gotify payload. HTTP status: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Failed to send Gotify payload. Network error: {}", e)),
    }
}

async fn report_handler(Json(duplicati_payload): Json<DuplicatiPayload>) -> impl IntoResponse {
    println!("[INFO] Received payload: {:?}", duplicati_payload);

    // Construct the Gotify payload
    let gotify_payload = match construct_gotify_payload(Json(duplicati_payload)).await {
        Ok(payload) => payload,
        Err(e) => {
            println!("[ERROR] Failed to construct Gotify payload: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to construct Gotify payload");
        }
    };

    // Send the Gotify payload
    match gotify_send(gotify_payload).await {
        Ok(true) => {
            println!("[INFO] Gotify notification sent successfully.");
            (StatusCode::OK, "Notification sent successfully")
        }
        Ok(false) => {
            println!("[WARN] Gotify notification was not sent.");
            (StatusCode::INTERNAL_SERVER_ERROR, "Notification not sent")
        }
        Err(e) => {
            println!("[ERROR] Failed to send Gotify notification: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send Gotify notification")
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("[DEBUG] Loading environment variables...");

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let tcp = TcpListener::bind(&addr).await.unwrap();

    // Define the router
    let router: Router = Router::new().route("/report", post(report_handler));

    println!("Server running on http://{}", addr);

    axum::serve(tcp, router).await.unwrap();
}
