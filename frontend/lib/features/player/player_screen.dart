import 'package:flutter/material.dart';

import '../../core/constants/app_colors.dart';
import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/primary_button.dart';
import '../settings/settings_screen.dart';

class PlayerScreen extends StatelessWidget {
  const PlayerScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(AppSpacing.lg),
            child: LayoutBuilder(
              builder: (context, constraints) {

                final desktop = constraints.maxWidth > 900;

                if (desktop) {

                  return Row(
                    children: [

                      Expanded(
                        flex: 2,
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [

                            Container(
                              height: 320,
                              width: 320,
                              decoration: BoxDecoration(
                                borderRadius:
                                    BorderRadius.circular(30),
                                gradient: const LinearGradient(
                                  colors: [
                                    AppColors.primary,
                                    AppColors.secondary,
                                  ],
                                  begin: Alignment.topLeft,
                                  end: Alignment.bottomRight,
                                ),
                              ),
                              child: const Icon(
                                Icons.music_note_rounded,
                                size: 140,
                                color: Colors.white,
                              ),
                            ),

                            const SizedBox(
                              height: AppSpacing.lg,
                            ),

                            Text(
                              "Blinding Lights",
                              style: AppTextStyles.heading,
                              textAlign: TextAlign.center,
                            ),

                            const SizedBox(
                              height: AppSpacing.sm,
                            ),

                            Text(
                              "The Weeknd",
                              style: AppTextStyles.body,
                            ),

                          ],
                        ),
                      ),

                      const SizedBox(
                        width: AppSpacing.xl,
                      ),

                      Expanded(
                        flex: 3,
                        child: Column(
                          crossAxisAlignment:
                              CrossAxisAlignment.start,
                          children: [

                            Row(
  mainAxisAlignment: MainAxisAlignment.spaceBetween,
  children: [
    Text(
      "Now Playing",
      style: AppTextStyles.heading,
    ),
    IconButton(
      icon: const Icon(
        Icons.settings,
        color: Colors.white,
      ),
      onPressed: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (_) => const SettingsScreen(),
          ),
        );
      },
    ),
  ],
),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            const LinearProgressIndicator(
                              value: .45,
                              minHeight: 8,
                              borderRadius:
                                  BorderRadius.all(
                                Radius.circular(20),
                              ),
                            ),

                            const SizedBox(
                              height: AppSpacing.sm,
                            ),

                            const Row(
                              mainAxisAlignment:
                                  MainAxisAlignment.spaceBetween,
                              children: [

                                Text(
                                  "1:35",
                                  style: TextStyle(
                                    color: Colors.white70,
                                  ),
                                ),

                                Text(
                                  "3:44",
                                  style: TextStyle(
                                    color: Colors.white70,
                                  ),
                                ),

                              ],
                            ),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            Row(
                              mainAxisAlignment:
                                  MainAxisAlignment.spaceEvenly,
                              children: [

                                IconButton(
                                  onPressed: () {},
                                  icon: const Icon(
                                    Icons.skip_previous,
                                    size: 40,
                                    color: Colors.white,
                                  ),
                                ),

                                Container(
                                  decoration: BoxDecoration(
                                    color: AppColors.primary,
                                    shape: BoxShape.circle,
                                  ),
                                  child: IconButton(
                                    onPressed: () {},
                                    icon: const Icon(
                                      Icons.play_arrow,
                                      size: 42,
                                      color: Colors.black,
                                    ),
                                  ),
                                ),

                                IconButton(
                                  onPressed: () {},
                                  icon: const Icon(
                                    Icons.skip_next,
                                    size: 40,
                                    color: Colors.white,
                                  ),
                                ),

                              ],
                            ),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            Text(
                              "Volume",
                              style: AppTextStyles.title,
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            const Slider(
                              value: .65,
                              onChanged: null,
                            ),

                            const SizedBox(
                              height: AppSpacing.xl,
                            ),

                            Text(
                              "Connected Devices",
                              style: AppTextStyles.title,
                            ),

                            const SizedBox(
                              height: AppSpacing.md,
                            ),

                            Expanded(
                              child: ListView(
                                children: const [
                                                                      ListTile(
                                    leading: CircleAvatar(
                                      backgroundColor: Colors.green,
                                      child: Icon(
                                        Icons.phone_android,
                                        color: Colors.white,
                                      ),
                                    ),
                                    title: Text(
                                      "Satyadev's Phone",
                                      style: TextStyle(color: Colors.white),
                                    ),
                                    subtitle: Text(
                                      "Connected",
                                      style: TextStyle(color: Colors.greenAccent),
                                    ),
                                  ),

                                  ListTile(
                                    leading: CircleAvatar(
                                      backgroundColor: Colors.green,
                                      child: Icon(
                                        Icons.laptop,
                                        color: Colors.white,
                                      ),
                                    ),
                                    title: Text(
                                      "Rahul's Laptop",
                                      style: TextStyle(color: Colors.white),
                                    ),
                                    subtitle: Text(
                                      "Connected",
                                      style: TextStyle(color: Colors.greenAccent),
                                    ),
                                  ),

                                  ListTile(
                                    leading: CircleAvatar(
                                      backgroundColor: Colors.green,
                                      child: Icon(
                                        Icons.phone_android,
                                        color: Colors.white,
                                      ),
                                    ),
                                    title: Text(
                                      "Aman's Phone",
                                      style: TextStyle(color: Colors.white),
                                    ),
                                    subtitle: Text(
                                      "Connected",
                                      style: TextStyle(color: Colors.greenAccent),
                                    ),
                                  ),

                                  ListTile(
                                    leading: CircleAvatar(
                                      backgroundColor: Colors.green,
                                      child: Icon(
                                        Icons.phone_android,
                                        color: Colors.white,
                                      ),
                                    ),
                                    title: Text(
                                      "Riya's Phone",
                                      style: TextStyle(color: Colors.white),
                                    ),
                                    subtitle: Text(
                                      "Connected",
                                      style: TextStyle(color: Colors.greenAccent),
                                    ),
                                  ),
                                ],
                              ),
                            ),

                            const SizedBox(
                              height: AppSpacing.lg,
                            ),

                            Row(
                              children: const [
                                Icon(
                                  Icons.sync,
                                  color: Colors.greenAccent,
                                ),
                                SizedBox(width: 8),
                                Text(
                                  "All Devices Synced • 18 ms",
                                  style: TextStyle(
                                    color: Colors.greenAccent,
                                    fontSize: 16,
                                  ),
                                ),
                              ],
                            ),

                            const SizedBox(
                              height: AppSpacing.lg,
                            ),

                            PrimaryButton(
                              title: "Start Playback",
                              onTap: () {},
                            ),
                          ],
                        ),
                      ),
                    ],
                  );
                }

                // ---------- MOBILE LAYOUT ----------

                return SingleChildScrollView(
                  child: Column(
                    children: [

                      Container(
                        height: 250,
                        width: 250,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(30),
                          gradient: const LinearGradient(
                            colors: [
                              AppColors.primary,
                              AppColors.secondary,
                            ],
                          ),
                        ),
                        child: const Icon(
                          Icons.music_note_rounded,
                          color: Colors.white,
                          size: 120,
                        ),
                      ),

                      const SizedBox(height: AppSpacing.lg),

                      Text(
                        "Blinding Lights",
                        style: AppTextStyles.title,
                      ),

                      const SizedBox(height: AppSpacing.sm),

                      Text(
                        "The Weeknd",
                        style: AppTextStyles.body,
                      ),

                      const SizedBox(height: AppSpacing.xl),

                      const LinearProgressIndicator(
                        value: .45,
                        minHeight: 8,
                      ),

                      const SizedBox(height: AppSpacing.md),

                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                        children: [
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.skip_previous,
                                color: Colors.white, size: 36),
                          ),
                          Container(
                            decoration: const BoxDecoration(
                              color: AppColors.primary,
                              shape: BoxShape.circle,
                            ),
                            child: IconButton(
                              onPressed: () {},
                              icon: const Icon(
                                Icons.play_arrow,
                                color: Colors.black,
                                size: 42,
                              ),
                            ),
                          ),
                          IconButton(
                            onPressed: () {},
                            icon: const Icon(Icons.skip_next,
                                color: Colors.white, size: 36),
                          ),
                        ],
                      ),

                      const SizedBox(height: AppSpacing.lg),

                      Text(
                        "Volume",
                        style: AppTextStyles.title,
                      ),

                      const Slider(
                        value: .65,
                        onChanged: null,
                      ),

                      const SizedBox(height: AppSpacing.lg),

                      PrimaryButton(
                        title: "Start Playback",
                        onTap: () {},
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}