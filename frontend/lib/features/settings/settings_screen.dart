import 'package:flutter/material.dart';

import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/glass_card.dart';
import '../../shared/widgets/primary_button.dart';
import '../device_management/device_management_screen.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {

  bool darkMode = true;
  bool notifications = true;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.lg),
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [

                  Center(
                    child: Text(
                      "Settings",
                      style: AppTextStyles.heading,
                    ),
                  ),

                  const SizedBox(height: AppSpacing.xl),

                  GlassCard(
                    child: Column(
                      children: [

                        ListTile(
                          leading: const Icon(
                            Icons.phone_android,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "Device Name",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: const Text(
                            "Satyadev's Phone",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),

                        const Divider(),

                        ListTile(
                          leading: const Icon(
                            Icons.high_quality,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "Audio Quality",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: const Text(
                            "High",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),

                        const Divider(),

                        ListTile(
                          leading: const Icon(
                            Icons.timer,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "Sync Delay",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: const Text(
                            "20 ms",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),

                        const Divider(),
                                                SwitchListTile(
                          secondary: const Icon(
                            Icons.dark_mode,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "Dark Theme",
                            style: TextStyle(color: Colors.white),
                          ),
                          value: darkMode,
                          onChanged: (value) {
                            setState(() {
                              darkMode = value;
                            });
                          },
                        ),

                        const Divider(),

                        SwitchListTile(
                          secondary: const Icon(
                            Icons.notifications,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "Notifications",
                            style: TextStyle(color: Colors.white),
                          ),
                          value: notifications,
                          onChanged: (value) {
                            setState(() {
                              notifications = value;
                            });
                          },
                        ),

                        const Divider(),

                        ListTile(
                          leading: const Icon(
                            Icons.info_outline,
                            color: Colors.cyan,
                          ),
                          title: const Text(
                            "About",
                            style: TextStyle(color: Colors.white),
                          ),
                          subtitle: const Text(
                            "EchoSync v1.0",
                            style: TextStyle(color: Colors.white70),
                          ),
                        ),
                      ],
                    ),
                  ),

                  const SizedBox(height: AppSpacing.xl),

                  Column(
  children: [

    PrimaryButton(
      title: "Device Management",
      onTap: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (_) => const DeviceManagementScreen(),
          ),
        );
      },
    ),

    const SizedBox(height: 16),

    PrimaryButton(
      title: "Save Settings",
      onTap: () {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text("Settings Saved"),
          ),
        );
      },
    ),

  ],
),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}