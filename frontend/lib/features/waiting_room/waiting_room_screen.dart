import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../core/services/socket_service.dart';
import '../../models/room.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/participant_tile.dart';
import '../../shared/widgets/primary_button.dart';
import '../../shared/widgets/qr_card.dart';
import '../../shared/widgets/room_info_card.dart';
import '../player/player_screen.dart';

class WaitingRoomScreen extends StatefulWidget {
  final Room room;

  const WaitingRoomScreen({
    super.key,
    required this.room,
  });

  @override
  State<WaitingRoomScreen> createState() =>
      _WaitingRoomScreenState();
}

class _WaitingRoomScreenState
    extends State<WaitingRoomScreen> {

  late Room room;

  @override
  void initState() {
    super.initState();

    room = widget.room;

    SocketService.instance.connect();

    SocketService.instance.joinRoom(room.roomId);

    SocketService.instance.onRoomUpdated((data) {

      final updatedRoom = Room.fromJson(data);

      if (!mounted) return;

      setState(() {
        room = updatedRoom;
      });

    });

    SocketService.instance.onPartyStarted((_) {

      if (!mounted) return;

      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (_) => const PlayerScreen(),
        ),
      );

    });
  }

  @override
  void dispose() {

    SocketService.instance.leaveRoom(room.roomId);

    SocketService.instance.dispose();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {

    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.lg),
            child: Column(
              children: [

                Text(
                  "Waiting Room",
                  style: AppTextStyles.heading,
                ),

                const SizedBox(
                  height: AppSpacing.lg,
                ),

                Expanded(
                  child: LayoutBuilder(
                    builder: (context, constraints) {

                      final desktop =
                          constraints.maxWidth > 900;

                      if (desktop) {

                        return Row(
                          crossAxisAlignment:
                              CrossAxisAlignment.start,
                          children: [

                            Expanded(
                              flex: 2,
                              child: Column(
                                children: [

                                  RoomInfoCard(
                                    roomName: room.roomName,
                                    roomCode: room.roomId,
                                  ),

                                  const SizedBox(
                                    height: AppSpacing.lg,
                                  ),

                                  const QRCard(),

                                ],
                              ),
                            ),

                            const SizedBox(
                              width: AppSpacing.lg,
                            ),

                            Expanded(
                              flex: 3,
                              child: Column(
                                crossAxisAlignment:
                                    CrossAxisAlignment.start,
                                children: [

                                  Text(
                                    "Participants",
                                    style:
                                        AppTextStyles.title,
                                  ),

                                  const SizedBox(
                                    height:
                                        AppSpacing.md,
                                  ),

                                  Expanded(
                                    child: ListView(
                                      children:
                                          room.members
                                              .map(
                                                (member) =>
                                                    Padding(
                                                  padding:
                                                      const EdgeInsets.only(
                                                    bottom:
                                                        AppSpacing.md,
                                                  ),
                                                  child:
                                                      ParticipantTile(
                                                    name:
                                                        member.name,
                                                    isHost:
                                                        member.host,
                                                  ),
                                                ),
                                              )
                                              .toList(),
                                    ),
                                  ),

                                  const SizedBox(
                                    height:
                                        AppSpacing.lg,
                                  ),

                                  PrimaryButton(
                                    title:
                                        "Start Party",
                                    onTap: () {

                                      SocketService
                                          .instance
                                          .startParty(
                                              room.roomId);

                                    },
                                  ),
                                ],
                              ),
                            ),
                          ],
                        );
                      }

                      // ===== CONTINUE IN PART 2 =====
                                            return SingleChildScrollView(
                        child: Column(
                          children: [

                            RoomInfoCard(
                              roomName: room.roomName,
                              roomCode: room.roomId,
                            ),

                            const SizedBox(
                              height: AppSpacing.lg,
                            ),

                            const QRCard(),

                            const SizedBox(
                              height: AppSpacing.lg,
                            ),

                            Align(
                              alignment: Alignment.centerLeft,
                              child: Text(
                                "Participants",
                                style: AppTextStyles.title,
                              ),
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            ...room.members.map(
                              (member) => Padding(
                                padding: const EdgeInsets.only(
                                  bottom: AppSpacing.md,
                                ),
                                child: ParticipantTile(
                                  name: member.name,
                                  isHost: member.host,
                                ),
                              ),
                            ),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            PrimaryButton(
                              title: "Start Party",
                              onTap: () {
                                SocketService.instance
                                    .startParty(room.roomId);
                              },
                            ),
                          ],
                        ),
                      );
                    },
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}