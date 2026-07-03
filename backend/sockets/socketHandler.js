module.exports = (io) => {
  io.on("connection", (socket) => {
    console.log(`✅ User Connected: ${socket.id}`);

    // ==========================
    // Join a Room
    // ==========================
    socket.on("join-room", (roomId) => {
      socket.join(roomId);

      console.log(
        `📥 Socket ${socket.id} joined room ${roomId}`
      );
    });

    // ==========================
    // Leave a Room
    // ==========================
    socket.on("leave-room", (roomId) => {
      socket.leave(roomId);

      console.log(
        `📤 Socket ${socket.id} left room ${roomId}`
      );
    });

    // ==========================
    // Start Party
    // ==========================
    socket.on("start-party", (roomId) => {
      console.log(
        `🎵 Party Started in ${roomId}`
      );

      io.to(roomId).emit("party-started");
    });

    // ==========================
    // Disconnect
    // ==========================
    socket.on("disconnect", () => {
      console.log(`❌ User Disconnected: ${socket.id}`);
    });
  });
};