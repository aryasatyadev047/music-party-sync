import 'package:flutter/material.dart';

import '../../core/constants/app_colors.dart';
import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import 'glass_card.dart';

class ParticipantTile extends StatelessWidget {
  final String name;
  final bool isHost;
  final bool isConnected;

  const ParticipantTile({
    super.key,
    required this.name,
    this.isHost = false,
    this.isConnected = true,
  });

  @override
  Widget build(BuildContext context) {
    return GlassCard(
      child: Row(
        children: [
          CircleAvatar(
            radius: 24,
            backgroundColor: AppColors.primary.withValues(alpha: 0.15),
            child: Icon(
              Icons.person,
              color: AppColors.primary,
            ),
          ),

          const SizedBox(width: AppSpacing.md),

          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  name,
                  style: AppTextStyles.title,
                ),

                const SizedBox(height: 4),

                Text(
                  isHost ? "Host" : "Participant",
                  style: AppTextStyles.body,
                ),
              ],
            ),
          ),

          Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(
              color: isConnected ? Colors.green : Colors.red,
              shape: BoxShape.circle,
            ),
          ),
        ],
      ),
    );
  }
}