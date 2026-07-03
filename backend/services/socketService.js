let io = null;

const initSocket = (socketServer) => {
  io = socketServer;
};

const getIO = () => {
  if (!io) {
    throw new Error("Socket.IO not initialized");
  }

  return io;
};

module.exports = {
  initSocket,
  getIO,
};