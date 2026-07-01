import 'package:flutter/material.dart';

class PageBackground extends StatelessWidget {
  final Widget child;

  const PageBackground({
    super.key,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            Color(0xff090B12),
            Color(0xff111827),
            Color(0xff090B12),
          ],
        ),
      ),
      child: child,
    );
  }
}