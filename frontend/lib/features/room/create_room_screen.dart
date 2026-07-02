import 'package:flutter/material.dart';

import '../../core/services/room_service.dart';

class CreateRoomScreen extends StatefulWidget {
  const CreateRoomScreen({super.key});

  @override
  State<CreateRoomScreen> createState() => _CreateRoomScreenState();
}

class _CreateRoomScreenState extends State<CreateRoomScreen> {

  final roomController = TextEditingController();
  final hostController = TextEditingController();

  bool loading = false;

  String roomId = "";

  Future<void> createRoom() async {
  setState(() {
    loading = true;
  });

  try {
    final response = await RoomService.createRoom(
      roomController.text,
      hostController.text,
    );

    if (!mounted) return;

    setState(() {
      roomId = response["room"]["roomId"].toString();
    });
  } catch (e) {
    if (!mounted) return;

    debugPrint("ERROR: $e");

    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(e.toString()),
      ),
    );
  } finally {
    if (mounted) {
      setState(() {
        loading = false;
      });
    }
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

              child: loading
                  ? const CircularProgressIndicator()
                  : const Text("Create Room"),

            ),

            const SizedBox(height: 30),

            Text(
              roomId,
              style: const TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.bold,
              ),
            ),

          ],
        ),
      ),
    );
  }
}