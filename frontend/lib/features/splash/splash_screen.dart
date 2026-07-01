import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';

import '../../shared/widgets/glow_logo.dart';
import '../../shared/widgets/page_background.dart';
import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../home/home_screen.dart';

class SplashScreen extends StatefulWidget {
  const SplashScreen({super.key});

  @override
  State<SplashScreen> createState() => _SplashScreenState();
}

class _SplashScreenState extends State<SplashScreen> {
  @override
  void initState() {
    super.initState();

    Timer(const Duration(seconds: 3), () {
      Navigator.pushReplacement(
        context,
        MaterialPageRoute(
          builder: (_) => const HomeScreen(),
        ),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [

                const GlowLogo()
                    .animate()
                    .scale(
                      duration: 800.ms,
                      curve: Curves.easeOutBack,
                    )
                    .fadeIn(),

                const SizedBox(height: AppSpacing.xl),

                Text(
                  "SyncBeat",
                  style: AppTextStyles.heading,
                )
                    .animate(delay: 300.ms)
                    .fadeIn()
                    .slideY(begin: .4),

                const SizedBox(height: 8),

                Text(
                  "Listen Together",
                  style: AppTextStyles.body,
                )
                    .animate(delay: 700.ms)
                    .fadeIn(),

                const SizedBox(height: 50),

                const SizedBox(
                  width: 30,
                  height: 30,
                  child: CircularProgressIndicator(
                    strokeWidth: 3,
                  ),
                )
                    .animate(delay: 900.ms)
                    .fadeIn(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}