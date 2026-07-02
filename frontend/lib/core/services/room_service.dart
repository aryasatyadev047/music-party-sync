import 'package:frontend/core/services/api_service.dart';

class RoomService {
  static Future<Map<String, dynamic>> createRoom(
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

    return Map<String, dynamic>.from(response.data);
  }
}