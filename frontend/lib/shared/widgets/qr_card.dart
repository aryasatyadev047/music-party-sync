import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import 'glass_card.dart';

class QRCard extends StatelessWidget {
  const QRCard({super.key});

  @override
  Widget build(BuildContext context) {
    return GlassCard(
      child: Column(
        children: [
          Text(
            "Room QR",
            style: AppTextStyles.title,
          ),

          const SizedBox(height: AppSpacing.lg),

          Container(
            width: 180,
            height: 180,
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(16),
            ),
            child: const Center(
              child: Icon(
                Icons.qr_code_2_rounded,
                size: 120,
                color: Colors.black,
              ),
            ),
          ),

          const SizedBox(height: AppSpacing.md),

          Text(
            "Scan to Join",
            style: AppTextStyles.body,
          ),
        ],
      ),
    );
  }
}