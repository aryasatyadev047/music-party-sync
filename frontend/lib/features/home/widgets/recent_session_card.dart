import 'package:flutter/material.dart';

import '../../../core/constants/app_spacing.dart';
import '../../../core/constants/app_text_styles.dart';
import '../../../shared/widgets/glass_card.dart';

class RecentSessionCard extends StatelessWidget {
  const RecentSessionCard({super.key});

  @override
  Widget build(BuildContext context) {
    return GlassCard(
      child: Row(
        children: [
          const Icon(
            Icons.history,
            color: Colors.white70,
          ),

          const SizedBox(width: AppSpacing.md),

          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  "No Recent Sessions",
                  style: AppTextStyles.title,
                ),
                const SizedBox(height: 6),
                Text(
                  "Create or join a room to start listening together.",
                  style: AppTextStyles.body,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}