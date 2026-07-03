const express = require("express");
const http = require("http");
const { Server } = require("socket.io");
const cors = require("cors");

const roomRoutes = require("./routes/roomRoutes");
const socketHandler = require("./sockets/socketHandler");
const { initSocket } = require("./services/socketService");

const app = express();
const server = http.createServer(app);

// ================================
// Socket.IO
// ================================
const io = new Server(server, {
  cors: {
    origin: "*",
    methods: ["GET", "POST"],
  },
});

// Initialize Socket Service
initSocket(io);

// ================================
// Middleware
// ================================
app.use(cors());
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// ================================
// Health Check
// ================================
app.get("/", (req, res) => {
  res.json({
    success: true,
    app: "SyncBeat Backend",
    version: "1.0.0",
    status: "Running",
  });
});

// ================================
// API Routes
// ================================
app.use("/api/rooms", roomRoutes);

// ================================
// Socket Handler
// ================================
socketHandler(io);

// ================================
// Server
// ================================
const PORT = process.env.PORT || 5000;

server.listen(PORT, () => {
  console.log("======================================");
  console.log("🎵 SyncBeat Backend Started");
  console.log(`🚀 Server running on port ${PORT}`);
  console.log(`🌐 http://localhost:${PORT}`);
  console.log("======================================");
});