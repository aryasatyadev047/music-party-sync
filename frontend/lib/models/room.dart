import 'member.dart';

class Room {
  final String roomId;
  final String roomName;
  final String hostName;
  final List<Member> members;
  final DateTime createdAt;

  Room({
    required this.roomId,
    required this.roomName,
    required this.hostName,
    required this.members,
    required this.createdAt,
  });

  factory Room.fromJson(Map<String, dynamic> json) {
    return Room(
      roomId: json["roomId"] ?? "",
      roomName: json["roomName"] ?? "",
      hostName: json["hostName"] ?? "",
      members: (json["members"] as List<dynamic>? ?? [])
          .map((e) => Member.fromJson(e))
          .toList(),
      createdAt: DateTime.tryParse(json["createdAt"] ?? "") ?? DateTime.now(),
    );
  }

  Map<String, dynamic> toJson() {
    return {
      "roomId": roomId,
      "roomName": roomName,
      "hostName": hostName,
      "members": members.map((e) => e.toJson()).toList(),
      "createdAt": createdAt.toIso8601String(),
    };
  }
}