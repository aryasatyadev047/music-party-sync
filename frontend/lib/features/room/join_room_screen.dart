import 'package:flutter/material.dart';

import '../../core/services/room_service.dart';
import '../../models/room.dart';
import '../waiting_room/waiting_room_screen.dart';

class JoinRoomScreen extends StatefulWidget {
  const JoinRoomScreen({super.key});

  @override
  State<JoinRoomScreen> createState() => _JoinRoomScreenState();
}

class _JoinRoomScreenState extends State<JoinRoomScreen> {
  final roomCodeController = TextEditingController();
  final nameController = TextEditingController();

  bool loading = false;

  @override
  void dispose() {
    roomCodeController.dispose();
    nameController.dispose();
    super.dispose();
  }

  Future<void> joinRoom() async {
    if (roomCodeController.text.trim().isEmpty ||
        nameController.text.trim().isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text("Please enter Room Code and Your Name"),
        ),
      );
      return;
    }

    setState(() {
      loading = true;
    });

    try {
      final Room room = await RoomService.joinRoom(
        roomCodeController.text.trim(),
        nameController.text.trim(),
      );

      if (!mounted) return;

      Navigator.pushReplacement(
        context,
        MaterialPageRoute(
          builder: (_) => WaitingRoomScreen(room: room),
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

    if (mounted) {
      setState(() {
        loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Join Room"),
      ),
      body: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            TextField(
              controller: roomCodeController,
              decoration: const InputDecoration(
                labelText: "Room Code",
              ),
            ),

            const SizedBox(height: 20),

            TextField(
              controller: nameController,
              decoration: const InputDecoration(
                labelText: "Your Name",
              ),
            ),

            const SizedBox(height: 30),

            SizedBox(
              width: double.infinity,
              child: ElevatedButton(
                onPressed: loading ? null : joinRoom,
                child: loading
                    ? const CircularProgressIndicator()
                    : const Text("Join Room"),
              ),
            ),
          ],
        ),
      ),
    );
  }
}