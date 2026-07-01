import 'package:flutter/material.dart';

import '../../core/constants/app_colors.dart';

class GlowLogo extends StatelessWidget {
  const GlowLogo({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 130,
      height: 130,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: AppColors.card,
        boxShadow: [
          BoxShadow(
            color: AppColors.primary.withValues(alpha: 0.5),
            blurRadius: 40,
            spreadRadius: 5,
          ),
        ],
      ),
      child: const Icon(
  Icons.graphic_eq_rounded,
  color: AppColors.primary,
  size: 62,
),
    );
  }
}   