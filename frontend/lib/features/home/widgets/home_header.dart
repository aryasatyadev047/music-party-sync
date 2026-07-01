import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';

import '../../../core/constants/app_spacing.dart';
import '../../../core/constants/app_text_styles.dart';
import '../../../shared/widgets/glow_logo.dart';

class HomeHeader extends StatelessWidget {
  const HomeHeader({super.key});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        const GlowLogo()
            .animate()
            .fadeIn(duration: 600.ms)
            .scale(),

        const SizedBox(height: AppSpacing.lg),

        Text(
          "SyncBeat",
          style: AppTextStyles.heading,
        )
            .animate(delay: 200.ms)
            .fadeIn(),

        const SizedBox(height: 8),

        Text(
          "Listen Together",
          style: AppTextStyles.body,
        )
            .animate(delay: 500.ms)
            .fadeIn(),

        const SizedBox(height: AppSpacing.xxl),
      ],
    );
  }
}