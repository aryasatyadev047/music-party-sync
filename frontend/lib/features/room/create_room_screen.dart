import 'package:flutter/material.dart';

import '../../core/services/room_service.dart';
import '../../models/room.dart';
import '../waiting_room/waiting_room_screen.dart';

class CreateRoomScreen extends StatefulWidget {
  const CreateRoomScreen({super.key});

  @override
  State<CreateRoomScreen> createState() => _CreateRoomScreenState();
}

class _CreateRoomScreenState extends State<CreateRoomScreen> {
  final roomController = TextEditingController();
  final hostController = TextEditingController();

  @override
  void dispose() {
    roomController.dispose();
    hostController.dispose();
    super.dispose();
  }

  Future<void> createRoom() async {
    if (roomController.text.trim().isEmpty ||
        hostController.text.trim().isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text("Please enter Room Name and Host Name"),
        ),
      );
      return;
    }

    try {
      final Room room = await RoomService.createRoom(
        roomController.text.trim(),
        hostController.text.trim(),
      );

      if (!mounted) return;

      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            "Room Created: ${room.roomId}",
          ),
        ),
      );

      Navigator.pushReplacement(
  context,
  MaterialPageRoute(
    builder: (_) => WaitingRoomScreen(
      room: room,
    ),
  ),
);
    } catch (e) {
      if (!mounted) return;

      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(e.toString()),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Create Room"),
      ),
      body: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            TextField(
              controller: roomController,
              decoration: const InputDecoration(
                labelText: "Room Name",
              ),
            ),
            const SizedBox(height: 20),
            TextField(
              controller: hostController,
              decoration: const InputDecoration(
                labelText: "Host Name",
              ),
            ),
            const SizedBox(height: 30),
            ElevatedButton(
              onPressed: createRoom,
              child: const Text("Create Room"),
            ),
          ],
        ),
      ),
    );
  }
}