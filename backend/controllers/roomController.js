const { v4: uuidv4 } = require("uuid");

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

  room.members.push({
    name: userName,
    host: false,
  });

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