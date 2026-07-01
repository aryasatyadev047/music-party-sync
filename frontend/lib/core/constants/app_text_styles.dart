import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';

import 'app_colors.dart';

class AppTextStyles {
  AppTextStyles._();

  static TextStyle heading = GoogleFonts.poppins(
    color: AppColors.white,
    fontSize: 34,
    fontWeight: FontWeight.bold,
  );

  static TextStyle title = GoogleFonts.poppins(
    color: AppColors.white,
    fontSize: 22,
    fontWeight: FontWeight.w600,
  );

  static TextStyle body = GoogleFonts.poppins(
    color: AppColors.grey,
    fontSize: 16,
  );

  static TextStyle button = GoogleFonts.poppins(
    color: Colors.black,
    fontSize: 18,
    fontWeight: FontWeight.bold,
  );
}