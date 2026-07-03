import 'package:socket_io_client/socket_io_client.dart' as IO;
import 'package:flutter/foundation.dart';

class SocketService {
  static final SocketService instance = SocketService._internal();

  late IO.Socket socket;

  SocketService._internal();

  void connect() {
    socket = IO.io(
      "http://172.20.10.2:5000",
      IO.OptionBuilder()
          .setTransports(['websocket'])
          .disableAutoConnect()
          .build(),
    );

    socket.connect();

    socket.onConnect((_) {
     debugPrint("✅ Socket Connected");
    });

    socket.onDisconnect((_) {
      debugPrint("❌ Socket Disconnected");
    });
  }

  void joinRoom(String roomId) {
    socket.emit("join-room", roomId);
  }

  void leaveRoom(String roomId) {
    socket.emit("leave-room", roomId);
  }

  void startParty(String roomId) {
    socket.emit("start-party", roomId);
  }

  void onRoomUpdated(Function(dynamic) callback) {
    socket.on("room-updated", callback);
  }

  void onPartyStarted(Function(dynamic) callback) {
    socket.on("party-started", callback);
  }

  void dispose() {
    socket.dispose();
  }
}