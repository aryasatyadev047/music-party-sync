import 'package:frontend/core/services/api_service.dart';
import 'package:frontend/models/room.dart';

class RoomService {
  static Future<Room> createRoom(
    String roomName,
    String hostName,
  ) async {
    final response = await ApiService.dio.post(
      '/rooms/create',
      data: {
        'roomName': roomName,
        'hostName': hostName,
      },
    );

    return Room.fromJson(response.data["room"]);
  }

  static Future<Room> joinRoom(
    String roomId,
    String userName,
  ) async {
    final response = await ApiService.dio.post(
      '/rooms/join',
      data: {
        'roomId': roomId,
        'userName': userName,
      },
    );

    return Room.fromJson(response.data["room"]);
  }

  static Future<Room> getRoom(
    String roomId,
  ) async {
    final response = await ApiService.dio.get(
      '/rooms/$roomId',
    );

    return Room.fromJson(response.data["room"]);
  }
}