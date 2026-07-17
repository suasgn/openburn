package com.siagian.openburn

import android.content.res.Configuration
import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    configureSystemBars()
    WindowCompat.setDecorFitsSystemWindows(window, false)

    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(systemBarColor())
    val initialPadding = intArrayOf(
      content.paddingLeft,
      content.paddingTop,
      content.paddingRight,
      content.paddingBottom,
    )

    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
      view.setPadding(
        initialPadding[0] + systemBars.left,
        initialPadding[1] + systemBars.top,
        initialPadding[2] + systemBars.right,
        initialPadding[3] + systemBars.bottom,
      )
      insets
    }
    ViewCompat.requestApplyInsets(content)
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    configureSystemBars()
    findViewById<View>(android.R.id.content).setBackgroundColor(systemBarColor())
  }

  private fun configureSystemBars() {
    val isNightMode = isNightMode()
    val color = systemBarColor()

    window.statusBarColor = color
    window.navigationBarColor = color
    window.decorView.setBackgroundColor(color)

    WindowCompat.getInsetsController(window, window.decorView).apply {
      isAppearanceLightStatusBars = !isNightMode
      isAppearanceLightNavigationBars = !isNightMode
    }
  }

  private fun systemBarColor(): Int {
    return if (isNightMode()) Color.rgb(28, 28, 30) else Color.WHITE
  }

  private fun isNightMode(): Boolean {
    return (resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES
  }
}
