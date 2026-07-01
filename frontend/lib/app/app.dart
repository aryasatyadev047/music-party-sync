import 'package:flutter/material.dart';

import '../features/splash/splash_screen.dart';
import 'theme.dart';

class MusicPartySyncApp extends StatelessWidget {
  const MusicPartySyncApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Music Party Sync',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.darkTheme,
      home: const SplashScreen(),
    );
  }
}