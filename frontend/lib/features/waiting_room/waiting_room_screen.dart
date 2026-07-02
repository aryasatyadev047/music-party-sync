import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/participant_tile.dart';
import '../../shared/widgets/primary_button.dart';
import '../../shared/widgets/qr_card.dart';
import '../../shared/widgets/room_info_card.dart';
import '../player/player_screen.dart';

class WaitingRoomScreen extends StatelessWidget {
  const WaitingRoomScreen({super.key});

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

                const SizedBox(height: AppSpacing.lg),

                Expanded(
                  child: LayoutBuilder(
                    builder: (context, constraints) {

                      final desktop = constraints.maxWidth > 900;

                      if (desktop) {

                        return Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [

                            Expanded(
                              flex: 2,
                              child: Column(
                                children: [

                                  const RoomInfoCard(
                                    roomName: "Saturday Party",
                                    roomCode: "ABCD-1234",
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
                                    style: AppTextStyles.title,
                                  ),

                                  const SizedBox(
                                    height: AppSpacing.md,
                                  ),

                                  Expanded(
                                    child: ListView(
                                      children: const [
                                                                                ParticipantTile(
                                          name: "Satyadev",
                                          isHost: true,
                                        ),

                                        SizedBox(
                                          height: AppSpacing.md,
                                        ),

                                        ParticipantTile(
                                          name: "Rahul",
                                        ),

                                        SizedBox(
                                          height: AppSpacing.md,
                                        ),

                                        ParticipantTile(
                                          name: "Aman",
                                        ),

                                        SizedBox(
                                          height: AppSpacing.md,
                                        ),

                                        ParticipantTile(
                                          name: "Riya",
                                        ),
                                      ],
                                    ),
                                  ),

                                  const SizedBox(
                                    height: AppSpacing.lg,
                                  ),

                                  PrimaryButton(
  title: "Start Party",
  onTap: () {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => const PlayerScreen(),
      ),
    );
  },
),
                                ],
                              ),
                            ),
                          ],
                        );
                      }

                      return SingleChildScrollView(
                        child: Column(
                          children: [

                            const RoomInfoCard(
                              roomName: "Saturday Party",
                              roomCode: "ABCD-1234",
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

                            const ParticipantTile(
                              name: "Satyadev",
                              isHost: true,
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            const ParticipantTile(
                              name: "Rahul",
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            const ParticipantTile(
                              name: "Aman",
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            const ParticipantTile(
                              name: "Riya",
                            ),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            PrimaryButton(
                              title: "Start Party",
                              onTap: () {},
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