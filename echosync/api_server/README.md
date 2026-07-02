# EchoSync API Server

Small HTTP API layer for the Flutter frontend.

## Run

```powershell
cargo run -p api_server
```

By default it listens on:

```text
http://0.0.0.0:5000
```

The Flutter app currently points to:

```text
http://192.168.1.11:5000/api
```

Make sure `192.168.1.11` is the IP address of the machine running this server.

## Endpoints

```text
GET /api/health
POST /api/rooms/create
GET /api/rooms/{roomId}
```

`POST /api/rooms/create` accepts the frontend payload:

```json
{
  "roomName": "Party",
  "hostName": "Host"
}
```

And returns the frontend-compatible response:

```json
{
  "success": true,
  "room": {
    "roomId": "ABC123",
    "roomName": "Party",
    "hostName": "Host"
  }
}
```
