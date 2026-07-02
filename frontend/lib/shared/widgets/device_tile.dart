import 'package:flutter/material.dart';

import '../../core/constants/app_colors.dart';
import '../../core/constants/app_radius.dart';
import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import 'glass_card.dart';

class DeviceTile extends StatelessWidget {
  final String deviceName;
  final String deviceType;
  final bool isAvailable;
  final VoidCallback onTap;

  const DeviceTile({
    super.key,
    required this.deviceName,
    required this.deviceType,
    required this.onTap,
    this.isAvailable = true,
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
              deviceType == "Laptop"
                  ? Icons.laptop_mac_rounded
                  : Icons.phone_android_rounded,
              color: AppColors.primary,
            ),
          ),
          const SizedBox(width: AppSpacing.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  deviceName,
                  style: AppTextStyles.title,
                ),
                const SizedBox(height: 4),
                Text(
                  isAvailable ? "Available" : "Busy",
                  style: AppTextStyles.body.copyWith(
                    color:
                        isAvailable ? Colors.greenAccent : Colors.orangeAccent,
                  ),
                ),
              ],
            ),
          ),
          ElevatedButton(
            onPressed: onTap,
            style: ElevatedButton.styleFrom(
              backgroundColor: AppColors.primary,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(AppRadius.md),
              ),
            ),
            child: const Text("Connect"),
          ),
        ],
      ),
    );
  }
}