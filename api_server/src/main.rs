use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
struct Room {
    room_id: String,
    room_name: String,
    host_name: String,
}

type Rooms = Arc<Mutex<HashMap<String, Room>>>;

fn main() -> std::io::Result<()> {
    let bind_addr = std::env::var("ECHOSYNC_API_ADDR").unwrap_or_else(|_| "0.0.0.0:5000".to_string());
    let listener = TcpListener::bind(&bind_addr)?;
    let rooms = Arc::new(Mutex::new(HashMap::new()));

    println!("EchoSync API server listening on http://{}", bind_addr);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let rooms = Arc::clone(&rooms);
                std::thread::spawn(move || {
                    if let Err(error) = handle_client(stream, rooms) {
                        eprintln!("request failed: {}", error);
                    }
                });
            }
            Err(error) => eprintln!("connection failed: {}", error),
        }
    }

    Ok(())
}

fn handle_client(mut stream: TcpStream, rooms: Rooms) -> std::io::Result<()> {
    let request = read_http_request(&mut stream)?;
    if request.is_empty() {
        return Ok(());
    }

    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();

    match (method, path) {
        ("OPTIONS", _) => write_response(&mut stream, 204, "No Content", ""),
        ("GET", "/api/health") => write_response(&mut stream, 200, "OK", "{\"status\":\"ok\"}"),
        ("POST", "/api/rooms/create") => create_room(&mut stream, body, rooms),
        ("GET", path) if path.starts_with("/api/rooms/") => get_room(&mut stream, path, rooms),
        _ => write_response(
            &mut stream,
            404,
            "Not Found",
            "{\"error\":\"route not found\"}",
        ),
    }
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];

    loop {
        let bytes_read = stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..bytes_read]);

        if let Some(header_end) = find_header_end(&bytes) {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let total_expected = header_end + 4 + content_length;
            if bytes.len() >= total_expected {
                break;
            }
        }
    }

    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn create_room(stream: &mut TcpStream, body: &str, rooms: Rooms) -> std::io::Result<()> {
    let room_name = json_string_value(body, "roomName").unwrap_or_else(|| "Music Room".to_string());
    let host_name = json_string_value(body, "hostName").unwrap_or_else(|| "Host".to_string());
    let room_id = generate_room_id();

    let room = Room {
        room_id: room_id.clone(),
        room_name,
        host_name,
    };

    rooms.lock().expect("rooms lock poisoned").insert(room_id, room.clone());

    write_response(stream, 200, "OK", &room_json(&room))
}

fn get_room(stream: &mut TcpStream, path: &str, rooms: Rooms) -> std::io::Result<()> {
    let room_id = path.trim_start_matches("/api/rooms/");
    let rooms = rooms.lock().expect("rooms lock poisoned");

    if let Some(room) = rooms.get(room_id) {
        write_response(stream, 200, "OK", &room_json(room))
    } else {
        write_response(stream, 404, "Not Found", "{\"error\":\"room not found\"}")
    }
}

fn write_response(
    stream: &mut TcpStream,
    status_code: u16,
    reason: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n{}",
        status_code,
        reason,
        body.as_bytes().len(),
        body
    );

    stream.write_all(response.as_bytes())
}

fn room_json(room: &Room) -> String {
    format!(
        "{{\"success\":true,\"room\":{{\"roomId\":\"{}\",\"roomName\":\"{}\",\"hostName\":\"{}\"}}}}",
        json_escape(&room.room_id),
        json_escape(&room.room_name),
        json_escape(&room.host_name)
    )
}

fn json_string_value(body: &str, key: &str) -> Option<String> {
    let key_pattern = format!("\"{}\"", key);
    let key_pos = body.find(&key_pattern)?;
    let after_key = &body[key_pos + key_pattern.len()..];
    let colon_pos = after_key.find(':')?;
    let after_colon = after_key[colon_pos + 1..].trim_start();

    if !after_colon.starts_with('"') {
        return None;
    }

    let mut value = String::new();
    let mut chars = after_colon[1..].chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(value),
            '\\' => {
                if let Some(escaped) = chars.next() {
                    value.push(match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        '/' => '/',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => other,
                    });
                }
            }
            other => value.push(other),
        }
    }

    None
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn generate_room_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let short = millis % 36_u128.pow(6);
    format!("{:0>6}", to_base36(short)).to_uppercase()
}

fn to_base36(mut value: u128) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    if value == 0 {
        return "0".to_string();
    }

    let mut chars = Vec::new();
    while value > 0 {
        let digit = (value % 36) as usize;
        chars.push(DIGITS[digit] as char);
        value /= 36;
    }

    chars.iter().rev().collect()
}
