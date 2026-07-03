import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/primary_button.dart';
import 'widgets/home_header.dart';
import 'widgets/recent_session_card.dart';
import '../room/create_room_screen.dart';
import '../room/join_room_screen.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.lg),
            child: Column(
              children: [
                const HomeHeader(),

                PrimaryButton(
  title: "Create Room",
  onTap: () {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => const CreateRoomScreen(),
      ),
    );
  },
),

                const SizedBox(height: AppSpacing.md),

                OutlinedButton(
                  onPressed: () {
  Navigator.push(
    context,
    MaterialPageRoute(
      builder: (_) => const JoinRoomScreen(),
    ),
  );
},
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size(
                      double.infinity,
                      58,
                    ),
                    side: const BorderSide(
                      color: Color(0xFF00D4FF),
                      width: 2,
                    ),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(20),
                    ),
                  ),
                  child: const Text(
                    "Join Room",
                    style: TextStyle(
                      color: Color(0xFF00D4FF),
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),

                const SizedBox(height: AppSpacing.xxl),

                const RecentSessionCard(),

                const Spacer(),

                const Text(
                  "Version 1.0",
                  style: TextStyle(
                    color: Colors.white38,
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