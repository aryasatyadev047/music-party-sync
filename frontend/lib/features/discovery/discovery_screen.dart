import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/device_tile.dart';
import '../../shared/widgets/page_background.dart';
import '../waiting_room/waiting_room_screen.dart';

class DiscoveryScreen extends StatelessWidget {
  const DiscoveryScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.lg),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                IconButton(
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.arrow_back),
                ),
                const SizedBox(height: AppSpacing.md),
                Text(
                  "Discover Devices",
                  style: AppTextStyles.heading,
                ),
                const SizedBox(height: 8),
                Text(
                  "Searching nearby devices...",
                  style: AppTextStyles.body,
                ),
                const SizedBox(height: 30),
                const Center(
                  child: CircularProgressIndicator(),
                ),
                const SizedBox(height: 30),
                Expanded(
                  child: ListView(
                    children: [
                      DeviceTile(
                        deviceName: "Satyadev's Phone",
                        deviceType: "Phone",
                        onTap: () {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (_) => const WaitingRoomScreen(),
                            ),
                          );
                        },
                      ),
                      const SizedBox(height: AppSpacing.md),
                      DeviceTile(
                        deviceName: "Rahul's Laptop",
                        deviceType: "Laptop",
                        onTap: () {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (_) => const WaitingRoomScreen(),
                            ),
                          );
                        },
                      ),
                      const SizedBox(height: AppSpacing.md),
                      DeviceTile(
                        deviceName: "Aman's Phone",
                        deviceType: "Phone",
                        onTap: () {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (_) => const WaitingRoomScreen(),
                            ),
                          );
                        },
                      ),
                    ],
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