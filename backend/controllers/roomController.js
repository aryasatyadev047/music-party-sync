const { v4: uuidv4 } = require("uuid");
const { getIO } = require("../services/socketService");

// Temporary in-memory storage
const rooms = {};

const createRoom = (req, res) => {
  const { roomName, hostName } = req.body;

  const roomId = "SB-" + uuidv4().substring(0, 6).toUpperCase();

  rooms[roomId] = {
    roomId,
    roomName,
    hostName,
    members: [
      {
        name: hostName,
        host: true,
      },
    ],
    createdAt: new Date(),
  };

  res.status(201).json({
    success: true,
    room: rooms[roomId],
  });
};

const joinRoom = (req, res) => {
  const { roomId, userName } = req.body;

  const room = rooms[roomId];

  if (!room) {
    return res.status(404).json({
      success: false,
      message: "Room not found",
    });
  }

  // Prevent duplicate names
  const exists = room.members.find(
    (member) => member.name === userName,
  );

  if (!exists) {
    room.members.push({
      name: userName,
      host: false,
    });
  }

  // Broadcast updated room to everyone connected
  try {
    const io = getIO();

    io.to(roomId).emit("room-updated", room);

    console.log(`📢 Room Updated: ${roomId}`);
  } catch (err) {
    console.log("Socket not initialized yet.");
  }

  res.json({
    success: true,
    room,
  });
};

const getRoom = (req, res) => {
  const room = rooms[req.params.roomId];

  if (!room) {
    return res.status(404).json({
      success: false,
      message: "Room not found",
    });
  }

  res.json({
    success: true,
    room,
  });
};

module.exports = {
  createRoom,
  joinRoom,
  getRoom,
};