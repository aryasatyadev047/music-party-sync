import 'dart:math';

import 'package:flutter/material.dart';

import '../../core/constants/app_colors.dart';
import '../../core/constants/app_spacing.dart';
import '../../core/constants/app_text_styles.dart';
import '../../shared/widgets/page_background.dart';
import '../../shared/widgets/primary_button.dart';
import '../settings/settings_screen.dart';

class PlayerScreen extends StatefulWidget {
  const PlayerScreen({super.key});

  @override
  State<PlayerScreen> createState() => _PlayerScreenState();
}

class _PlayerScreenState extends State<PlayerScreen>
    with TickerProviderStateMixin {

  late final AnimationController _rotationController;
  late final AnimationController _pulseController;
  late final AnimationController _equalizerController;

  bool isPlaying = true;

  double progress = 0.45;
  double volume = 0.65;

  final List<Map<String, dynamic>> devices = [
    {
      "name": "Satyadev",
      "icon": Icons.phone_android,
      "host": true,
    },
    {
      "name": "Rahul",
      "icon": Icons.laptop,
      "host": false,
    },
    {
      "name": "Aman",
      "icon": Icons.phone_android,
      "host": false,
    },
    {
      "name": "Riya",
      "icon": Icons.phone_android,
      "host": false,
    },
  ];

  @override
  void initState() {
    super.initState();

    _rotationController = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 18),
    )..repeat();

    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 900),
      lowerBound: 0.92,
      upperBound: 1.05,
    )..repeat(reverse: true);

    _equalizerController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 600),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _rotationController.dispose();
    _pulseController.dispose();
    _equalizerController.dispose();
    super.dispose();
  }
    @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: PageBackground(
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 600),
          decoration: const BoxDecoration(
            gradient: LinearGradient(
              colors: [
                Color(0xFF08111F),
                Color(0xFF131C35),
                Color(0xFF101A2C),
              ],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
          ),
          child: SafeArea(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final desktop = constraints.maxWidth > 900;

                return Padding(
                  padding: const EdgeInsets.all(AppSpacing.lg),
                  child: desktop
                      ? _buildDesktopLayout(context)
                      : _buildMobileLayout(context),
                );
              },
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildDesktopLayout(BuildContext context) {
    return Row(
      children: [
        Expanded(
          flex: 2,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              _buildAlbumArt(),
              const SizedBox(height: 30),

              Text(
                "Blinding Lights",
                style: AppTextStyles.heading,
                textAlign: TextAlign.center,
              ),

              const SizedBox(height: 8),

              Text(
                "The Weeknd",
                style: AppTextStyles.body,
              ),

              const SizedBox(height: 30),

              _buildEqualizer(),

              const SizedBox(height: 20),

              _buildSyncCard(),
            ],
          ),
        ),

        const SizedBox(width: 40),

        Expanded(
          flex: 3,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [

                Row(
                  mainAxisAlignment:
                      MainAxisAlignment.spaceBetween,
                  children: [

                    Text(
                      "Now Playing",
                      style: AppTextStyles.heading,
                    ),

                    IconButton(
                      onPressed: () {
                        Navigator.push(
                          context,
                          MaterialPageRoute(
                            builder: (_) =>
                                const SettingsScreen(),
                          ),
                        );
                      },
                      icon: const Icon(
                        Icons.settings,
                        color: Colors.white,
                      ),
                    ),
                  ],
                ),

                const SizedBox(height: 30),

                Slider(
                  value: progress,
                  onChanged: (value) {
                    setState(() {
                      progress = value;
                    });
                  },
                ),

                Row(
                  mainAxisAlignment:
                      MainAxisAlignment.spaceBetween,
                  children: [

                    const Text(
                      "01:35",
                      style: TextStyle(
                        color: Colors.white70,
                      ),
                    ),

                    Text(
                      "${(progress * 100).toInt()} %",
                      style: const TextStyle(
                        color: Colors.greenAccent,
                        fontWeight: FontWeight.bold,
                      ),
                    ),

                    const Text(
                      "03:44",
                      style: TextStyle(
                        color: Colors.white70,
                      ),
                    ),

                  ],
                ),

                const SizedBox(height: 40),

                _buildPlayerControls(),

                const SizedBox(height: 40),

                Text(
                  "Volume",
                  style: AppTextStyles.title,
                ),

                Slider(
                  value: volume,
                  onChanged: (value) {
                    setState(() {
                      volume = value;
                    });
                  },
                ),

                const SizedBox(height: 30),

                Text(
                  "Connected Devices",
                  style: AppTextStyles.title,
                ),

                const SizedBox(height: 15),

                _buildDeviceList(),

                const SizedBox(height: 25),

                PrimaryButton(
                  title: "Start Playback",
                  onTap: () {},
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildMobileLayout(BuildContext context) {
    return SingleChildScrollView(
      child: Column(
        children: [

          _buildAlbumArt(),

          const SizedBox(height: 25),

          Text(
            "Blinding Lights",
            style: AppTextStyles.title,
          ),

          const SizedBox(height: 8),

          Text(
            "The Weeknd",
            style: AppTextStyles.body,
          ),

          const SizedBox(height: 20),

          _buildEqualizer(),

          const SizedBox(height: 25),

          Slider(
            value: progress,
            onChanged: (value) {
              setState(() {
                progress = value;
              });
            },
          ),

          _buildPlayerControls(),

          const SizedBox(height: 20),

          Text(
            "Volume",
            style: AppTextStyles.title,
          ),

          Slider(
            value: volume,
            onChanged: (value) {
              setState(() {
                volume = value;
              });
            },
          ),

          const SizedBox(height: 20),

          _buildSyncCard(),

          const SizedBox(height: 20),

          _buildDeviceList(),

          const SizedBox(height: 30),

          PrimaryButton(
            title: "Start Playback",
            onTap: () {},
          ),
        ],
      ),
    );
  }

  // ===== PART 3 CONTINUES FROM HERE =====
  Widget _buildAlbumArt() {
  return AnimatedBuilder(
    animation: _rotationController,
    builder: (context, child) {
      return Transform.rotate(
        angle: _rotationController.value * 2 * pi,
        child: Container(
          height: 280,
          width: 280,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(35),
            gradient: const LinearGradient(
              colors: [
                Color(0xFF00D4FF),
                Color(0xFF7B61FF),
              ],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
            boxShadow: const [
              BoxShadow(
                color: Color(0x5500D4FF),
                blurRadius: 35,
                spreadRadius: 8,
              ),
            ],
          ),
          child: const Center(
            child: Icon(
              Icons.album_rounded,
              color: Colors.white,
              size: 130,
            ),
          ),
        ),
      );
    },
  );
}

Widget _buildEqualizer() {
  return AnimatedBuilder(
    animation: _equalizerController,
    builder: (context, child) {
      return Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: List.generate(
          12,
          (index) {
            final height =
                15 +
                (sin(
                          (_equalizerController.value * 2 * pi) +
                              index,
                        ) +
                        1) *
                    18;

            return Padding(
              padding:
                  const EdgeInsets.symmetric(horizontal: 3),
              child: AnimatedContainer(
                duration:
                    const Duration(milliseconds: 180),
                width: 6,
                height: height,
                decoration: BoxDecoration(
                  color: AppColors.primary,
                  borderRadius:
                      BorderRadius.circular(20),
                ),
              ),
            );
          },
        ),
      );
    },
  );
}

Widget _buildPlayerControls() {
  return Row(
    mainAxisAlignment: MainAxisAlignment.spaceEvenly,
    children: [

      IconButton(
        onPressed: () {},
        icon: const Icon(
          Icons.skip_previous_rounded,
          size: 42,
          color: Colors.white,
        ),
      ),

      ScaleTransition(
        scale: _pulseController,
        child: Container(
          decoration: const BoxDecoration(
            color: AppColors.primary,
            shape: BoxShape.circle,
          ),
          child: IconButton(
            onPressed: () {
              setState(() {
                isPlaying = !isPlaying;
              });
            },
            icon: Icon(
              isPlaying
                  ? Icons.pause_rounded
                  : Icons.play_arrow_rounded,
              color: Colors.black,
              size: 45,
            ),
          ),
        ),
      ),

      IconButton(
        onPressed: () {},
        icon: const Icon(
          Icons.skip_next_rounded,
          size: 42,
          color: Colors.white,
        ),
      ),
    ],
  );
}

Widget _buildSyncCard() {
  return Container(
    padding: const EdgeInsets.all(18),
    decoration: BoxDecoration(
      color: Colors.white.withOpacity(.08),
      borderRadius: BorderRadius.circular(22),
      border: Border.all(
        color: Colors.greenAccent.withOpacity(.4),
      ),
    ),
    child: const Row(
      children: [

        Icon(
          Icons.sync,
          color: Colors.greenAccent,
        ),

        SizedBox(width: 12),

        Expanded(
          child: Text(
            "Perfect Sync\nLatency : 18 ms",
            style: TextStyle(
              color: Colors.greenAccent,
              fontWeight: FontWeight.bold,
            ),
          ),
        ),

      ],
    ),
  );
}

Widget _buildDeviceList() {
  return Column(
    children: devices.map((device) {

      return Container(
        margin: const EdgeInsets.only(bottom: 12),
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: Colors.white.withOpacity(.05),
          borderRadius: BorderRadius.circular(20),
        ),
        child: Row(
          children: [

            CircleAvatar(
              backgroundColor: AppColors.primary,
              child: Icon(
                device["icon"],
                color: Colors.black,
              ),
            ),

            const SizedBox(width: 15),

            Expanded(
              child: Column(
                crossAxisAlignment:
                    CrossAxisAlignment.start,
                children: [

                  Text(
                    device["name"],
                    style: const TextStyle(
                      color: Colors.white,
                      fontWeight: FontWeight.bold,
                    ),
                  ),

                  Text(
                    device["host"]
                        ? "Host"
                        : "Connected",
                    style: TextStyle(
                      color: device["host"]
                          ? Colors.orange
                          : Colors.greenAccent,
                    ),
                  ),

                ],
              ),
            ),

            const Icon(
              Icons.circle,
              color: Colors.greenAccent,
              size: 14,
            ),

          ],
        ),
      );

    }).toList(),
  );
}

// ===== PART 4 STARTS HERE =====
}