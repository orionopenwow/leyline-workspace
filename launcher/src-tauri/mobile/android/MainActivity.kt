package com.leylinelauncher.app

// The phone app's activity (docs/ANDROID.md). `tauri android init` makes the Android project in gen/android (never
// committed); the Android workflow (tools/github-workflows/leyline-android.yml) puts this file over its MainActivity.kt,
// keeping the package line init wrote. The one change from Tauri's own: full screen, the status and navigation bars
// hidden (a swipe from the edge shows them for a moment), so the browser game and the launcher get the whole screen.
// phone.css keeps the launcher clear of the camera cut-out with the safe-area insets.

import android.os.Bundle
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    WindowCompat.setDecorFitsSystemWindows(window, false)
    fullScreen()
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) fullScreen()
  }

  private fun fullScreen() {
    val bars = WindowCompat.getInsetsController(window, window.decorView)
    bars.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    bars.hide(WindowInsetsCompat.Type.systemBars())
  }
}
