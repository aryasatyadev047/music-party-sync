import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import 'glass_card.dart';

class RoomInfoCard extends StatelessWidget {
  final String roomName;
  final String roomCode;

  const RoomInfoCard({
    super.key,
    required this.roomName,
    required this.roomCode,
  });

  @override
  Widget build(BuildContext context) {
    return GlassCard(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            roomName,
            style: AppTextStyles.title,
          ),
          const SizedBox(height: AppSpacing.md),
          Text(
            "Room Code",
            style: AppTextStyles.body,
          ),
          const SizedBox(height: AppSpacing.sm),
          SelectableText(
            roomCode,
            style: AppTextStyles.heading.copyWith(
              fontSize: 26,
            ),
          ),
        ],
      ),
    );
  }
}