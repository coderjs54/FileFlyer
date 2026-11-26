package com.fileflyer.transfer

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    acquireMulticastLock()
    requestLocalNetworkPermissionIfNeeded()
  }

  /**
   * Android 17 起，targetSdk 37 的应用必须获得本地网络运行时权限，
   * 否则 UDP 广播和局域网 TCP 连接都会被系统拦截。
   */
  private fun requestLocalNetworkPermissionIfNeeded() {
    if (Build.VERSION.SDK_INT < 37) {
      return
    }
    val permission = Manifest.permission.ACCESS_LOCAL_NETWORK
    if (checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(permission), REQUEST_LOCAL_NETWORK)
    }
  }

  /** 持有 Wi-Fi 组播锁，确保能稳定接收局域网 UDP 广播 */
  private fun acquireMulticastLock() {
    val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    multicastLock = wifiManager.createMulticastLock("fileflyer-discovery").apply {
      setReferenceCounted(false)
      acquire()
    }
  }

  override fun onDestroy() {
    multicastLock?.let { lock ->
      if (lock.isHeld) {
        lock.release()
      }
    }
    multicastLock = null
    super.onDestroy()
  }

  companion object {
    private const val REQUEST_LOCAL_NETWORK = 1001
  }
}
