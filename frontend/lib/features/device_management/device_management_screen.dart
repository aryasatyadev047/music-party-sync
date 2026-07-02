import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/glass_card.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/primary_button.dart';

class DeviceManagementScreen extends StatelessWidget {
  const DeviceManagementScreen({super.key});

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
                Center(
                  child: Text(
                    "Device Management",
                    style: AppTextStyles.heading,
                  ),
                ),

                const SizedBox(height: AppSpacing.lg),

                Expanded(
                  child: GlassCard(
                    child: ListView(
                      children: const [
                        ListTile(
                          leading: CircleAvatar(
                            backgroundColor: Colors.green,
                            child: Icon(Icons.phone_android,
                                color: Colors.white),
                          ),
                          title: Text(
                            "Satyadev's Phone",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: Text(
                            "Android • Battery 86%",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),
                        Divider(),

                        ListTile(
                          leading: CircleAvatar(
                            backgroundColor: Colors.green,
                            child:
                                Icon(Icons.laptop, color: Colors.white),
                          ),
                          title: Text(
                            "Rahul's Laptop",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: Text(
                            "Windows • Battery 72%",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),
                        Divider(),

                        ListTile(
                          leading: CircleAvatar(
                            backgroundColor: Colors.green,
                            child: Icon(Icons.phone_android,
                                color: Colors.white),
                          ),
                          title: Text(
                            "Aman's Phone",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: Text(
                            "Android • Battery 94%",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),
                        Divider(),

                        ListTile(
                          leading: CircleAvatar(
                            backgroundColor: Colors.green,
                            child: Icon(Icons.phone_android,
                                color: Colors.white),
                          ),
                          title: Text(
                            "Riya's Phone",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: Text(
                            "Android • Battery 63%",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),

                const SizedBox(height: AppSpacing.lg),

                const GlassCard(
                  child: Column(
                    children: [
                      Row(
                        children: [
                          Icon(Icons.check_circle,
                              color: Colors.green),
                          SizedBox(width: 10),
                          Text(
                            "4 Devices Connected",
                            style: TextStyle(
                              color: Colors.white,
                              fontSize: 18,
                            ),
                          ),
                        ],
                      ),
                      SizedBox(height: 12),
                      Row(
                        children: [
                          Icon(Icons.speed,
                              color: Colors.cyan),
                          SizedBox(width: 10),
                          Text(
                            "Latency : 18 ms",
                            style: TextStyle(
                              color: Colors.white70,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),

                const SizedBox(height: AppSpacing.lg),

                Row(
                  children: [
                    Expanded(
                      child: PrimaryButton(
                        title: "Refresh",
                        onTap: () {},
                      ),
                    ),
                    const SizedBox(width: AppSpacing.md),
                    Expanded(
                      child: PrimaryButton(
                        title: "Disconnect",
                        onTap: () {},
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}